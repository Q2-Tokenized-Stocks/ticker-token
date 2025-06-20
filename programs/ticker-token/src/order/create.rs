use anchor_lang::prelude::*;

use anchor_spl::{
	token::{self, TokenAccount, Token},
	associated_token::{
		AssociatedToken, get_associated_token_address
	}
};

use crate::{
	Registry,
	errors::ErrorCode,
	utils::verify_ed25519_ix,
	order::{
		types::*,
		state::OrderState,
	},
};

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

    /// CHECK: PDA для блокировки средств перед выполнением ордера на покупку
    #[account(
        mut,
        seeds = [b"payment_escrow", payer.key().as_ref(), payload.payment_mint.as_ref()],
        bump,
    )]
    pub payment_escrow_account: UncheckedAccount<'info>,

    /// CHECK: escrow PDA аккаунт
    #[account(mut)]
    pub escrow_owner: UncheckedAccount<'info>,

    // АТА для блокировки токенов проавца при продаже
    #[account(mut)]
    pub token_escrow_account: Account<'info, TokenAccount>,

    // АТА для гарантии выплаты продавцу при продаже
    #[account(mut)]
    pub payment_release_account: Account<'info, TokenAccount>,

    /// CHECK: lp pool PDA — источник средств для выплаты продавцу
    #[account(
        mut,
        seeds = [b"lp", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
        bump,
    )]
    pub lp_pool: UncheckedAccount<'info>,

    /// CHECK: системная переменная с инструкциями — используется для верификации ed25519-подписи
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[event]
pub struct OrderCreated {
    pub id: u64,
    pub maker: Pubkey,
    pub side: Side,
    pub order_type: OrderType,
    pub symbol: [u8; 8],
    pub amount: u64,
    pub price: u64,
    pub fee: u64,
    pub payment_mint: Pubkey,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub sig: [u8; 64],
}

pub fn order_create(
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

	// Оракул установлен
	require!(ctx.accounts.registry.oracle != Pubkey::default(), ErrorCode::InvalidOracle);
	// Проверяем подпись оракула
	verify_ed25519_ix(&ctx.accounts.instruction_sysvar, &ctx.accounts.registry.oracle, &hash.0)?;

	let token_escrow_account = get_associated_token_address(
		&ctx.accounts.escrow_owner.key(),
		&payload.token_mint,
	);
	require!(ctx.accounts.token_escrow_account.key() == token_escrow_account, ErrorCode::InvalidEscrowAccount);

	let expected_release_account = get_associated_token_address(
		&ctx.accounts.escrow_owner.key(),
		&payload.payment_mint,
	);
	require!(ctx.accounts.payment_release_account.key() == expected_release_account, ErrorCode::InvalidEscrowAccount);

	let (expected_payment_escrow, _) = Pubkey::find_program_address(
		&[b"payment_escrow", ctx.accounts.payer.key().as_ref(), payload.payment_mint.as_ref()],
		ctx.program_id,
	);
	require!(ctx.accounts.payment_escrow_account.key() == expected_payment_escrow, ErrorCode::InvalidVaultAccount);

	let (expected_lp_pool, _) = Pubkey::find_program_address(
		&[b"lp", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
		ctx.program_id,
	);
	require!(ctx.accounts.lp_pool.key() == expected_lp_pool, ErrorCode::InvalidVaultAccount);

	let total = payload.amount
		.checked_mul(payload.price).ok_or(ErrorCode::Overflow)?
		.checked_add(payload.fee).ok_or(ErrorCode::Overflow)?;

	match payload.side {
		Side::Buy => {
			// Покупка: переводим платёж в payment_escrow_account (escrow аналог)
			let cpi_ctx = CpiContext::new(
				ctx.accounts.token_program.to_account_info(),
				token::Transfer {
					from: ctx.accounts.user_payment_account.to_account_info(),
					to: ctx.accounts.payment_escrow_account.to_account_info(),
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
					to: ctx.accounts.token_escrow_account.to_account_info(),
					authority: ctx.accounts.payer.to_account_info(),
				},
			);
			token::transfer(cpi_ctx, payload.amount)?;

			// Переводим из lp_pool платёжный токен в payment_release_account (гарантируем выплату)
			let cpi_ctx = CpiContext::new(
				ctx.accounts.token_program.to_account_info(),
				token::Transfer {
					from: ctx.accounts.lp_pool.to_account_info(),
					to: ctx.accounts.payment_release_account.to_account_info(),
					authority: ctx.accounts.payer.to_account_info(),
				},
			);
			token::transfer(cpi_ctx, payload.amount * payload.price)?;
		},
	}

	// Записываем минимальные данные в OrderState (всё остальное — в ивенте)
	let order = &mut ctx.accounts.order;
	order.maker = ctx.accounts.payer.key();
	order.side = payload.side.clone();
	order.status = OrderStatus::Pending;
	order.created_at = now;

	emit!(OrderCreated {
		id: payload.id,
		maker: order.maker,
		side: payload.side,
		order_type: payload.order_type,
		symbol: payload.symbol,
		amount: payload.amount,
		price: payload.price,
		fee: payload.fee,
		payment_mint: payload.payment_mint,
		created_at: now,
		expires_at: payload.expires_at,
		sig,
	});

	Ok(())
}