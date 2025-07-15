use anchor_lang::prelude::*;
use super::types::*;

#[account]
pub struct OrderState {
    pub id: u64,                // уникальный идентификатор заявки

    pub maker: Pubkey,          // адрес создателя ордера
    pub status: OrderStatus,    // текущий статус заявки

    pub ticker_mint: Pubkey,     // адрес токена, который покупаем/продаем
    pub amount: u64,

    pub payment_mint: Pubkey,   // адрес токена для оплаты
	pub price: u64,
	pub fee: u64,

    pub expires_at: i64,        // когда истекает (если применимо)
}
