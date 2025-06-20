use anchor_lang::prelude::*;
use super::types::*;

#[account]
pub struct OrderState {
    pub id: u64,                // уникальный идентификатор заявки
    pub maker: Pubkey,          // адрес создателя ордера
    pub side: Side,             // направление (покупка/продажа)
    pub status: OrderStatus,    // текущий статус заявки
    pub created_at: i64,        // когда была создана
}

