import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'

import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from '@solana/spl-token'
import { Keypair, PublicKey, SystemProgram, SYSVAR_RENT_PUBKEY } from '@solana/web3.js'

import { TickerToken } from '~/target/types/ticker_token'

anchor.setProvider(anchor.AnchorProvider.env())

export const pda = (seeds = []) => PublicKey.findProgramAddressSync(
	seeds.map(seed => Buffer.from(seed)),
	anchor.workspace.tickerToken.programId
)

export class Ticker {

	#program = anchor.workspace.ticker_token as Program<TickerToken>
	get program () { return this.#program }
	
	#provider = anchor.getProvider()
	get provider () { return this.#provider }

	#signer = null
	get signer () { return this.#signer || this.#provider.wallet.payer }

	#registryPDA = pda(['registry'])[0]
	get registry () {
		return this.#program.account.registry.fetch(this.#registryPDA)
	}

	ticker (symbol : string) {
		const [tickerDataPDA] = pda(['ticker', symbol])
		return this.#program.account.tickerData.fetch(tickerDataPDA)
	}

	get tickers () {
		return this.#program.account.tickerData.all()
			.then(tickers => tickers.map(ticker => ticker.account))
	}

	connect (signer = null) : Ticker { return new Ticker(signer || Keypair.generate()) }

	constructor (signer = null) {
		this.#signer = signer
	}

	async init () {
		const { signer } = this

		await this.#program.methods
			.init()
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA,
				systemProgram: SystemProgram.programId
			}).rpc()

		return this
	}

	setOracle (oracle : PublicKey) {
		const { signer } = this
		
		return this.#program.methods.setOracle(oracle)
      		.accounts({
				// @ts-ignore
				authority: signer.publicKey,
        		registry: this.#registryPDA
      		})
      		.signers([signer])
      		.rpc()
	}

	transferAuthority (authority : PublicKey) {
		const { signer } = this

		return this.#program.methods
			.transferAuthority(authority)
			.accounts({
				// @ts-ignore
				authority: signer.publicKey,
				registry: this.#registryPDA
			})
			.signers([signer])
			.rpc()
	}

	createTicker (symbol : string, decimals = 0) {
		const { signer } = this
		
		const [tickerData] = pda(['ticker', symbol])
		const [mint] = pda(['mint', symbol])

		return this.#program.methods
			.createTicker(symbol, decimals)
			.accounts({
				authority: signer.publicKey,
				registry: this.#registryPDA,
				tickerData,
				mint,
				rent: SYSVAR_RENT_PUBKEY,
				tokenProgram: TOKEN_PROGRAM_ID,
				systemProgram: SystemProgram.programId
			})
			.signers([signer]).rpc()
	}

	async createOrder (payload, signature) {
		const { signer } = this

		const [mint] = pda(['mint', payload.symbol])

		const [escrowOwner] = pda(['escrow', signer.publicKey.toBuffer(), mint.toBuffer()])
		const escrowToken = await getAssociatedTokenAddress(
			mint, escrowOwner, true, TOKEN_PROGRAM_ID, this.#program.programId
		)
		const escrowPayment = await getAssociatedTokenAddress(
			mint, signer.publicKey, true, TOKEN_PROGRAM_ID, this.#program.programId
		)
		const vault = await getAssociatedTokenAddress(
			mint, this.#registryPDA, true, TOKEN_PROGRAM_ID, this.#program.programId
		)
		const userPaymentAccount = await getAssociatedTokenAddress(
			mint, signer.publicKey, true, TOKEN_PROGRAM_ID, this.#program.programId
		)

		return this.#program.methods
      		.createOrder(payload, signature)
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA,
				userPaymentAccount,
				vault,
				escrowOwner,
				escrowToken,
				escrowPayment
			})
			.signers([signer])
			.rpc()
	}
}

const ticker = new Ticker
await ticker.init()
export default ticker