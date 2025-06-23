use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    sysvar::instructions::load_instruction_at_checked,
    ed25519_program::ID as ED25519_PROGRAM_ID,
};
use anchor_spl::associated_token::get_associated_token_address;

use crate::errors::ErrorCode;

pub fn verify_ed25519_ix(
    instruction_sysvar: &AccountInfo,
    expected_pubkey: &Pubkey,
    expected_msg: &[u8],
) -> Result<()> {
    let ix = load_instruction_at_checked(0, instruction_sysvar)?;
    require!(ix.program_id == ED25519_PROGRAM_ID, ErrorCode::InvalidSignatureInstruction);

    let data = &ix.data;

    // Формат ed25519-инструкции:
    // https://docs.solana.com/developing/runtime-facilities/programs#ed25519-program
    if data.len() < 1 + 1 + 2*6 + 64 + 32 {
        return Err(ErrorCode::InvalidSignatureInstruction.into());
    }

    // Извлекаем смещения
    let pubkey_offset = u16::from_le_bytes([data[6], data[7]]) as usize;
    let message_offset = u16::from_le_bytes([data[10], data[11]]) as usize;
    let message_size = u16::from_le_bytes([data[12], data[13]]) as usize;

    require!(data.len() >= message_offset + message_size, ErrorCode::InvalidSignatureInstruction);
    require!(data.len() >= pubkey_offset + 32, ErrorCode::InvalidSignatureInstruction);

    let pubkey_bytes = &data[pubkey_offset..pubkey_offset + 32];
    let message_bytes = &data[message_offset..message_offset + message_size];

    require!(pubkey_bytes == expected_pubkey.as_ref(), ErrorCode::InvalidOracleSig);
    require!(message_bytes == expected_msg, ErrorCode::InvalidOracleSig);

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