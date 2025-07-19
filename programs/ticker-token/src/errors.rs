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
    InvalidOrderSide,

    #[msg("Invalid order type")]
    InvalidOrderType,

    #[msg("Insufficient funds")]
    InsufficientFunds,

    #[msg("Order already processed")]
    OrderAlreadyProcessed,

    #[msg("Invalid escrow mint")]
    InvalidEscrowMint,

    #[msg("Invalid escrow owner")]
    InvalidEscrowOwner,

    #[msg("Invalid refund mint")]
    InvalidRefundMint,

    #[msg("Invalid refund owner")]
    InvalidRefundOwner,

    #[msg("Bump seed not found")]
    BumpNotFound,

    #[msg("Invalid maker mint")]
    InvalidMakerMint,

    #[msg("Invalid maker")]
    InvalidMaker,

    #[msg("Invalid maker account")]
    InvalidMakerAccount,

    #[msg("Invalid payment mint")]
    InvalidPaymentMint,

    #[msg("Invalid ticker mint")]
    InvalidTickerMint,

    #[msg("Invalid maker payment account")]
    InvalidMakerPaymentAccount,

    #[msg("Invalid sell amount")]
    InvalidSellAmount,
}