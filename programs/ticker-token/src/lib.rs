use anchor_lang::prelude::*;
use anchor_lang::prelude::Pubkey;
use anchor_spl::token_interface::{initialize_mint2, InitializeMint2};
use anchor_spl::token::Token;

declare_id!("8mPWhPVTKG4zXp5JFqsxA5ZMNhUqWThz5MJjrQS4VB4Z");

const MAX_ADMINS: usize = 100;

#[account]
pub struct Registry {
    pub super_admin: Pubkey,
    pub admins: Vec<Pubkey>,
}

#[account]
pub struct TickerData {
    pub ticker: [u8; 8],
	pub decimals: u8,
    pub mint: Pubkey,
}

#[derive(Accounts)]
pub struct InitContext<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        seeds = [b"registry"],
        bump,
        payer = payer,
        space = 8					// Anchor-дескриптор (дисриминатор, нужен всегда)
              + 32                  // super_admin: Pubkey
              + 4                   // длина вектора admins (u32, 4 байта)
              + 32 * MAX_ADMINS     // каждый админ по 32 байта
    )]
    pub registry: Account<'info, Registry>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdminContext<'info> {
    #[account(mut, signer)]
    pub caller: Signer<'info>,

    #[account(mut, seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,
}

#[derive(Accounts)]
#[instruction(ticker: String, decimals: u8)]
pub struct TickerContext<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    #[account(
        seeds = [b"registry"],
        bump,
        constraint = registry.admins.contains(&caller.key()) @ TickerError::Unauthorized
    )]
    pub registry: Account<'info, Registry>,

    #[account(
        init,
        seeds = [b"ticker", ticker.as_bytes()],
        bump,
        payer = caller,
        space = 8 	// anchor header
			  + 8 	// ticker
			  + 32 	// mint
			  + 1 	// decimals
    )]
    pub ticker_data: Account<'info, TickerData>,

    /// CHECK: manually handled mint account
    #[account(mut)]
    pub mint: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[error_code]
pub enum TickerError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Ticker name too long")]
    TickerTooLong,
    #[msg("Too many admins")]
    TooManyAdmins,
	#[msg("Admin not found")]
	AdminNotFound,
}

#[program]
pub mod ticker_token {
    use super::*;

    pub fn init(
        ctx: Context<InitContext>
    ) -> Result<()> {
        let admin = ctx.accounts.payer.key();
        let registry = &mut ctx.accounts.registry;

        registry.super_admin = ctx.accounts.payer.key();
        registry.admins = vec![admin];

        Ok(())
    }

    pub fn add_admin(ctx: Context<AdminContext>, new_admin: Pubkey) -> Result<()> {
        let registry = &mut ctx.accounts.registry;

        assert_super_admin(registry, &ctx.accounts.caller.key())?;

        require!(registry.admins.len() < MAX_ADMINS, TickerError::TooManyAdmins);
        registry.admins.push(new_admin);

        Ok(())
    }

	pub fn remove_admin(ctx: Context<AdminContext>, target: Pubkey) -> Result<()> {
		let registry = &mut ctx.accounts.registry;

		assert_super_admin(registry, &ctx.accounts.caller.key())?;

		let index = registry.admins.iter().position(|a| *a == target)
			.ok_or(TickerError::AdminNotFound)?;

		registry.admins.remove(index);
		Ok(())
	}

    pub fn create_ticker(ctx: Context<TickerContext>, ticker: String, decimals: u8) -> Result<()> {
        require!(ticker.len() <= 8, TickerError::TickerTooLong);

        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            InitializeMint2 {
                mint: ctx.accounts.mint.to_account_info(),
            },
        );

        initialize_mint2(
            cpi_ctx,
            decimals,
            &ctx.accounts.caller.key(),
            Some(&ctx.accounts.caller.key()),
        )?;

        let mut fixed_ticker = [0u8; 8];
        fixed_ticker[..ticker.len()].copy_from_slice(ticker.as_bytes());

        let data = &mut ctx.accounts.ticker_data;

        data.ticker = fixed_ticker;
        data.mint = ctx.accounts.mint.key();
		data.decimals = decimals;

        Ok(())
    }
}

fn assert_super_admin(registry: &Registry, caller: &Pubkey) -> Result<()> {
    require!(
        *caller == registry.super_admin,
        TickerError::Unauthorized
    );

    Ok(())
}