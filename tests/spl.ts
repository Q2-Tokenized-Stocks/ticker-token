import * as anchor from '@coral-xyz/anchor'
import { createMint, getOrCreateAssociatedTokenAccount, mintTo } from '@solana/spl-token'
import { Keypair } from '@solana/web3.js'

const provider = anchor.getProvider()

export class SPLToken {
	constructor (
		public mint: anchor.web3.PublicKey, 
		public payer: anchor.web3.Keypair, 
		public symbol: string, 
		public decimals = 6
	) {}

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

	async mintTo (dest: anchor.web3.PublicKey, amount: number) {
		const ata = await getOrCreateAssociatedTokenAccount(
			provider.connection,
			this.payer,
			this.mint,
			dest
		)

		await mintTo(
			provider.connection,
			this.payer,
			this.mint,
			ata.address,
			this.payer,
			amount
		)

		return ata.address
	}
}