use anchor_lang::prelude::*;
use super::types::*;

#[event]
pub struct OrderCreated {
    pub id: u64,
    pub maker: Pubkey,

    pub created_at: i64,
    pub expires_at: i64,
}

#[account]
pub struct Order {
    pub id: u64, // уникальный идентификатор заявки
    
    pub side: OrderSide, // сторона заявки (Buy, Sell)
    
    pub maker: Pubkey, // адрес создателя ордера
    
    pub ticker_mint: Pubkey, // адрес тиккер токена
    pub amount: u64,
    
    pub payment_mint: Pubkey, // адрес токена для оплаты
	pub price: u64,
	pub fee: u64,
    
    pub status: OrderStatus, // текущий статус заявки
    pub expires_at: i64, 
}
