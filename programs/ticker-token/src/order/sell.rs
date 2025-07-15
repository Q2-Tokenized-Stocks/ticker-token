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
pub struct SellOrderCreated {
    pub id: u64,

    pub maker: Pubkey,
    pub order_type: OrderType,

	pub token_mint: Pubkey,
    pub amount: u64,

    pub payment_mint: Pubkey,
    pub price: u64,
    pub fee: u64,

    pub created_at: i64,
    pub expires_at: i64,

    pub sig: [u8; 64],
}

#[derive(Accounts)]
#[instruction(payload: OraclePayload)]
pub struct CreateSellOrder<'info> {
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

	/// CHECK: PDA (для управления транзакциями токенов)
	#[account(
		init_if_needed,
		seeds = [b"lp_owner", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
		bump,
	)]
    pub lp_owner: UncheckedAccount<'info>,

	/// АТА пула ликвидности (привязан к lp_owner)
	#[account(
    	mut,
    	constraint = lp_vault.owner == lp_owner.key(),
    	constraint = lp_vault.mint == payload.payment_mint
	)]
	pub lp_vault: Account<'info, TokenAccount>,

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
		token::authority = order,
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
		token::authority = order,
	)]
    pub release_escrow_account: Account<'info, TokenAccount>,
	
	/// CHECK: instruction sysvar, used for verifying oracle signature
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,

    pub rent: Sysvar<'info, Rent>,
}

pub fn sell_order_create(
	ctx: Context<CreateSellOrder>,
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

	// lp_owner
	assert_pda(
		ctx.accounts.lp_owner.key(),
		&[b"lp_owner", payload.token_mint.as_ref(), payload.payment_mint.as_ref()],
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
			authority: ctx.accounts.lp_owner.to_account_info(),
		},
	);
	token::transfer(cpi_ctx, total)?;

	// Записываем минимальные данные в OrderState (всё остальное — в ивенте)
	let order = &mut ctx.accounts.order;

	order.id = payload.id;
	order.maker = ctx.accounts.payer.key();

	order.token_mint = payload.token_mint;
	order.amount = payload.amount;

	order.payment_mint = payload.payment_mint;
	order.price = payload.price;
	order.fee = payload.fee;

	order.status = OrderStatus::Pending;

	order.created_at = now;
	order.expires_at = payload.expires_at;

	emit!(SellOrderCreated {
		id: payload.id,

		maker: order.maker,
		order_type: payload.order_type,

		token_mint: payload.token_mint,
		amount: payload.amount,
		
		payment_mint: payload.payment_mint,
		price: payload.price,
		fee: payload.fee,
		
		created_at: now,
		expires_at: payload.expires_at,

		sig,
	});

	Ok(())
}