## Ticker Token (Anchor Program)

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

## Метаданные токена (name / symbol / image)
`createTicker(symbol, decimals)` создаёт только SPL mint. Имя токена, символ, описание и картинка хранятся отдельно в Metaplex metadata account.

Если совсем просто:
- `mint` — это адрес самого токена.
- `metadata` — отдельная запись в Solana, где лежит ссылка на JSON с `name`, `symbol`, `description`, `image`.
- клиент читает именно metadata, потом идёт по `metadata.uri` и забирает JSON из IPFS.

В текущем проекте metadata лучше создавать сразу после `createTicker(...)`. Без неё токен может не загрузиться нормально во фронте.

### Что нужно подготовить
1. В `oracle/.env` должны быть:
- `AUTHORITY_SOL_KEY`
- `SOLANA_RPC_ENDPOINT` или `SOLANA_CLUSTER`
- `IPFS_PIN_URL`
- `IPFS_PIN_JWT`

2. Важно не перепутать:
- использовать нужно тот же ключ, что стоит в `registry.authority`;
- `mint` — это адрес mint токена, а не ATA и не адрес кошелька;
- `symbol` в metadata лучше оставлять тем же, что использовался в `createTicker(symbol, ...)`;
- `imageUrl` должен быть публичным `https://...`;
- в примере ниже указан `image/png`, если картинка у вас `jpg` или `svg`, поменять `type`.

### Как добавить metadata
Из директории `oracle` выполнить:

```bash
deno eval -A --env=.env '
import bs58 from "npm:bs58"
import { Keypair } from "@solana/web3.js"
import { mplTokenMetadata, fetchDigitalAsset, createV1, updateV1, TokenStandard } from "@metaplex-foundation/mpl-token-metadata"
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults"
import { publicKey, keypairIdentity } from "@metaplex-foundation/umi"
import { connection } from "./src/helpers.ts"
import ipfs from "./src/ipfs.ts"

const mint = "<MINT>"
const name = "Apple Inc."
const symbol = "AAPL"
const description = "Apple Inc. stock token used in Q2."
const imageUrl = "https://example.com/aapl.png"

const imageCid = await ipfs.pin(new URL(imageUrl))
const metadataCid = await ipfs.pin({
	name,
	symbol,
	description,
	image: `ipfs://${imageCid}`,
	properties: {
		files: [
			{ uri: `ipfs://${imageCid}`, type: "image/png" },
			{ uri: `https://ipfs.io/ipfs/${imageCid}`, type: "image/png" }
		]
	}
})

const uri = `https://ipfs.io/ipfs/${metadataCid}`
const secret = Deno.env.get("AUTHORITY_SOL_KEY") || ""
const authority = Keypair.fromSecretKey(bs58.decode(secret))

const umi = createUmi(connection).use(mplTokenMetadata())
const signer = umi.eddsa.createKeypairFromSecretKey(authority.secretKey)
umi.use(keypairIdentity(signer))

const asset = await fetchDigitalAsset(umi, publicKey(mint)).catch(() => null)
const tx = asset?.metadata
	? await updateV1(umi, {
		mint: publicKey(mint),
		data: {
			name,
			symbol,
			uri,
			sellerFeeBasisPoints: 0,
			creators: asset.metadata.creators
		}
	})
	: await createV1(umi, {
		mint: publicKey(mint),
		authority: umi.identity,
		name,
		symbol,
		uri,
		sellerFeeBasisPoints: 0 as any,
		tokenStandard: TokenStandard.Fungible,
		isMutable: true
	})

const sig = await tx.send(umi, { maxRetries: 3 })
const latest = await umi.rpc.getLatestBlockhash()
await umi.rpc.confirmTransaction(sig, {
	strategy: { type: "blockhash", ...latest }
})

console.log("metadata uri:", uri)
console.log("tx:", sig)
'
```
