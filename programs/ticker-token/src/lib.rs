use anchor_lang::prelude::*;
use anchor_lang::prelude::Pubkey;

use anchor_spl::token::{TokenAccount, Token};
use anchor_spl::token_interface::{initialize_mint2, InitializeMint2};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("8mPWhPVTKG4zXp5JFqsxA5ZMNhUqWThz5MJjrQS4VB4Z");

#[program]
pub mod ticker_token {
    use super::*;

    pub fn init(ctx: Context<InitContext>) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.payer.key();

        Ok(())
    }

    pub fn set_oracle(ctx: Context<AuthorityContext>, new_oracle: Pubkey) -> Result<()> {
        let registry = &mut ctx.accounts.registry;

        registry.oracle = new_oracle;
        Ok(())
    }

    pub fn transfer_authority(ctx: Context<AuthorityContext>, new_authority: Pubkey) -> Result<()> {
        require!(new_authority != Pubkey::default(), TickerError::InvalidAuthority);

        ctx.accounts.registry.authority = new_authority;
        Ok(())
    }

    pub fn create_ticker(ctx: Context<TickerContext>, symbol: String, decimals: u8) -> Result<()> {
        require!(symbol.len() <= 8, TickerError::TickerTooLong);

        // Symbol of mint PDA must match the ticker symbol
        let (expected_mint, _) = Pubkey::find_program_address(
            &[b"mint", symbol.as_bytes()],
            ctx.program_id
        );
        require!(ctx.accounts.mint.key() == expected_mint, TickerError::Unauthorized);

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            InitializeMint2 {
                mint: ctx.accounts.mint.to_account_info(),
            },
        );

        initialize_mint2(
            cpi_ctx,
            decimals,
            &ctx.accounts.authority.key(),
            Some(&ctx.accounts.authority.key()),
        )?;

        let mut fixed_symbol = [0u8; 8];
        fixed_symbol[..symbol.len()].copy_from_slice(symbol.as_bytes());

        let data = &mut ctx.accounts.ticker_data;

        data.decimals = decimals;
        data.symbol = fixed_symbol;
        data.mint = ctx.accounts.mint.key();

        Ok(())
    }
    
    pub fn create_order(
        ctx: Context<CreateOrder>,
        payload: OraclePayload,
        sig: [u8; 64],
    ) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        if let Some(exp) = payload.expires_at {
            require!(now <= exp, ErrorCode::PayloadExpired);
        }

        use anchor_lang::solana_program::keccak;
        let mut serialized = vec![];
        payload.serialize(&mut serialized)?;
        let hash = keccak::hash(&serialized);

        // Проверяем подпись оракула на полезной нагрузке
        require!(ctx.accounts.registry.oracle != Pubkey::default(), ErrorCode::InvalidOracle);
        verify_ed25519_ix(&ctx.accounts.instruction_sysvar, &ctx.accounts.registry.oracle, &hash.0)?;

        let total = payload.amount
            .checked_mul(payload.price).ok_or(ErrorCode::Overflow)?
            .checked_add(payload.fee).ok_or(ErrorCode::Overflow)?;

        match payload.side {
            Side::Buy => {
                // Покупка: переводим платёж в vault (escrow аналог)
                let cpi_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.user_payment_account.to_account_info(),
                        to: ctx.accounts.vault.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                );
                token::transfer(cpi_ctx, total)?;
            },
            Side::Sell => {
                // Продажа: переводим токены в escrow
                let cpi_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.user_payment_account.to_account_info(),
                        to: ctx.accounts.escrow_token.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                );
                token::transfer(cpi_ctx, payload.amount)?;

                // Переводим из vault платёжный токен в escrow (гарантируем выплату)
                let cpi_ctx = CpiContext::new(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: ctx.accounts.vault.to_account_info(),
                        to: ctx.accounts.escrow_payment.to_account_info(),
                        authority: ctx.accounts.payer.to_account_info(),
                    },
                );
                token::transfer(cpi_ctx, payload.amount * payload.price)?;
            },
        }

        // Записываем минимальные данные в OrderState (всё остальное — в ивенте)
        let order = &mut ctx.accounts.order;
        order.maker = ctx.accounts.payer.key();
        order.id = payload.id;
        order.side = payload.side.clone();
        order.status = OrderStatus::Pending;
        order.created_at = now;

        emit!(OrderCreated {
            maker: order.maker,
            id: order.id,
            side: payload.side,
            order_type: payload.order_type,
            amount: payload.amount,
            price: payload.price,
            fee: payload.fee,
            token_mint: payload.token_mint,
            payment_mint: payload.payment_mint,
            created_at: now,
            expires_at: payload.expires_at,
            sig,
        });

        Ok(())
    }
}

