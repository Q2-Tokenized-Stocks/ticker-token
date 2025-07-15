import BN from 'bn.js'
import { keccak_256 } from '@noble/hashes/sha3.js'

import { PublicKey } from '@solana/web3.js'
import { createKeyPairFromBytes, createPrivateKeyFromBytes, fixCodecSize, getBytesCodec, getStructCodec, getU64Codec, getU8Codec, signBytes } from '@solana/kit'

import { SPLToken } from './spl.ts'
import { pda, randomString } from './utils.ts'
import Ticker from './ticker-tocken.ts'

export type OraclePayload = {
	id : BN
	orderType : BN // 0 for market, 1 for limit

	tickerMint : PublicKey
	amount : BN

	paymentMint : PublicKey
	price : BN
	fee : BN

	expiresAt : BN
}

const payloadCodec = getStructCodec([
	['id', getU64Codec()],
	['orderType', getU8Codec()], // 0 for market, 1 for
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

	async order (symbol: string, side: 'buy' | 'sell', amount: number, price?: number) {
		const orderType = new BN(price ? 1 : 0) // 0 for market, 1 for limit
		const now = Math.floor(Date.now() / 1000)

		price ??= Math.floor(Math.random() * 100 + 1)

		const [tickerMint] = pda(['mint', symbol])
		const paymentToken = await SPLToken.create(randomString())

		const bnPrice = new BN(price)
		const bnAmount = new BN(amount)
		const bnFee = bnPrice.mul(bnAmount).muln(fee).divn(100)

		const payload = {
			id: new BN(now),
			orderType,

			tickerMint: tickerMint,
			amount: new BN(amount),

			paymentMint: paymentToken.mint,
			price: bnPrice,
			fee: bnFee,
			
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