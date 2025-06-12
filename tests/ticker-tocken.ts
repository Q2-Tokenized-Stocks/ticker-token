import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'

import { TOKEN_PROGRAM_ID } from '@solana/spl-token'
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

	#registryPDA : PublicKey
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

	connect (signer = Keypair.generate()) : Ticker { return new Ticker(signer) }

	constructor (test = null) {
		const [registryPDA] = pda(['registry'])
		
		this.#registryPDA = registryPDA
		this.#signer = test
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
				// @ts-ignore
				registry: this.#registryPDA,
				tickerData,
				mint,
				rent: SYSVAR_RENT_PUBKEY,
				tokenProgram: TOKEN_PROGRAM_ID,
				systemProgram: SystemProgram.programId
			})
			.signers([signer]).rpc()
	}
}

const ticker = new Ticker
export default ticker