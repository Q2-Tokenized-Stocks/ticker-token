import { test } from 'node:test'
import assert from 'node:assert/strict'

import { Keypair } from '@solana/web3.js'
import TickerToken, { pda } from './ticker-tocken.ts'
import Oracle from './oracle.ts'

import { randomString } from './utils.ts'

test('registry was initialized correctly', async () => {
  	const { authority } = await TickerToken.registry

	assert.equal(
		authority.toBase58(), TickerToken.provider.wallet.publicKey.toBase58(), 
  		'Super admin should be the wallet used to initialize the registry'
	)
})

test('init fails if registry already initialized', () => 
	assert.rejects(TickerToken.init())
)

test('updating oracle', async () => {
	await assert.rejects(
		TickerToken.connect(null).setOracle(Oracle.signer),
		'Only the authority can set the oracle'
	)

	await TickerToken.setOracle(Oracle.signer)

	// Verify that the oracle was set correctly
	const { oracle } = await TickerToken.registry
	assert.equal(
		oracle.toBase58(), Oracle.signer.toBase58(),
		'Oracle should be updated to the new oracle'
	)
})

test('can transfer authority', async () => {
	const { authority } = await TickerToken.registry
	const newAuthority = Keypair.generate()

	await assert.rejects(
		TickerToken.connect(null).transferAuthority(newAuthority.publicKey),
		'Only the authority can transfer authority'
	)

	await TickerToken.transferAuthority(newAuthority.publicKey)
	// Verify that the authority was updated
	const updated = await TickerToken.registry
	assert.equal(
		updated.authority.toBase58(), newAuthority.publicKey.toBase58(),
		'Authority should be updated to the new authority'
	)

	await TickerToken.connect(newAuthority).transferAuthority(authority)
})

test('can create a ticker', async () => {
	const symbol = randomString()
	const decimals = 6

	await assert.rejects(
		TickerToken.ticker(symbol),
		'Ticker should not exist before creation'
	)

	await assert.rejects(
		TickerToken.connect().createTicker(symbol),
		'Only the authority can create a ticker'
	)

	await TickerToken.createTicker(symbol, decimals)
	const ticker = await TickerToken.ticker(symbol)

	const [mintPDA] = pda(['mint', symbol])
	assert.equal(
		ticker.mint.toBase58(), mintPDA.toBase58(),
		'Ticker mint should match the expected PDA'
	)

	assert.equal(
		symbol, 
		Buffer.from(ticker.symbol).toString('utf8').replace(/\0/g, ''), 
		'Ticker symbol should match'
	)
	assert.equal(ticker.decimals, decimals, 'Ticker decimals should match')
	
})