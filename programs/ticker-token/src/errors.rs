use anchor_lang::prelude::*;

#[error_code]
pub enum TickerError {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Ticker name too long")]
    TickerTooLong,
    #[msg("New authority must not be zero")]
    InvalidAuthority,
    #[msg("Invalid metadata PDA")]
    InvalidMetadataPda,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Oracle public key is not set in registry")]
    InvalidOracle,

    #[msg("Invalid oracle signature")]
    InvalidOracleSig,

    #[msg("Payload has expired")]
    PayloadExpired,

    #[msg("Math overflow")]
    Overflow,

    #[msg("Invalid payer token account")]
    InvalidUserTokenAccount,

    #[msg("Vault PDA mismatch or not found")]
    InvalidVaultAccount,

    #[msg("Escrow PDA mismatch or not found")]
    InvalidEscrowAccount,

    #[msg("Unauthorized attempt to access this resource")]
    Unauthorized,

    #[msg("Order with same ID already exists")]
    DuplicateOrderId,

    #[msg("Invalid signature instruction (not ED25519 program)")]
    InvalidSignatureInstruction,

    #[msg("Invalid PDA account")]
    InvalidPDA,

    #[msg("Invalid ATA account")]
    InvalidATA,

    #[msg("Invalid side")]
    InvalidSide,

    #[msg("Invalid order type")]
    InvalidOrderType,
}