use anchor_lang::prelude::*;
use anchor_spl::{
    token::{Token, Mint},
};

use mpl_token_metadata::instructions::CreateMetadataAccountV3CpiBuilder;
use mpl_token_metadata::types::DataV2;
use mpl_token_metadata::ID as METAPLEX_PROGRAM_ID;

use crate::{
    Registry, 
    errors::TickerError
};

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

#[derive(Accounts)]
pub struct CreateMetadata<'info> {
     #[account(
        mut,
        constraint = authority.key() == registry.authority @ TickerError::Unauthorized,
    )]
    pub authority: Signer<'info>,

    #[account(seeds = [b"registry"], bump)]
    pub registry: Account<'info, Registry>,

    /// CHECK: Metaplex PDA derived 
    #[account(mut)]
    pub metadata: UncheckedAccount<'info>,

    /// CHECK: mint account
    pub mint: Account<'info, Mint>,

    /// CHECK: Metaplex Token Metadata
    #[account(address = METAPLEX_PROGRAM_ID)]
    pub token_metadata_program: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn metadata(ctx: Context<CreateMetadata>, name: String, symbol: String, uri: String) -> Result<()> {
    let accounts = &ctx.accounts;

    let (metadata_pda, _bump) = Pubkey::find_program_address(
        &[b"metadata", METAPLEX_PROGRAM_ID.as_ref(), accounts.mint.key().as_ref()],
        &METAPLEX_PROGRAM_ID,
    );
    require_keys_eq!(metadata_pda, accounts.metadata.key(), TickerError::InvalidMetadataPda);

    let data = DataV2 {
        name: name.to_string(),
        symbol: symbol.to_string(),
        uri: uri.to_string(),
        seller_fee_basis_points: 0,
        creators: None,
        collection: None,
        uses: None,
    };

    let token_metadata_program = accounts.token_metadata_program.to_account_info();
    let metadata = accounts.metadata.to_account_info();
    let mint = accounts.mint.to_account_info();
    let mint_authority = accounts.authority.to_account_info();
    let payer = accounts.authority.to_account_info();
    let update_authority = accounts.authority.to_account_info();
    let system_program = accounts.system_program.to_account_info();
    let rent = accounts.rent.to_account_info();

    let mut builder = CreateMetadataAccountV3CpiBuilder::new(&token_metadata_program);

    builder
        .metadata(&metadata)
        .mint(&mint)
        .mint_authority(&mint_authority)
        .payer(&payer)
        .update_authority(&update_authority, true)
        .system_program(&system_program)
        .rent(Some(&rent))
        .data(data)
        .is_mutable(true);

    builder.invoke()?;
    Ok(())
}