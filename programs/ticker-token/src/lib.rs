use anchor_lang::prelude::*;
use anchor_lang::prelude::Pubkey;

pub mod utils;

mod errors;
use errors::{TickerError};

mod ticker;
pub use ticker::*;

mod order;
use order::*;

declare_id!("EjJFMSVeNQYjjJJkC3fic9pTHj9AcowTbEz7CcGFkXXk");

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
    #[account(
        mut,
        constraint = authority.key() == registry.authority @ TickerError::Unauthorized,
    )]
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
        require!(new_authority != Pubkey::default(), TickerError::InvalidAuthority);

        ctx.accounts.registry.authority = new_authority;
        Ok(())
    }

    pub fn create_ticker(_ctx: Context<CreateTicker>, symbol: String, decimals: u8) -> Result<()> {
        emit!(TickerCreated { ticker: symbol.clone() });
        Ok(())
    }

    pub fn create_buy_order(ctx: Context<CreateBuyOrder>, payload: OrderPayload) -> Result<()> {
        order::create::buy(ctx, payload)
    }

    pub fn create_sell_order(ctx: Context<CreateSellOrder>, payload: OrderPayload) -> Result<()> {
        order::create::sell(ctx, payload)
    }

    pub fn cancel_order(ctx: Context<CancelOrder>, id: u64) -> Result<()> {
        order::cancel(ctx)
    }

    pub fn process_order(ctx: Context<ProcessOrder>) -> Result<()> {
        order::process(ctx)
    }

    pub fn execute_order(ctx: Context<ExecuteOrder>, order_id: u64, spent: u64, proof_cid: Vec<u8>) -> Result<()> {
        order::execute(ctx, spent, proof_cid)
    }
}
