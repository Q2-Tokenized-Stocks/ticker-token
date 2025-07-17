import BN from 'bn.js'
import { keccak_256 } from '@noble/hashes/sha3.js'

import { PublicKey } from '@solana/web3.js'
import { createKeyPairFromBytes, fixCodecSize, getBytesCodec, getU8Codec, getStructCodec, getU64Codec, signBytes } from '@solana/kit'

import { SPLToken } from './spl.ts'
import { pda, randomString } from './utils.ts'
import Ticker from './ticker-tocken.ts'

enum OrderType { Market, Limit }
export enum OrderSide { Buy, Sell }

export type OraclePayload = {
	id : BN

	//orderType : OrderType
	//orderSide : OrderSide

	tickerMint : PublicKey
	amount : BN

	paymentMint : PublicKey
	price : BN
	fee : BN

	expiresAt : BN
}

const payloadCodec = getStructCodec([
	['id', getU64Codec()],
	['tickerMint', fixCodecSize(getBytesCodec(), 32)],
	['amount', getU64Codec()],
	['paymentMint', fixCodecSize(getBytesCodec(), 32)],
	['price', getU64Codec()],
	['fee', getU64Codec()],
	['expiresAt', getU64Codec()]
])

const TTL = 60 // 60 seconds
const fee = 10 // 10% fee

export class Oracle {
	#secretKey = Ticker.signer.secretKey
	get secretKey () { return this.#secretKey }

	constructor (secretKey = Ticker.signer.secretKey) {
		this.#secretKey = secretKey
	}

	async order (symbol: string, orderSide: OrderSide, amount: number, price?: number) {
		//const orderType = price ? OrderType.Limit : OrderType.Market
		const now = Math.floor(Date.now() / 1000)

		price ??= Math.floor(Math.random() * 100 + 1)

		const [tickerMint] = pda(['mint', symbol])
		const paymentToken = await SPLToken.create(randomString())

		const bnPrice = new BN(price)
		const bnAmount = new BN(amount)
		const bnFee = bnPrice.mul(bnAmount).muln(fee).divn(100)

		const payload = {
			id: new BN(now),
			//orderType,
			//orderSide,

			tickerMint: tickerMint,
			amount: new BN(amount) as BN,

			paymentMint: paymentToken.mint,
			price: bnPrice as BN,
			fee: bnFee as BN,
			
			expiresAt: new BN(now + TTL)
		}

		const { signature, message } = await this.sign(payload)
		return { payload, message, signature, _paymentToken: paymentToken }
	}

	async sign (payload : OraclePayload) {
		const { secretKey } = this
		const { privateKey } = await createKeyPairFromBytes(secretKey)

		const encoded = payloadCodec.encode({
			...payload,
			tickerMint: payload.tickerMint.toBytes(),
			paymentMint: payload.paymentMint.toBytes()
		})

		const message = keccak_256(new Uint8Array(encoded))
		const signature = await signBytes(privateKey, message)

		return { signature: Array.from(signature), message }
	}
}

export default new Oracle