#[account]
pub struct TickerData {
    pub symbol: [u8; 8],
	pub decimals: u8,
    pub mint: Pubkey,
}

#[account]
pub struct Registry {
    pub authority: Pubkey,
    pub oracle: Pubkey,
}

#[derive(Accounts)]
pub struct InitContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        seeds = [b"registry"],
        bump,
        payer = payer,
        space = 8					// Anchor-дескриптор (дисриминатор, нужен всегда)
              + 32                  // authority: Pubkey
              + 32                  // oracle: Pubkey
    )]
    pub registry: Account<'info, Registry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AuthorityContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, has_one = authority)]
    pub registry: Account<'info, Registry>,
}

#[derive(Accounts)]
#[instruction(ticker: String, decimals: u8)]
pub struct TickerContext<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [b"registry"],
        bump,
        has_one = authority,
    )]
    pub registry: Account<'info, Registry>,

    #[account(
        init,
        seeds = [b"ticker", ticker.as_bytes()],
        bump,
        payer = authority,
        space = 8 	// anchor header
			  + 8 	// ticker
			  + 32 	// mint
			  + 1 	// decimals
    )]
    pub ticker_data: Account<'info, TickerData>,

    /// CHECK: mint account is created in this instruction and its validity is ensured by context
    #[account(
        init,
        seeds = [b"mint", ticker.as_bytes()],
        bump,
        payer = authority,
        space = 82, // 82 байта для mint согласно SPL Token
        owner = token_program.key()
    )]
    pub mint: AccountInfo<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[error_code]
pub enum TickerError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Ticker name too long")]
    TickerTooLong,
    #[msg("New authority must not be zero")]
    InvalidAuthority,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Oracle public key is not set in registry")]
    InvalidOracle,

    #[msg("Invalid oracle signature")]
    InvalidOracleSig,

    #[msg("Payload has expired")]
    PayloadExpired,

    #[msg("Math overflow")]
    Overflow,

    #[msg("Invalid payer token account")]
    InvalidUserTokenAccount,

    #[msg("Vault PDA mismatch or not found")]
    InvalidVaultAccount,

    #[msg("Escrow PDA mismatch or not found")]
    InvalidEscrowAccount,

    #[msg("Unauthorized attempt to access this resource")]
    Unauthorized,

    #[msg("Order with same ID already exists")]
    DuplicateOrderId,

    #[msg("Invalid signature instruction (not ED25519 program)")]
    InvalidSignatureInstruction,
}

// === enums ===
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Processing,
    Done,
    Cancelled,
}

// === order ===
#[account]
pub struct OrderState {
    pub maker: Pubkey,           // адрес создателя ордера
    pub id: u64,                 // уникальный идентификатор заявки
    pub side: Side,             // направление (покупка/продажа)
    pub status: OrderStatus,    // текущий статус заявки
    pub created_at: i64,        // когда была создана
}

#[event]
pub struct OrderCreated {
    pub maker: Pubkey,
    pub id: u64,
    pub side: Side,
    pub order_type: OrderType,
    pub amount: u64,
    pub price: u64,
    pub fee: u64,
    pub token_mint: Pubkey,
    pub payment_mint: Pubkey,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub sig: [u8; 64],
}

