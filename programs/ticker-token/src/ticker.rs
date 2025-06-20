use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token},
    token_interface::{initialize_mint2, InitializeMint2},
};

use crate::{Registry, errors::TickerError};

#[account]
pub struct TickerData {
    pub symbol: [u8; 8],
	pub decimals: u8,
    pub mint: Pubkey,
}

#[derive(Accounts)]
#[instruction(ticker: String, decimals: u8)]
pub struct CreateTicker<'info> {
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

pub fn ticker_create(ctx: Context<CreateTicker>, symbol: String, decimals: u8) -> Result<()> {
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