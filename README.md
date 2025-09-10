## Ticker Token (Anchor Program)

**Program ID**: `EjJFMSVeNQYjjJJkC3fic9pTHj9AcowTbEz7CcGFkXXk`

## Назначение
- Принимает заявки (ордера) на покупку/продажу «тикер»-токена.
- Оракл подписывает payload заявки (ed25519) и инициирует выполнение.
- При исполнении заявок происходит перевод/возврат средств, минт/берн тикер‑токена и пополнение/списание пула.

## PDAs и сущности
- `Registry` (PDA `['registry']`): хранит `authority` — публичный ключ оракула/админа. Используется для проверки подписи и прав на исполнение. (`programs/ticker-token/src/lib.rs`)
- `Mint(ticker)` (PDA `['mint', symbol]`): Mint тикер‑токена для символа `symbol`. (`programs/ticker-token/src/ticker.rs`)
- `Order` (PDA `['order', maker, id_le]`): состояние заявки. (`programs/ticker-token/src/order/state.rs`)
- `Escrow` (PDA `['escrow', order_pda]`): токенный счёт под залог средств/тикера, owner — `Order` PDA. (`programs/ticker-token/src/order/create.rs`)
- `Pool` (PDA `['pool', ticker_mint, payment_mint]`): пул платёжного токена для рынка данного тикера. Управляется `authority`. Создаётся при исполнении. (`programs/ticker-token/src/order/execute.rs`)

## События
- `TickerCreated { ticker }`
- `OrderCreated { id, maker, timestamp, expires_at }`
- `OrderProcessing { id, maker, timestamp }`
- `OrderExecuted { id, side, market, maker, ticker_mint, amount, payment_mint, price, fee, proof_cid, timestamp }`
- `OrderCanceled { id, maker, timestamp }`

## Инструкции

Все имена ниже — как в IDL (camelCase).

- `init()`
  - Цель: инициализация `Registry` и установка `authority = payer`.
  - Аккаунты: `payer (signer, mut)`, `registry (init, ['registry'])`, `system_program`.

- `transferAuthority(new_authority: Pubkey)`
  - Цель: смена `registry.authority`.
  - Аккаунты: `authority (signer == registry.authority)`, `registry (mut)`.

- `createTicker(symbol: string, decimals: u8)`
  - Цель: создать Mint тикер‑токена для `symbol`.
  - Аккаунты: `payer (signer == registry.authority)`, `registry`, `mint (init, ['mint', symbol])`, `rent`, `token_program`, `system_program`.
  - Событие: `TickerCreated`.

- `createBuyOrder(payload: OrderPayload)`
  - Цель: создать ордер «покупка», залочить платёжные токены в `Escrow`.
  - Аккаунты: `payer (signer)`, `registry`, `order (init, ['order', payer, id])`, `ticker_mint_account`, `payment_mint_account`, `maker_payment_account (ATA payer, payment_mint)`, `maker_ticker_account (init_if_needed ATA payer, ticker_mint)`, `escrow_account (init_if_needed ['escrow', order])`, `instruction_sysvar`, `system_program`, `token_program`, `associated_token_program`.
  - Требования: валидная ed25519‑подпись оракула (см. «Оракл»), не истёк `expires_at`.
  - Событие: `OrderCreated`.

- `createSellOrder(payload: OrderPayload)`
  - Цель: создать ордер «продажа», залочить тикер‑токены в `Escrow`.
  - Аккаунты: `payer (signer == payload.maker)`, `registry`, `order (init)`, `ticker_mint_account`, `payment_mint_account`, `maker_ticker_account (init_if_needed ATA payer, ticker_mint)`, `escrow_account (init_if_needed ['escrow', order])`, `instruction_sysvar`, `system_program`, `token_program`, `associated_token_program`.
  - Требования: валидная ed25519‑подпись оракула, не истёк `expires_at`.
  - Событие: `OrderCreated`.

- `processOrder()`
  - Цель: перевести ордер в статус `Processing`.
  - Аккаунты: `payer (signer == registry.authority)`, `registry`, `order (mut, Pending)`.
  - Событие: `OrderProcessing`.

