import nacl, { randomBytes } from 'tweetnacl'
import BN from 'bn.js'

import { PublicKey } from '@solana/web3.js'
import Ticker from './ticker-tocken.ts'

import { SPLToken } from './spl.ts'
import { ata, createUser, pda, randomString, splAccount } from './utils.ts'

type OrderSide = { buy: {} } | { sell: {} }
type OrderType = { market: {} } | { limit: {} }

export type OraclePayload = {
	id : BN
	side : OrderSide
	orderType : OrderType
	amount : BN
	price : BN
	fee : BN

	tokenMint : PublicKey
	paymentMint : PublicKey

	expiresAt : BN | null
}

const TTL = 60 // 60 seconds
const fee = .1 // 10% fee

const admin = Ticker.signer

export class Oracle {
	#keypair = nacl.sign.keyPair()
	get signer () { 
		return new PublicKey(this.#keypair.publicKey)
	}

	async order (symbol: string, side: 'buy' | 'sell', amount: number, price?: number) {
		const { signer } = this

		const orderType : OrderType = price ? { limit: {} } : { market: {} }
		const now = Math.floor(Date.now() / 1000)

		price ??= Math.floor(Math.random() * 100 + 1)

		const ticker = await Ticker.ticker(symbol)
		const paymentToken = await SPLToken.create(randomString())

		const [programOwner] = pda(['program_owner', ticker.mint.toBuffer(), paymentToken.mint.toBuffer()])
		
		// lp vault (ATA for liqidity provider, create it if not exists)
		await splAccount(
			await ata(paymentToken.mint, programOwner), 
			paymentToken.mint, admin, programOwner
		)

		const payload = {
			id: new BN(now),

			side: { [side]: {} } as OrderSide,
			orderType,

			tokenMint: ticker.mint,
			amount: new BN(amount),

			paymentMint: paymentToken.mint,
			price: new BN(price),
			fee: new BN(Math.floor(price * amount * fee)),
			
			expiresAt: new BN(now + TTL)
		}

		const signature = await this.sign(payload)
		return { payload, signature, _paymentToken: paymentToken }
	}

	sign (payload : OraclePayload) {
		const { secretKey } = this.#keypair

		const message = new TextEncoder().encode(JSON.stringify(payload))
		const sig = nacl.sign.detached(message, secretKey)

		return Array.from(sig)
	}
}

export default new Oracle