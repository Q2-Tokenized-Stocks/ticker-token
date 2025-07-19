use anchor_lang::{prelude::*};
use anchor_spl::{
	token::{self, TokenAccount, Token}
};
use crate::{
	errors::ErrorCode,
	order::{types::*, state::*},
};

#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CancelOrder<'info> {
	#[account(
		mut,
		constraint = payer.key() == order.maker @ ErrorCode::Unauthorized,
	)]
    pub payer: Signer<'info>,

	#[account(
		mut,
        seeds = [b"order", payer.key().as_ref(), &id.to_le_bytes()],
        bump,
		constraint = order.maker == payer.key() @ ErrorCode::Unauthorized, 
    )]
    pub order: Account<'info, Order>,

	#[account(
		mut,
		seeds = [b"escrow", order.key().as_ref()],
		bump,
		constraint = escrow_account.owner == order.key() @ ErrorCode::InvalidEscrowOwner
	)]
	pub escrow_account: Account<'info, TokenAccount>,

	#[account(
		mut,
		constraint = refund_account.owner == payer.key() @ ErrorCode::InvalidRefundOwner
	)]
	pub refund_account: Account<'info, TokenAccount>,

	pub token_program: Program<'info, Token>,
}

pub fn cancel(ctx: Context<CancelOrder>) -> Result<()> {
	let order = &ctx.accounts.order;
	require!(order.status == OrderStatus::Pending, ErrorCode::OrderAlreadyProcessed);

	refund(
		order,
		&ctx.accounts.escrow_account,
		&ctx.accounts.refund_account,
		&ctx.accounts.token_program,
		ctx.bumps.order,
	)?;

	close(
		order,
		&ctx.accounts.payer.to_account_info(),
		&ctx.accounts.escrow_account,
		&ctx.accounts.token_program,
		ctx.bumps.order,
	)?;

	emit!(OrderCancelled {
		id: order.id,
		maker: order.maker,
		timestamp: Clock::get()?.unix_timestamp,
	});

	Ok(())
}

pub fn close<'info>(
	order: &Account<'info, Order>,
	maker_account: &AccountInfo<'info>,
	escrow_account: &Account<'info, TokenAccount>,
	token_program: &Program<'info, Token>,
	order_bump: u8,
) -> Result<()> {
	let signer_seeds: [&[u8]; 4] = [
		b"order",
		order.maker.as_ref(),
		&order.id.to_le_bytes(),
		&[order_bump],
	];
	let signer: &[&[&[u8]]] = &[&signer_seeds];

	let cpi_ctx_close = CpiContext::new_with_signer(
		token_program.to_account_info(),
		token::CloseAccount {
			account: escrow_account.to_account_info(),
			destination: maker_account.to_account_info(),
			authority: order.to_account_info(),
		},
		signer,
	);
	token::close_account(cpi_ctx_close)?;

	order.close(maker_account.to_account_info())
}

pub fn refund<'info>(
	order: &Account<'info, Order>,
	escrow_account: &Account<'info, TokenAccount>,
	refund_account: &Account<'info, TokenAccount>,
	token_program: &Program<'info, Token>,
	order_bump: u8,
) -> Result<()> {
	let refund_amount: u64;

	match order.side {
		OrderSide::Buy => {
			refund_amount = order.amount
				.checked_mul(order.price).ok_or(ErrorCode::Overflow)?
				.checked_add(order.fee).ok_or(ErrorCode::Overflow)?;

			require!(escrow_account.mint == order.payment_mint, ErrorCode::InvalidEscrowMint);
			require!(refund_account.mint == order.payment_mint, ErrorCode::InvalidRefundMint);
		},
		OrderSide::Sell => {
			refund_amount = order.amount;
			require!(escrow_account.mint == order.ticker_mint, ErrorCode::InvalidEscrowMint);
			require!(refund_account.mint == order.ticker_mint, ErrorCode::InvalidRefundMint);
		},
	}

	let signer_seeds: [&[u8]; 4] = [
		b"order",
		order.maker.as_ref(),
		&order.id.to_le_bytes(),
		&[order_bump],
	];
	let signer: &[&[&[u8]]] = &[&signer_seeds];

	let cpi_ctx = CpiContext::new_with_signer(
		token_program.to_account_info(),
		token::Transfer {
			from: escrow_account.to_account_info(),
			to: refund_account.to_account_info(),
			authority: order.to_account_info(),
		},
		signer,
	);

	token::transfer(cpi_ctx, refund_amount)
}