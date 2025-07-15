use anchor_lang::prelude::*;
use anchor_lang::prelude::Pubkey;

pub mod utils;

mod errors;
use errors::{TickerError};

mod ticker;
pub use ticker::*;

mod order;
use order::*;

declare_id!("8mPWhPVTKG4zXp5JFqsxA5ZMNhUqWThz5MJjrQS4VB4Z");

#[account]
pub struct Registry {
    pub authority: Pubkey,
}

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        seeds = [b"registry"],
        bump,
        payer = payer,
        space = 8					// Anchor-дескриптор (дисриминатор, нужен всегда)
              + 32                  // authority: Pubkey
    )]
    pub registry: Account<'info, Registry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Authority<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,
}

#[program]
pub mod ticker_token {
    use super::*;

    pub fn init(ctx: Context<Init>) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.payer.key();

        Ok(())
    }

    pub fn transfer_authority(ctx: Context<Authority>, new_authority: Pubkey) -> Result<()> {
        require!(ctx.accounts.authority.key() == ctx.accounts.registry.authority, TickerError::Unauthorized);
        require!(new_authority != Pubkey::default(), TickerError::InvalidAuthority);

        ctx.accounts.registry.authority = new_authority;
        Ok(())
    }

    pub fn create_ticker(ctx: Context<CreateTicker>, symbol: String, decimals: u8) -> Result<()> {
        require!(ctx.accounts.payer.key() == ctx.accounts.registry.authority, TickerError::Unauthorized);
        msg!("Created ticker: {} with decimals: {}", symbol, decimals);

        Ok(())
    }

    pub fn ticker_metadata(ctx: Context<CreateMetadata>, name: String, symbol: String, uri: String) -> Result<()> {
        require!(ctx.accounts.authority.key() == ctx.accounts.registry.authority, TickerError::Unauthorized);
        ticker::metadata(ctx, name, symbol, uri)
    }

    //pub fn create_order(ctx: Context<CreateOrder>, payload: OraclePayload, sig: [u8; 64]) -> Result<()> {
    //    order_create(ctx, payload, sig)
    //}

    pub fn create_buy_order(ctx: Context<CreateBuyOrder>, payload: OrderPayload) -> Result<()> {
        order::buy::create(ctx, payload)
    }
}