// === payload  ===
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OraclePayload {
    pub side: Side,
    pub order_type: OrderType,
    pub amount: u64,
    pub price: u64,
    pub fee: u64,
    pub token_mint: Pubkey,
    pub payment_mint: Pubkey,
    pub expires_at: Option<i64>,
    pub id: u64,
}

// === context ===
#[derive(Accounts)]
#[instruction(payload: OraclePayload)]
pub struct CreateOrder<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        seeds = [b"order", payer.key().as_ref(), &payload.id.to_le_bytes()],
        bump,
        space = 8 + std::mem::size_of::<OrderState>(),
    )]
    pub order: Account<'info, OrderState>,

    #[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

    #[account(
        mut,
        constraint = user_payment_account.owner == payer.key(),
        constraint = user_payment_account.mint == payload.payment_mint,
    )]
    pub user_payment_account: Account<'info, TokenAccount>,

    /// CHECK: PDA vault — для залочки средств при покупке
    #[account(
        mut,
        seeds = [b"vault", payer.key().as_ref(), payload.payment_mint.as_ref()],
        bump,
    )]
    pub vault: UncheckedAccount<'info>,

    /// CHECK: escrow PDA аккаунт для временной залочки
    #[account(
        mut,
        seeds = [b"escrow", payer.key().as_ref()],
        bump,
    )]
    pub escrow_owner: UncheckedAccount<'info>,

    // АТА для токена, который продается
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = payload.token_mint,
        associated_token::authority = escrow_owner,
    )]
    pub escrow_token: Account<'info, TokenAccount>,

    // АТА для платежного токена (USDC или иное)
    #[account(
        init_if_needed,
        payer = payer,
        associated_token::mint = payload.payment_mint,
        associated_token::authority = escrow_owner,
    )]
    pub escrow_payment: Account<'info, TokenAccount>,

    /// CHECK: системная переменная с инструкциями — используется для верификации ed25519-подписи
    #[account(address = sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

// === utils ===
use solana_program::{
    sysvar::instructions::{load_instruction_at_checked},
    ed25519_program::ID as ED25519_PROGRAM_ID,
    instruction::Instruction,
};
//use crate::ErrorCode;

pub fn verify_ed25519_ix(
    instruction_sysvar: &AccountInfo,
    expected_pubkey: &Pubkey,
    expected_msg: &[u8],
) -> Result<()> {
    let ix = load_instruction_at_checked(0, instruction_sysvar)?;
    require!(ix.program_id == ED25519_PROGRAM_ID, ErrorCode::InvalidSignatureInstruction);

    let data = &ix.data;

    // Формат ed25519-инструкции:
    // https://docs.solana.com/developing/runtime-facilities/programs#ed25519-program
    // 1 byte - num_signatures
    // 1 byte - padding
    // 2 bytes - signature_offset
    // 2 bytes - signature_instruction_index
    // 2 bytes - public_key_offset
    // 2 bytes - public_key_instruction_index
    // 2 bytes - message_data_offset
    // 2 bytes - message_data_size
    // 2 bytes - message_instruction_index
    // 64 bytes - signature
    // 32 bytes - public key
    // N bytes - message

    if data.len() < 1 + 1 + 2*6 + 64 + 32 {
        return Err(ErrorCode::InvalidSignatureInstruction.into());
    }

    // Извлекаем смещения
    let pubkey_offset = u16::from_le_bytes([data[6], data[7]]) as usize;
    let message_offset = u16::from_le_bytes([data[10], data[11]]) as usize;
    let message_size = u16::from_le_bytes([data[12], data[13]]) as usize;

    // Проверка длины
    require!(data.len() >= message_offset + message_size, ErrorCode::InvalidSignatureInstruction);
    require!(data.len() >= pubkey_offset + 32, ErrorCode::InvalidSignatureInstruction);

    let pubkey_bytes = &data[pubkey_offset..pubkey_offset + 32];
    let message_bytes = &data[message_offset..message_offset + message_size];

    // Сравнение
    require!(pubkey_bytes == expected_pubkey.as_ref(), ErrorCode::InvalidOracleSig);
    require!(message_bytes == expected_msg, ErrorCode::InvalidOracleSig);

    Ok(())
}