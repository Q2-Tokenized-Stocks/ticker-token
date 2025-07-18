import * as anchor from '@coral-xyz/anchor'
import { createMint, mintTo } from '@solana/spl-token'
import { Keypair } from '@solana/web3.js'
import { splAccount, ata } from './utils.ts'

const provider = anchor.getProvider()

export class SPLToken {
	static async create (symbol: string, decimals = 6) {
		const payer = Keypair.generate()

		const block = await provider.connection.getLatestBlockhash()
		const sig = await provider.connection.requestAirdrop(payer.publicKey, 1e9)
		
		await provider.connection.confirmTransaction({
			signature: sig,
			...block
		})

		const mint = await createMint(
			provider.connection,
			payer,
			payer.publicKey,
			null,
			decimals
		)

		return new SPLToken(mint, payer, symbol, decimals)
	}

	constructor (
		public mint: anchor.web3.PublicKey, 
		public payer: anchor.web3.Keypair, 
		public symbol: string, 
		public decimals = 6
	) {}

	async mintTo (dest: anchor.web3.PublicKey, amount: number, signer = null) {
		const { mint, payer } = this
		const account = await splAccount(await ata(mint, dest), mint, signer)

		await mintTo(
			provider.connection,
			payer, mint,
			account.address, payer,
			amount
		)

		return account
	}

	async account (address: anchor.web3.PublicKey) {
		return splAccount(await ata(this.mint, address), this.mint)
	}
}

export const USDC = await SPLToken.create('USDC', 6)