import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'

import { PublicKey, SystemProgram } from '@solana/web3.js'

import { TickerToken } from '~/target/types/ticker_token'

anchor.setProvider(anchor.AnchorProvider.env())

export class Ticker {
	#program = anchor.workspace.ticker_token as Program<TickerToken>
	get signer () { return this.#provider.wallet }

	#provider = anchor.getProvider()
	get provider () { return this.#provider }

	#registryPDA : PublicKey
	get registry () {
		return this.#program.account.registry.fetch(this.#registryPDA)
	}

	constructor () {
		const [registryPDA] = PublicKey.findProgramAddressSync(
			[Buffer.from('registry')],
			this.#program.programId
		)

		this.#registryPDA = registryPDA
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

	addAdmin (admin : PublicKey, signer = this.signer.payer) {
		return this.#program.methods
			.addAdmin(admin)
			.accounts({
	    		caller: signer.publicKey,
    			// @ts-ignore
    			registry: this.#registryPDA
  			})
			.signers([signer]).rpc()
	}

	removeAdmin (admin : PublicKey, signer = this.signer.payer) {
		return this.#program.methods
			.removeAdmin(admin)
			.accounts({
	    		caller: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA
  			})
			.signers([signer]).rpc()
	}
}

const ticker = new Ticker
await ticker.init()

export default ticker