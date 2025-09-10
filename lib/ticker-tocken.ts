import * as anchor from '@coral-xyz/anchor'
import BN from 'bn.js'

import { Keypair, PublicKey, Ed25519Program } from '@solana/web3.js'

import { TickerToken } from '~/target/types/ticker_token'
import IDL from '../target/idl/ticker_token.json' with { type: 'json' }

import { pda, ata } from './utils.ts'
import type { OraclePayload } from './oracle.ts'

export const METAPLEX_PROGRAM_ID = new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s")

anchor.setProvider(anchor.AnchorProvider.env())

export class Ticker {
	get owner () {
		return this.provider.wallet.payer
	}

	get idl () { return IDL as anchor.Idl }
	
	#provider = anchor.getProvider()
	get provider () { return this.#provider }

	#program = anchor.workspace.ticker_token as anchor.Program<TickerToken>
	get program () { return this.#program }

	#signer = null
	get signer () { return (this.#signer || this.#provider.wallet.payer) as Keypair }

	#registryPDA = null
	get registry () {
		return this.#program.account.registry.fetch(this.#registryPDA)
	}

	connect (signer = null) : Ticker { return new Ticker(signer || Keypair.generate()) }

	constructor (signer = null) {
		this.#signer = signer
		this.#registryPDA = this.pda(['registry'])
	}

	pda (seeds : any[]) {
		return pda(seeds, this.#program.programId)[0]
	}

	async supply (ticker : string) {
		const mint = this.pda(['mint', ticker])
		const account = await this.provider.connection.getTokenSupply(mint)
		return BigInt(account.value.amount)
	}

	async balance (ticker : string, address : PublicKey) {
		const mint = this.pda(['mint', ticker])
		const account = await this.provider.connection.getTokenAccountsByOwner(
			address, { mint }
		)
		const balance = await this.provider.connection.getTokenAccountBalance(
			account.value[0].pubkey
		)

		return BigInt(balance.value.amount)
	}

	async order (maker : PublicKey, id : number) {
		const orderPda = this.pda(
			['order', maker.toBuffer(), new BN(id).toArrayLike(Buffer, 'le', 8)]
		)
		return this.#program.account.order.fetch(orderPda)
	}

	async init () {
		const { signer } = this

		if (await this.#program.account.registry.fetchNullable(this.#registryPDA))
			return this

		await this.#program.methods
			.init()
			.accounts({ payer: signer.publicKey })
			.rpc()

		return this
	}

	transferAuthority (authority : PublicKey) {
		const { signer } = this

		return this.#program.methods
			.transferAuthority(authority)
			.accounts({ authority: signer.publicKey })
			.signers([signer])
			.rpc()
	}

	createTicker (symbol : string, decimals = 0) {
		const { signer } = this
		const mint = this.pda(['mint', symbol])

		return this.#program.methods
			.createTicker(symbol, decimals)
			// @ts-ignore
			.accounts({ mint })
			.signers([signer]).rpc()
	}

	async buy (payload : OraclePayload, { message, signature }) {
		const { signer } = this
		
		const { authority } = await this.registry
		const makerPaymentAccount = await ata(payload.paymentMint, signer.publicKey)

		const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
			publicKey: authority.toBytes(),
			message,
			signature: Buffer.from(signature)
		})

		const createOrder = await this.#program.methods
      		.createBuyOrder(payload)
			.accounts({
				payer: signer.publicKey,
				
				tickerMintAccount: payload.tickerMint,
				paymentMintAccount: payload.paymentMint,
				makerPaymentAccount
			})
			.transaction()

		const tx = new anchor.web3.Transaction()
			.add(ed25519Ix)
			.add(createOrder)

		const txid = await this.#provider.sendAndConfirm(tx, [signer], {
			commitment: 'confirmed'
		})

		return this.provider.connection.getTransaction(txid, {
			commitment: 'confirmed',
			maxSupportedTransactionVersion: 0
		})
	}

	async sell (payload : OraclePayload, { message, signature }) {
		const { signer } = this
		const { authority } = await this.registry
		const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
			publicKey: authority.toBytes(),
			message,
			signature: Buffer.from(signature)
		})
		const createOrder = await this.#program.methods
			.createSellOrder(payload)
			.accounts({
				payer: signer.publicKey,
				tickerMintAccount: payload.tickerMint,
				paymentMintAccount: payload.paymentMint
			})
			.transaction()

		const tx = new anchor.web3.Transaction()
			.add(ed25519Ix)
			.add(createOrder)

		const txid = await this.#provider.sendAndConfirm(tx, [signer], {
			commitment: 'confirmed'
		})

		return this.provider.connection.getTransaction(txid, {
			commitment: 'confirmed',
			maxSupportedTransactionVersion: 0
		})
	}

	async cancel (orderId : number) {
		const { signer } = this

		const order = await this.order(signer.publicKey, orderId) as any
		const refundAccount = await ata(
			Object.keys(order.side)[0] === 'buy' ? order.paymentMint : order.tickerMint,
			signer.publicKey
		)

		return this.#program.methods
			.cancelOrder(orderId)
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				refundAccount
			})
			.signers([signer]).rpc()
	}

	async process (maker : PublicKey, orderId : number) {
		const { signer } = this
		const order = this.pda(['order', maker.toBuffer(), new BN(orderId).toArrayLike(Buffer, 'le', 8)])

		return this.#program.methods
			.processOrder()
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				order
			})
			.signers([signer]).rpc()
	}

	async execute (maker : PublicKey, orderId : number, spent : bigint, proofCid : number[]) {
		const { signer } = this
		const { side, tickerMint, paymentMint } = await this.order(maker, orderId)
		
		const refundAccount = await ata(paymentMint, maker)
		const makerAccount = Object.keys(side)[0] === 'buy' 
			? await ata(tickerMint, maker)
			: refundAccount


		const executeOrder = await this.#program.methods
			.executeOrder(orderId, new BN(spent), Buffer.from(proofCid))
			.accounts({
				payer: signer.publicKey,
				maker,
				makerAccount,
				refundAccount,
				paymentMint,
				tickerMint,
			})
			.transaction()
		
		const tx = new anchor.web3.Transaction()
			.add(executeOrder)

		const txid = await this.#provider.sendAndConfirm(tx, [signer], {
			commitment: 'confirmed'
		})

		return this.provider.connection.getTransaction(txid, {
			commitment: 'confirmed',
			maxSupportedTransactionVersion: 0
		})
	}
}

const TickerToken = await new Ticker().init()
export default TickerToken