- `executeOrder(order_id: u64, spent: u64, proof_cid: bytes)`
  - Цель: финальное исполнение (BUY: списать из Escrow → Pool, вернуть сдачу, заминтить тикер; SELL: выплатить из Pool, сжечь тикер из Escrow). Закрывает `Order` и `Escrow`.
  - Аккаунты: `payer (signer == registry.authority)`, `registry`, `order (mut, close=payer, Pending|Processing)`, `maker`, `maker_account (ATA maker)`, `refund_account (ATA maker, payment_mint)`, `escrow_account (['escrow', order], owner=order)`, `payment_mint`, `ticker_mint`, `pool (init_if_needed ['pool', ticker_mint, payment_mint])`, `instruction_sysvar`, `token_program`, `system_program`.
  - Событие: `OrderExecuted`.

- `cancelOrder(id: u64)`
  - Цель: отмена ордера автором. Возврат средств из `Escrow` и закрытие.
  - Аккаунты: `payer (signer == maker)`, `order (mut, ['order', payer, id], Pending)`, `escrow_account (['escrow', order], owner=order)`, `refund_account (ATA payer)`, `token_program`.
  - Событие: `OrderCanceled`.

### OrderPayload (подпись оракула)
- Поля: `id: u64`, `maker: Pubkey`, `market: bool`, `ticker_mint: Pubkey`, `amount: u64`, `payment_mint: Pubkey`, `price: u64`, `fee: u64`, `expires_at: i64`.
- Верификация: см. `programs/ticker-token/src/utils.rs` → `verify_ed25519_ix`.

## Оракл и подпись
- Ключ оракула: `registry.authority`.
- Проверка подписи: ed25519‑инструкция должна быть в TX перед вызовом `create*Order` и находиться в `instruction_sysvar` на индексе 0. Сообщение — `keccak256(serialized(payload))`.
- От имени `authority` разрешены: `processOrder`, `executeOrder`, выпуск/заморозка тикер‑mint’ов и управление `Pool`.

## Потоки
- Покупка (BUY): `createBuyOrder` → `processOrder` → `executeOrder(spent, proof_cid)`.
  - В `executeOrder`: `Escrow(payment)` → `Pool`; возврат сдачи → `refund_account`; минт тикера → `maker_account`.
- Продажа (SELL): `createSellOrder` → `processOrder` → `executeOrder(spent, proof_cid)`.
  - В `executeOrder`: перевод из `Pool(payment)` → `maker_account`; берн тикера из `Escrow`.
- Отмена: `cancelOrder` для `Pending` ордеров, полностью возвращает залог и закрывает PDA.

## Вызовы с клиента (готовая обёртка)
См. `lib/ticker-tocken.ts`.
- `await TickerToken.init()` — разовая инициализация `Registry`.
- `await TickerToken.createTicker(symbol, decimals?)` — создать тикер.
- `await TickerToken.connect(user).buy(payload, { message, signature })` — создать BUY.
- `await TickerToken.connect(user).sell(payload, { message, signature })` — создать SELL.
- `await TickerToken.process(maker, orderId)` — установить `Processing` (только `authority`).
- `await TickerToken.execute(maker, orderId, spent, proofCid)` — исполнить (только `authority`).
- `await TickerToken.connect(user).cancel(orderId)` — отменить `Pending` ордер.
- Вспомогательное: `TickerToken.order(maker, id)`, `TickerToken.balance(symbol, owner)`, `TickerToken.supply(symbol)`, `TickerToken.pda([...])`.

Минимальный порядок для `create*Order` в одном TX: сначала `Ed25519Program.createInstructionWithPublicKey(...)`, затем — инструкция `createBuyOrder`/`createSellOrder` (см. реализацию в `lib/ticker-tocken.ts`).

## Ошибки (основные)
См. `programs/ticker-token/src/errors.rs`:
- `Unauthorized` — неверный авторизованный подписант/владелец.
- `InvalidOracleSig`, `InvalidSignatureInstruction` — проблемы с подписью оракула.
- `PayloadExpired` — истек срок payload.
- `OrderAlreadyProcessed` — неверный статус ордера.
- `Invalid*`/`Insufficient*` — несовпадение минтов/владельцев/балансов и др.

## Замечания
- Минт тикера создаётся с `mint::authority = registry.authority` (админ/оракл).
- `Pool` создаётся при первом исполнении пары `(ticker_mint, payment_mint)` и принадлежит `authority`.
- При `executeOrder` `Order` и `Escrow` закрываются, лампорты возвращаются `maker`.