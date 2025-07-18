import * as anchor from '@coral-xyz/anchor'
import BN from 'bn.js'

import { 
	Keypair, PublicKey, Ed25519Program,
	SystemProgram, SYSVAR_INSTRUCTIONS_PUBKEY, SYSVAR_RENT_PUBKEY 
} from '@solana/web3.js'

import { TOKEN_PROGRAM_ID } from '@solana/spl-token'

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
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA,
				systemProgram: SystemProgram.programId
			})
			.rpc()

		return this
	}

	transferAuthority (authority : PublicKey) {
		const { signer } = this

		return this.#program.methods
			.transferAuthority(authority)
			.accounts({
				authority: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA
			})
			.signers([signer])
			.rpc()
	}

	createTicker (symbol : string, decimals = 0) {
		const { signer } = this
		const mint = this.pda(['mint', symbol])

		return this.#program.methods
			.createTicker(symbol, decimals)
			.accounts({
				// @ts-ignore
				registry: this.#registryPDA,
				mint,
				systemProgram: SystemProgram.programId,
				tokenProgram: TOKEN_PROGRAM_ID,
				rent: SYSVAR_RENT_PUBKEY,
			})
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
				// @ts-ignore
				registry: this.#registryPDA,

				tickerMintAccount: payload.tickerMint,
				paymentMintAccount: payload.paymentMint,
				
				makerPaymentAccount,

				instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
				systemProgram: SystemProgram.programId,
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
		const refundAccount = await ata(order.paymentMint, signer.publicKey)

		return this.#program.methods
			.cancelOrder(orderId)
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				refundAccount
			})
			.signers([signer]).rpc()
	}
}

const TickerToken = await new Ticker().init()
export default TickerToken