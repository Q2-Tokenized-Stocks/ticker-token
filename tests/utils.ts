import * as anchor from '@coral-xyz/anchor'

import { 
	ASSOCIATED_TOKEN_PROGRAM_ID, TOKEN_PROGRAM_ID,
	createAssociatedTokenAccountInstruction, createInitializeAccountInstruction, 
	getAccount, getAssociatedTokenAddress, getMinimumBalanceForRentExemptAccount
} from '@solana/spl-token'
import { PublicKey, SystemProgram, Transaction } from '@solana/web3.js'

const TOKEN_ACCOUNT_LEN = 165 // 165 bytes for a token account
const provider = anchor.getProvider()

export const randomString = (length = 6) => Math.random().toString(36).substring(2, 2 + length).toUpperCase()

export const pda = (
	seeds = [], 
	programId = anchor.workspace.tickerToken.programId
) => PublicKey.findProgramAddressSync(
	seeds.map(s => Buffer.isBuffer(s) ? s : Buffer.from(s)),
	programId
)

export async function createPDA (address, mint, signer) {
	const lamports = await getMinimumBalanceForRentExemptAccount(provider.connection)
	const tx = new Transaction().add(
		SystemProgram.createAccount({
			fromPubkey: signer.publicKey,
			newAccountPubkey: address,
			lamports,
			space: 165,
			programId: TOKEN_PROGRAM_ID
		}),
		createInitializeAccountInstruction(
			address,
			mint,
			address,
			TOKEN_PROGRAM_ID
		)
	)

	await provider.sendAndConfirm(tx, [signer])
	return getAccount(provider.connection, address)
}

export async function ata (mint, owner) {
	const ata = await getAssociatedTokenAddress(
		mint, owner, true, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID
	)

	return ata
} 

export async function createATA (ata, mint, signer, owner?) {
	const ix = createAssociatedTokenAccountInstruction(
		signer.publicKey,
		ata, owner || signer.publicKey, mint
	)

	const tx = new anchor.web3.Transaction().add(ix)
	await provider.sendAndConfirm(tx, [signer])

	return getAccount(provider.connection, ata)
}

export async function createUser ({ tokens = [], sol = 1e9 } = {}) {
	const user = anchor.web3.Keypair.generate()
	sol ??= 1e18 // Default to 10 SOL if not specified

	await provider.connection
		.requestAirdrop(user.publicKey, sol)
		.then(sig => provider.connection.confirmTransaction(sig))

	for (const { token, balance } of tokens || [])
		await token.mintTo(user.publicKey, balance, user)

	return user
}

export async function splAccount (address : PublicKey, mint, payer?, owner?) {
	const account = await provider.connection.getAccountInfo(address)

	if (account) return getAccount(provider.connection, address)
	if (!payer)
		throw new Error('Payer is required to create a new account')

	const ata = await getAssociatedTokenAddress(
		mint, owner || payer.publicKey, true
	)

	return address.equals(ata)
		? createATA(ata, mint, payer, owner)
		: createPDA(address, mint, payer)
}