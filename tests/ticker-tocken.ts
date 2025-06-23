import * as anchor from '@coral-xyz/anchor'

import { 
	Keypair, PublicKey, Ed25519Program,
	SystemProgram, SYSVAR_INSTRUCTIONS_PUBKEY, SYSVAR_RENT_PUBKEY 
} from '@solana/web3.js'
import { TOKEN_PROGRAM_ID } from '@solana/spl-token'

import { TickerToken } from '~/target/types/ticker_token'
import IDL from '../target/idl/ticker_token.json' with { type: 'json' }

import { pda, ata } from './utils.ts'
import type { OraclePayload } from './oracle'

anchor.setProvider(anchor.AnchorProvider.env())

export class Ticker {

	get idl () { return IDL as anchor.Idl }

	#program = anchor.workspace.ticker_token as anchor.Program<TickerToken>
	get program () { return this.#program }
	
	#provider = anchor.getProvider()
	get provider () { return this.#provider }

	#signer = null
	get signer () { return this.#signer || this.#provider.wallet.payer }

	#registryPDA = null
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
		this.#registryPDA = pda(['registry'])[0]
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
				authority: signer.publicKey,
				// @ts-ignore
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
				authority: signer.publicKey,
				// @ts-ignore
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

				systemProgram: SystemProgram.programId,
				tokenProgram: TOKEN_PROGRAM_ID,
				rent: SYSVAR_RENT_PUBKEY,
			})
			.signers([signer]).rpc()
	}

	async createOrder (payload, signature) {
		const { signer } = this
		
		const makerPaymentAccount = await ata(payload.paymentMint, signer.publicKey)

		// escrow
		const [paymentEscrowAccount] = pda(
			['payment_escrow', signer.publicKey.toBuffer(), payload.paymentMint.toBuffer()]
		)
		const [tokenEscrowAccount] = pda(
			['token_escrow', signer.publicKey.toBuffer(), payload.tokenMint.toBuffer()]
		)
		const [releaseEscrowAccount] = pda(
			['release_escrow', signer.publicKey.toBuffer(), payload.paymentMint.toBuffer()]
		)
		const programOwner = pda(['program_owner', payload.tokenMint.toBuffer(), payload.paymentMint.toBuffer()])[0]
		const lpVault = await ata(payload.paymentMint, programOwner)

		const { oracle } = await this.registry
		const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
			publicKey: oracle.toBuffer(),
			message: Buffer.from(JSON.stringify(payload)),
			signature: Buffer.from(signature)
		})

		const createOrder = await this.#program.methods
      		.createOrder(payload, signature)
			.accounts({
				payer: signer.publicKey,
				// @ts-ignore
				registry: this.#registryPDA,

				paymentMintAccount: payload.paymentMint,
				tokenMintAccount: payload.tokenMint,
				
				makerPaymentAccount,

				programOwner,
				lpVault,
				paymentEscrowAccount,
				tokenEscrowAccount,
				releaseEscrowAccount,

				instructionSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
				systemProgram: SystemProgram.programId,
			})
			.transaction()

		const tx = new anchor.web3.Transaction()
			.add(ed25519Ix)
			.add(createOrder)

		return this.#provider.sendAndConfirm(tx, [signer])
	}
}

const ticker = new Ticker
await ticker.init()
export default ticker