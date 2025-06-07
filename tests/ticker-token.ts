import { test, before } from 'node:test'
import assert from 'node:assert/strict'

import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'

import { PublicKey, SystemProgram, Keypair, LAMPORTS_PER_SOL } from '@solana/web3.js'

import { TickerToken } from '~/target/types/ticker_token'

anchor.setProvider(anchor.AnchorProvider.env())

const provider = anchor.getProvider()
const program = anchor.workspace.ticker_token as Program<TickerToken>

const registryPDA = () => PublicKey.findProgramAddressSync(
    [Buffer.from('registry')],
    program.programId
)

//const findTickerPda = (ticker : string) =>
//	PublicKey.findProgramAddressSync(
//		[Buffer.from('ticker'), Buffer.from(ticker)], 
//		program.programId
//	)

before(async () => {
	const [registry] = registryPDA()
  	
	const instance = await program.methods
		.init()
		.accounts({
			payer: provider.wallet.publicKey,
			// @ts-ignore
			registry,
			systemProgram: SystemProgram.programId
		})
		.rpc()
})

test('registry was initialized correctly', async () => {
  const [registry] = registryPDA()
  const account = await program.account.registry.fetch(registry)

  assert.equal(
	account.superAdmin.toBase58(), provider.wallet.publicKey.toBase58(), 
  	'Super admin should be the wallet used to initialize the registry'
  )
})

test('init fails if registry already initialized', async () => {
	const [registry] = registryPDA()

	await assert.rejects(async () => {
		await program.methods.init().accounts({
    		payer: provider.wallet.publicKey,
			// @ts-ignore
    		registry,
    		systemProgram: SystemProgram.programId
    	}).rpc()
 	})
})

test('super admin can add a new admin', async () => {
	const [registry] = registryPDA()
	const admin = Keypair.generate()

	await program.methods.addAdmin(admin.publicKey).accounts({
    	caller: provider.wallet.publicKey,
    	// @ts-ignore
    	registry
  	}).rpc()

	const { admins } = await program.account.registry.fetch(registry)

  	assert.ok(
    	admins.some(a => a.toBase58() === admin.publicKey.toBase58()),
    	'New admin should be added to the registry'
  	)
})

test('non-admin cannot add new admin', async () => {
	const [registry] = registryPDA()
	const admin = Keypair.generate()
	const nonadmin = Keypair.generate()

	await assert.rejects(
    	program.methods.addAdmin(admin.publicKey).accounts({
    		caller: nonadmin.publicKey,
	      	// @ts-ignore
    	  	registry
    	}).signers([nonadmin]).rpc()
	)
})

test('admin can be removed', async () => {
	const [registry] = registryPDA()
	const adminToRemove = Keypair.generate()

	// Add admin first
	await program.methods.addAdmin(adminToRemove.publicKey).accounts({
    	caller: provider.wallet.publicKey,
		// @ts-ignore
		registry
	}).rpc()

	// Then remove
	await program.methods.removeAdmin(adminToRemove.publicKey).accounts({
    	caller: provider.wallet.publicKey,
    	// @ts-ignore
    	registry
	}).rpc()

	const { admins } = await program.account.registry.fetch(registry)

  	assert.ok(
    	!admins.some(admin => admin.toBase58() === adminToRemove.publicKey.toBase58()),
    	'Admin should be removed from the registry'
  	)
})

test('non-admin cannot remove admin', async () => {
	const [registry] = registryPDA()
	const nonadmin = Keypair.generate()
	const target = Keypair.generate()

	await assert.rejects(
    	program.methods.removeAdmin(target.publicKey).accounts({
      		caller: nonadmin.publicKey,
      		// @ts-ignore
      		registry
    	}).signers([nonadmin]).rpc()
	)
})
