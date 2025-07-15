import { test } from 'node:test'
import assert from 'node:assert/strict'

import { Keypair } from '@solana/web3.js'
import TickerToken from './ticker-tocken.ts'

import { randomString, pda } from './utils.ts'
import { getMint } from '@solana/spl-token'

test('registry was initialized correctly', async () => {
  	const { authority } = await TickerToken.registry

	assert.equal(
		authority.toBase58(), TickerToken.owner.publicKey.toBase58(), 
  		'Super admin should be the wallet used to initialize the registry'
	)
})

test('init fails if registry already initialized', () => 
	assert.rejects(TickerToken.init())
)

//test('can transfer authority', async () => {
//	const { authority } = await TickerToken.registry
//	const newAuthority = Keypair.generate()

//	await assert.rejects(
//		TickerToken.connect(null).transferAuthority(newAuthority.publicKey),
//		'Only the authority can transfer authority'
//	)

//	await TickerToken.transferAuthority(newAuthority.publicKey)

//	// Verify that the authority was updated
//	const updated = await TickerToken.registry
//	assert.equal(
//		updated.authority.toBase58(), newAuthority.publicKey.toBase58(),
//		'Authority should be updated to the new authority'
//	)

//	await TickerToken.connect(newAuthority).transferAuthority(authority)
//})

test('can create a ticker', async () => {
	const symbol = randomString()
	const decimals = 6
	const uri = 'https://ipfs.io/ipfs/${symbol}.json'

	await assert.rejects(
		TickerToken.connect().createTicker(symbol),
		'Only the authority can create a ticker'
	)

	await TickerToken.createTicker(symbol, decimals)

	const [mintPDA] = pda(['mint', symbol])
	const mint = await getMint(TickerToken.provider.connection, mintPDA)

	assert.equal(mint.decimals, decimals, `Mint for ${symbol} should have ${decimals} decimals`)
	assert.equal(mint.supply.toString(), '0', `Mint for ${symbol} should have zero supply`)
})
