use anchor_lang::{
	prelude::*,
	solana_program::keccak,
};
use anchor_spl::{
	token::{self, TokenAccount, Token, Mint}
};
use crate::{
	Registry,
	errors::ErrorCode,
	utils::{verify_ed25519_ix},
	order::{types::*, state::OrderState},
};


#[event]
pub struct OrderCreated {
    pub id: u64,
    pub maker: Pubkey,

    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Accounts)]
#[instruction(payload: OrderPayload)]
pub struct CreateBuyOrder<'info> {
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

	/// Токен (тикер), который мейкер хочет купить
	#[account(constraint = ticker_mint_account.key() == payload.ticker_mint)]
	pub ticker_mint_account: Account<'info, Mint>,

	/// Платежный токен
	#[account(constraint = payment_mint_account.key() == payload.payment_mint)]
	pub payment_mint_account: Account<'info, Mint>,

	/// АТА мейкера под токен (тикер), чтобы получить его после покупки.
	/// Создаётся (если не существует), при создании ордера, что бы рента была оплачена мейкером.
	#[account(
		init_if_needed,
		payer = payer,
		associated_token::mint = ticker_mint_account,
		associated_token::authority = payer,
	)]
	pub maker_ticker_account: Account<'info, TokenAccount>,

	/// АТА мейкера для платежного токена
    #[account(
        mut,
        constraint = maker_payment_account.owner == payer.key(),
        constraint = maker_payment_account.mint == payload.payment_mint,
    )]
    pub maker_payment_account: Account<'info, TokenAccount>,

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
		token::authority = order
    )]
    pub escrow_payment_account: Account<'info, TokenAccount>,

	/// PDA для блокировки тикера перед выполнением ордера на продажу
	//#[account(
	//	init_if_needed,
	//	payer = payer,
	//	seeds = [
	//		b"ticker_escrow",
	//		payer.key().as_ref(),
	//		payload.ticker_mint.as_ref()
	//	],
	//	bump,
	//	token::mint = ticker_mint_account,
	//	token::authority = order
	//)]
	//pub escrow_ticker_account: Account<'info, TokenAccount>,
	
    pub rent: Sysvar<'info, Rent>,

	/// CHECK: instruction sysvar, used for verifying oracle signature
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
	pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
}

pub fn create(
	ctx: Context<CreateBuyOrder>,
	payload: OrderPayload,
) -> Result<()> {
	// Payload не устарел
	let now = Clock::get()?.unix_timestamp;
	require!(now <= payload.expires_at, ErrorCode::PayloadExpired);
	
	// Проверка подписи от оракула
	let mut serialized = vec![];
	payload.serialize(&mut serialized)?;

	let hash = keccak::hash(&serialized);
	verify_ed25519_ix(&ctx.accounts.instruction_sysvar, &ctx.accounts.registry.authority, &hash.0)?;

	// Переводим средства мейкера в эскроу
	let total = payload.amount
		.checked_mul(payload.price).ok_or(ErrorCode::Overflow)?
		.checked_add(payload.fee).ok_or(ErrorCode::Overflow)?;
	
	let cpi_ctx = CpiContext::new(
		ctx.accounts.token_program.to_account_info(),
		token::Transfer {
			from: ctx.accounts.maker_payment_account.to_account_info(),
			to: ctx.accounts.escrow_payment_account.to_account_info(),
			authority: ctx.accounts.payer.to_account_info(),
		},
	);
	token::transfer(cpi_ctx, total)?;

	// Записываем данные в OrderState
	let order = &mut ctx.accounts.order;

	order.id = payload.id;

	order.maker = ctx.accounts.payer.key();
	order.status = OrderStatus::Pending;

	order.ticker_mint = payload.ticker_mint;
	order.amount = payload.amount;

	order.payment_mint = payload.payment_mint;
	order.price = payload.price;
	order.fee = payload.fee;

	order.expires_at = payload.expires_at;

	emit!(OrderCreated {
		id: payload.id,
		maker: order.maker,
		created_at: now,
		expires_at: payload.expires_at,
	});

	Ok(())
}