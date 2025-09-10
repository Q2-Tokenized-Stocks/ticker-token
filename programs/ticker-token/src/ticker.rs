use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, Mint},
};

use crate::{
    Registry, 
    errors::TickerError
};

#[event]
pub struct TickerCreated {
    pub ticker: String
}
// TODO: Metaplex support

#[derive(Accounts)]
#[instruction(ticker: String, decimals: u8)]
pub struct CreateTicker<'info> {
    #[account(
        mut,
        constraint = payer.key() == registry.authority @ TickerError::Unauthorized,
    )]
    pub payer: Signer<'info>,

    #[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

    /// CHECK: mint account is created in this instruction and its validity is ensured by context
    #[account(
        init,
        seeds = [b"mint", ticker.as_bytes()],
        bump,
        payer = payer,
        mint::decimals = decimals,
	    mint::authority = registry.authority,
	    mint::freeze_authority = registry.authority,
    )]
    pub mint: Account<'info, Mint>,

    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
