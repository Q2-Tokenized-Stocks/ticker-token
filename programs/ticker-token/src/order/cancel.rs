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
	#[account(mut)]
    pub payer: Signer<'info>,

	#[account(
		mut,
        seeds = [b"order", payer.key().as_ref(), &id.to_le_bytes()],
        bump,
		constraint = order.maker == payer.key() @ ErrorCode::Unauthorized, 
		close = payer
    )]
    pub order: Account<'info, Order>,

	#[account(
		mut,
		seeds = [b"escrow", order.key().as_ref()],
		bump,
		constraint = escrow_account.mint == order.payment_mint @ ErrorCode::InvalidEscrowMint,
		constraint = escrow_account.owner == order.key() @ ErrorCode::InvalidEscrowOwner
	)]
	pub escrow_account: Account<'info, TokenAccount>,

	#[account(
		mut,
		constraint = refund_account.mint == order.payment_mint @ ErrorCode::InvalidRefundMint,
		constraint = refund_account.owner == payer.key() @ ErrorCode::InvalidRefundOwner
	)]
	pub refund_account: Account<'info, TokenAccount>,

	pub token_program: Program<'info, Token>,
}

pub fn cancel(ctx: Context<CancelOrder>, id: u64) -> Result<()> {
	let order = &ctx.accounts.order;
	let payer = &ctx.accounts.payer;

	require!(order.status == OrderStatus::Pending, ErrorCode::OrderAlreadyProcessed);
	
	let escrow = &ctx.accounts.escrow_account;
	let refund = &ctx.accounts.refund_account;
	let refund_amount = match order.side {
		OrderSide::Buy => {
			let total = order.amount
				.checked_mul(order.price)
				.ok_or(ErrorCode::Overflow)?;
			total.checked_add(order.fee).ok_or(ErrorCode::Overflow)?
		}
		OrderSide::Sell => order.amount,
	};

	let signer_seeds: [&[u8]; 4] = [
		b"order",
		payer.key.as_ref(),
		&id.to_le_bytes(),
		&[ctx.bumps.order],
	];
	let signer: &[&[&[u8]]] = &[&signer_seeds];

	let cpi_ctx = CpiContext::new_with_signer(
		ctx.accounts.token_program.to_account_info(),
		token::Transfer {
			from: escrow.to_account_info(),
			to: refund.to_account_info(),
			authority: order.to_account_info(),
		},
		signer,
	);
	token::transfer(cpi_ctx, refund_amount)?;

	let cpi_ctx_close = CpiContext::new_with_signer(
		ctx.accounts.token_program.to_account_info(),
		token::CloseAccount {
			account: escrow.to_account_info(),
			destination: payer.to_account_info(),
			authority: order.to_account_info(),
		},
		signer,
	);
	token::close_account(cpi_ctx_close)?;

	emit!(OrderCancelled {
		id: order.id,
		maker: order.maker,
		cancelled_at: Clock::get()?.unix_timestamp,
	});

	Ok(())
}
