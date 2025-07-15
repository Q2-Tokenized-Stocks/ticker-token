use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    sysvar::instructions::load_instruction_at_checked,
    ed25519_program::ID as ED25519_PROGRAM_ID,
};
use anchor_spl::associated_token::get_associated_token_address;
//use anchor_spl::token_interface::TokenAccount;

use crate::errors::ErrorCode;

pub fn verify_ed25519_ix(
    instruction_sysvar: &AccountInfo,
    expected_pubkey: &Pubkey,
    expected_msg: &[u8],
) -> Result<()> {
    let ix = load_instruction_at_checked(0, instruction_sysvar)?;
    require!(ix.program_id == ED25519_PROGRAM_ID, ErrorCode::InvalidSignatureInstruction);

    let data = &ix.data;

    let pubkey_off = u16::from_le_bytes([data[6], data[7]]) as usize;
    let msg_off = u16::from_le_bytes([data[10], data[11]]) as usize;
    let msg_len = u16::from_le_bytes([data[12], data[13]]) as usize;

    let actual_pubkey = &data[pubkey_off..pubkey_off + 32];
    let actual_msg = &data[msg_off..msg_off + msg_len];

    require!(actual_pubkey == expected_pubkey.as_ref(), ErrorCode::InvalidOracleSig);
    require!(actual_msg == expected_msg, ErrorCode::InvalidOracleSig);

    Ok(())
}

pub fn assert_ata (
	account: Pubkey,
	owner: &Pubkey,
	mint: &Pubkey,
) -> Result<()> {
	let expected = get_associated_token_address(owner, mint);
	require!(account == expected, ErrorCode::InvalidATA);
	Ok(())
}

pub fn assert_pda (
	account: Pubkey,
	seeds: &[&[u8]],
) -> Result<()> {
	let (expected, _) = Pubkey::find_program_address(seeds, &crate::ID);
	require!(account == expected, ErrorCode::InvalidPDA);
	Ok(())
}

//pub fn assert_token_account(
//    acc: &TokenAccount,
//    expected_mint: &Pubkey,
//    expected_authority: &Pubkey,
//) -> Result<()> {
//    require_keys_eq!(acc.mint, *expected_mint, ErrorCode::InvalidMint);
//    require_keys_eq!(acc.owner, *expected_authority, ErrorCode::InvalidAuthority);
//    Ok(())
//}