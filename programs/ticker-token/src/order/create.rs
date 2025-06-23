use anchor_lang::prelude::*;
use anchor_spl::{
	token::{self, TokenAccount, Token, Mint},
	associated_token::{AssociatedToken}
};
use crate::{
	Registry,
	errors::ErrorCode,
	utils::{verify_ed25519_ix, assert_pda},
	order::{types::*, state::OrderState},
};

#[event]
pub struct OrderCreated {
    pub id: u64,
    pub maker: Pubkey,
    pub side: Side,
    pub order_type: OrderType,

    pub amount: u64,
    pub price: u64,
    pub fee: u64,
    pub payment_mint: Pubkey,

    pub created_at: i64,
    pub expires_at: i64,
    pub sig: [u8; 64],
}

#[derive(Accounts)]
#[instruction(payload: OraclePayload)]
pub struct CreateOrder<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

	#[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

	#[account(
        init,
        payer = payer,
        seeds = [b"order", payer.key().as_ref(), &payload.id.to_le_bytes()],
        bump,
        space = 8 + std::mem::size_of::<OrderState>(),
    )]
    pub order: Account<'info, OrderState>,

	/// Платежный токен
	#[account(constraint = payment_mint_account.key() == payload.payment_mint)]
	pub payment_mint_account: Account<'info, Mint>,

	#[account(constraint = token_mint_account.key() == payload.token_mint)]
    pub token_mint_account: Account<'info, Mint>,

	/// АТА мейкера для платежного токена
    #[account(
        mut,
        constraint = maker_payment_account.owner == payer.key(),
        constraint = maker_payment_account.mint == payload.payment_mint,
    )]
    pub maker_payment_account: Account<'info, TokenAccount>,

	/// CHECK: Order Authority PDA (для управления трансакциями токенов)
	#[account(
		seeds = [b"program_owner", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
		bump,
	)]
    pub program_owner: UncheckedAccount<'info>,

	/// АТА пула ликвидности (привязан к program_owner)
	#[account(
    	mut,
    	constraint = lp_vault.owner == program_owner.key(),
    	constraint = lp_vault.mint == payload.payment_mint
	)]
	pub lp_vault: Account<'info, TokenAccount>,

	/// PDA для блокировки средств перед выполнением ордера на покупку
	#[account(
		init_if_needed,
		payer = payer,
        seeds = [
			b"payment_escrow", 
			payer.key().as_ref(), 
			payload.payment_mint.as_ref()
		],
        bump,
		token::mint = payment_mint_account,
		token::authority = program_owner
    )]
    pub payment_escrow_account: Account<'info, TokenAccount>,

    // PDA для блокировки токенов проавца при продаже
	#[account(
		init_if_needed,
		payer = payer,
		seeds = [
			b"token_escrow",
			payer.key().as_ref(),
			payload.token_mint.as_ref()
		],
		bump,
		token::mint = token_mint_account,
		token::authority = program_owner,
	)]
    pub token_escrow_account: Account<'info, TokenAccount>,

    // PDA для гарантии выплаты продавцу при продаже
	#[account(
		init_if_needed,
		payer = payer,
		seeds = [
			b"release_escrow",
			payer.key().as_ref(),
			payload.payment_mint.as_ref()
		],
		bump,
		token::mint = payment_mint_account,
		token::authority = program_owner,
	)]
    pub release_escrow_account: Account<'info, TokenAccount>,
	
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,

	/// CHECK: instruction sysvar, used for verifying oracle signature
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,
    pub system_program: Program<'info, System>,

    pub rent: Sysvar<'info, Rent>,
}

pub fn order_create(
	ctx: Context<CreateOrder>,
	payload: OraclePayload,
	sig: [u8; 64],
) -> Result<()> {
	// Payload не устарел
	let now = Clock::get()?.unix_timestamp;
	require!(now <= payload.expires_at, ErrorCode::PayloadExpired);

	// Проверка подписи от оракула
	use anchor_lang::solana_program::keccak;
	let mut serialized = vec![];
	payload.serialize(&mut serialized)?;
	let hash = keccak::hash(&serialized);

	require!(ctx.accounts.registry.oracle != Pubkey::default(), ErrorCode::InvalidOracle);
	verify_ed25519_ix(&ctx.accounts.instruction_sysvar, &ctx.accounts.registry.oracle, &hash.0)?;

	// payment_escrow_account
	assert_pda(
		ctx.accounts.payment_escrow_account.key(),
		&[b"payment_escrow", ctx.accounts.payer.key().as_ref(), payload.payment_mint.as_ref()],
	)?;

	// program_owner
	assert_pda(
		ctx.accounts.program_owner.key(),
		&[b"program_owner", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
	)?;

	// token_escrow_account
	assert_pda(
		ctx.accounts.token_escrow_account.key(),
		&[b"token_escrow", ctx.accounts.payer.key().as_ref(), payload.token_mint.as_ref()],
	)?;

	// release_escrow_account
	assert_pda(
		ctx.accounts.release_escrow_account.key(),
		&[b"release_escrow", ctx.accounts.payer.key().as_ref(), payload.payment_mint.as_ref()],
	)?;

	let total = payload.amount
		.checked_mul(payload.price).ok_or(ErrorCode::Overflow)?
		.checked_add(payload.fee).ok_or(ErrorCode::Overflow)?;
	
	match payload.side {
		Side::Buy => {
			// Покупка: переводим платёж в payment_escrow_account (escrow аналог)
			let cpi_ctx = CpiContext::new(
				ctx.accounts.token_program.to_account_info(),
				token::Transfer {
					from: ctx.accounts.maker_payment_account.to_account_info(),
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
					from: ctx.accounts.maker_payment_account.to_account_info(),
					to: ctx.accounts.token_escrow_account.to_account_info(),
					authority: ctx.accounts.payer.to_account_info(),
				},
			);
			token::transfer(cpi_ctx, payload.amount)?;

			// Переводим из lp_vault платёжный токен в release_escrow_account (гарантируем выплату)
			let cpi_ctx = CpiContext::new(
				ctx.accounts.payment_mint_account.to_account_info(),
				token::Transfer {
					from: ctx.accounts.lp_vault.to_account_info(),
					to: ctx.accounts.release_escrow_account.to_account_info(),
					authority: ctx.accounts.program_owner.to_account_info(),
				},
			);
			token::transfer(cpi_ctx, total)?;
		},
	}

	// Записываем минимальные данные в OrderState (всё остальное — в ивенте)
	let order = &mut ctx.accounts.order;

	order.id = payload.id;
	order.maker = ctx.accounts.payer.key();

	order.amount = payload.amount;
	order.price = payload.price;
	order.fee = payload.fee;
	order.payment_mint = payload.payment_mint;

	order.status = OrderStatus::Pending;
	order.created_at = now;
	order.expires_at = payload.expires_at;

	emit!(OrderCreated {
		id: payload.id,
		maker: order.maker,
		side: payload.side,
		order_type: payload.order_type,

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