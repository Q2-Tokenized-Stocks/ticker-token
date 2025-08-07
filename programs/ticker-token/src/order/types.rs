use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Processing,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OrderPayload {
    pub id: u64,
    pub market: bool,

    pub ticker_mint: Pubkey,
    pub amount: u64,

    pub payment_mint: Pubkey,
    pub price: u64,
    pub fee: u64,

    pub expires_at: i64,
}