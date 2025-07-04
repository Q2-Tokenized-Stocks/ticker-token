use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Pending,
    Processing,
    Done,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct OraclePayload {
    pub id: u64,

    pub side: Side,
    pub order_type: OrderType,

    pub token_mint: Pubkey,
    pub amount: u64,

    pub payment_mint: Pubkey,
    pub price: u64,
    pub fee: u64,

    pub expires_at: i64,
}