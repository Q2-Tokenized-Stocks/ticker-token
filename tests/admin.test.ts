import { test } from 'node:test'
import assert from 'node:assert/strict'

import { Keypair } from '@solana/web3.js'
import ticker from './ticker-tocken.ts'


test('registry was initialized correctly', async () => {
  const { superAdmin } = await ticker.registry

  assert.equal(
	superAdmin.toBase58(), ticker.provider.wallet.publicKey.toBase58(), 
  	'Super admin should be the wallet used to initialize the registry'
  )
})

test('init fails if registry already initialized', () => 
	assert.rejects(ticker.init())
)

test('super admin can add a new admin', async () => {
	const admin = Keypair.generate()

	await ticker.addAdmin(admin.publicKey)
	const { admins } = await ticker.registry

  	assert.ok(
    	admins.some(a => a.toBase58() === admin.publicKey.toBase58()),
    	'New admin should be added to the registry'
  	)
})

test('non-admin cannot add new admin', async () => {
	const admin = Keypair.generate()
	const nonadmin = Keypair.generate()

	assert.rejects(ticker.addAdmin(admin.publicKey, nonadmin))
})

test('admin can be removed', async () => {
	const adminToRemove = Keypair.generate()

	// Add admin first
	await ticker.addAdmin(adminToRemove.publicKey)
	// Then remove
	await ticker.removeAdmin(adminToRemove.publicKey)

	const { admins } = await ticker.registry

  	assert.ok(
    	!admins.some(admin => admin.toBase58() === adminToRemove.publicKey.toBase58()),
    	'Admin should be removed from the registry'
  	)
})

test('non-admin cannot remove admin', async () => {
	const nonadmin = Keypair.generate()
	const target = Keypair.generate()

	await assert.rejects(
		ticker.removeAdmin(target.publicKey, nonadmin)
	)
})
