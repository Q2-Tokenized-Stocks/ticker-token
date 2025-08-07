use anchor_lang::{prelude::*};
use anchor_spl::{
	token::{self, TokenAccount, Token, Mint},
};
use crate::{
	Registry,
	errors::ErrorCode,
	order::{types::*, state::*, cancel::close},
};

#[derive(Accounts)]
pub struct ProcessOrder<'info> {
	#[account(
		mut,
		constraint = payer.key() == registry.authority @ ErrorCode::Unauthorized,
	)]
    pub payer: Signer<'info>,

    #[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

	#[account(
		mut,
        seeds = [b"order", order.maker.key().as_ref(), &order.id.to_le_bytes()],
        bump,
		constraint = order.status == OrderStatus::Pending @ ErrorCode::OrderAlreadyProcessed,
    )]
    pub order: Account<'info, Order>,
}

#[derive(Accounts)]
#[instruction(order_id: u64)]
pub struct ExecuteOrder<'info> {
	#[account(
		mut,
		constraint = payer.key() == registry.authority @ ErrorCode::Unauthorized,
	)]
    pub payer: Signer<'info>,

    #[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

	#[account(
		mut,
		seeds = [b"order", maker.key().as_ref(), &order_id.to_le_bytes()],
		bump,
		constraint = order.status == OrderStatus::Processing @ ErrorCode::OrderAlreadyProcessed,
		close = payer,
	)]
	pub order: Account<'info, Order>,

	/// CHECK: checked manually via constraint order.maker == maker.key()
	#[account(
		mut,
		constraint = order.maker == maker.key() @ ErrorCode::InvalidMaker,
	)]
	pub maker: AccountInfo<'info>, // Мейкер (куда выводить лампорты после закрытия ордера)

	#[account(
		mut,
		constraint = maker_account.owner == maker.key() @ ErrorCode::InvalidMakerAccount,
	)]
	pub maker_account: Account<'info, TokenAccount>, // Куда переводить токены (платежный или тиккер)

	#[account(
		mut,
        seeds = [b"escrow", order.key().as_ref()],
        bump,
		constraint = escrow_account.owner == order.key() @ ErrorCode::InvalidEscrowOwner,
    )]
    pub escrow_account: Account<'info, TokenAccount>,

	#[account(constraint = order.payment_mint.key() == payment_mint.key() @ ErrorCode::InvalidPaymentMint)]
	pub payment_mint: Account<'info, Mint>,

	#[account(
		mut,
		constraint = order.ticker_mint.key() == ticker_mint.key() @ ErrorCode::InvalidTickerMint
	)]
	pub ticker_mint: Account<'info, Mint>,

	#[account(
    	init_if_needed,
    	payer = payer,
    	seeds = [b"pool", ticker_mint.key().as_ref(), payment_mint.key().as_ref()],
    	bump,
    	token::mint = payment_mint,
    	token::authority = payer,
	)]
	pub pool: Account<'info, TokenAccount>,

	/// CHECK: instruction sysvar, used for verifying oracle signature
	#[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub instruction_sysvar: AccountInfo<'info>,
	pub token_program: Program<'info, Token>,
	pub system_program: Program<'info, System>,
}

pub fn process(ctx: Context<ProcessOrder>) -> Result<()> {
	ctx.accounts.order.status = OrderStatus::Processing;

	emit!(OrderProcessing {
		id: ctx.accounts.order.id,
		maker: ctx.accounts.order.maker,
		timestamp: Clock::get()?.unix_timestamp,
	});

	Ok(())
}

pub fn execute<'info>(ctx: Context<ExecuteOrder>, proof_cid: [u8; 32]) -> Result<()> {
	let order = &ctx.accounts.order;
	let signer_seeds: [&[u8]; 4] = [
		b"order",
		order.maker.as_ref(),
		&order.id.to_le_bytes(),
		&[ctx.bumps.order],
	];
	let signer: &[&[&[u8]]] = &[&signer_seeds];
	match order.side {
		OrderSide::Buy => {
			let amount = order.amount
				.checked_mul(order.price).ok_or(ErrorCode::Overflow)?
				.checked_add(order.fee).ok_or(ErrorCode::Overflow)?;

			require!(ctx.accounts.escrow_account.mint == order.payment_mint, ErrorCode::InvalidEscrowMint);
			let cpi_ctx = CpiContext::new_with_signer(
				ctx.accounts.token_program.to_account_info(),
				token::Transfer {
					from: ctx.accounts.escrow_account.to_account_info(),
					to: ctx.accounts.pool.to_account_info(),
					authority: order.to_account_info(),
				},
				signer,
			);
			token::transfer(cpi_ctx, amount)?;

			require!(ctx.accounts.maker_account.mint == order.ticker_mint, ErrorCode::InvalidMakerMint);
			let cpi_ctx_mint = CpiContext::new(
				ctx.accounts.token_program.to_account_info(),
				token::MintTo {
					mint: ctx.accounts.ticker_mint.to_account_info(),
					to: ctx.accounts.maker_account.to_account_info(),
					authority: ctx.accounts.payer.to_account_info()
				},
			);
			token::mint_to(cpi_ctx_mint, order.amount)?;
		}
		OrderSide::Sell => {
			let amount = order.amount
				.checked_mul(order.price).ok_or(ErrorCode::Overflow)?
				.checked_sub(order.fee).ok_or(ErrorCode::Overflow)?;

			require!(amount > 0, ErrorCode::InvalidSellAmount);
			require!(ctx.accounts.maker_account.mint == order.payment_mint, ErrorCode::InvalidMakerMint);
			require!(ctx.accounts.escrow_account.mint == order.ticker_mint, ErrorCode::InvalidEscrowMint);
			
			// перевод токенов из пулла на аккаунт мейкера
			let cpi_ctx = CpiContext::new(
				ctx.accounts.token_program.to_account_info(),
				token::Transfer {
					from: ctx.accounts.pool.to_account_info(),
					to: ctx.accounts.maker_account.to_account_info(),
					authority: ctx.accounts.payer.to_account_info(),
				},
			);
			token::transfer(cpi_ctx, amount)?;

			// сжигаем токены тикера из эскроу-аккаунта
			let cpi_ctx_burn = CpiContext::new_with_signer(
				ctx.accounts.token_program.to_account_info(),
				token::Burn {
					mint: ctx.accounts.ticker_mint.to_account_info(),
					from: ctx.accounts.escrow_account.to_account_info(),
					authority: ctx.accounts.order.to_account_info(),
				},
				signer
			);
			token::burn(cpi_ctx_burn, order.amount)?;
		}
	}

	// закрываем ПДАшки
	close(
		&ctx.accounts.order,
		&ctx.accounts.maker, // лампорты юзеру
		&ctx.accounts.escrow_account,
		&ctx.accounts.token_program,
		ctx.bumps.order,
	)?;

	emit!(OrderExecuted {
		id: order.id,

		side: order.side,
		market: order.market,
		maker: order.maker,

		ticker_mint: order.ticker_mint,
		amount: order.amount,

		payment_mint: order.payment_mint,
		price: order.price,
		fee: order.fee,

		proof_cid,

		timestamp: Clock::get()?.unix_timestamp,
	});

	Ok(())
}