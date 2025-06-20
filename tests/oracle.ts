import { PublicKey } from '@solana/web3.js'

import nacl from 'tweetnacl'
import bs58 from 'bs58'
import BN from 'bn.js'

import { SPLToken } from './spl.ts'

export const USDC = await SPLToken.create('USDC', 6)

type OrderSide = { buy: {} } | { sell: {} }
type OrderType = { market: {} } | { limit: {} }

type OraclePayload = {
	id : BN
	side : OrderSide
	orderType : OrderType
	symbol : number[]
	amount : BN
	price : BN
	fee : BN
	paymentMint : PublicKey
	expiresAt : BN | null
}

const TTL = 60 // 60 seconds
const fee = .1 // 10% fee

export class Oracle {
	#keypair = nacl.sign.keyPair()
	get signer () { 
		return new PublicKey(this.#keypair.publicKey)
	}

	order (symbol: string, side: 'buy' | 'sell', amount: number, price?: number) {
		const now = Math.floor(Date.now() / 1000)
		const orderType : OrderType = price ? { limit: {} } : { market: {} }

		price ??= Math.floor(Math.random() * 100 + 1)

		const payload = {
			id: new BN(now),
			side: { [side]: {} } as OrderSide,
			orderType,
			symbol: Array.from(Buffer.from(symbol.padEnd(8, '\0')).slice(0, 8)),
			amount: new BN(amount),
			price: new BN(price),
			fee: new BN(Math.floor(price * amount * fee)),
			paymentMint: USDC.mint,
			expiresAt: new BN(now + TTL)
		}

		console.log(payload)

		return this.sign(payload)
	}

	sign (payload : OraclePayload) {
		const { signer } = this
		const { secretKey } = this.#keypair

		const message = new TextEncoder().encode(JSON.stringify(payload))
		const sig = nacl.sign.detached(message, secretKey)

		return {
			payload, signer,
			signature: Array.from(sig)
		}
	}
}

export default new Oracle