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
    pub symbol: [u8; 8],
    pub amount: u64,
    pub price: u64,
    pub fee: u64,
    pub payment_mint: Pubkey,
    pub expires_at: Option<i64>,
    pub token_mint: Pubkey, // новое поле
}