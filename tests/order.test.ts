import { test } from 'node:test'
import assert from 'node:assert/strict'

import { Keypair } from '@solana/web3.js'
import TickerToken, { pda } from './ticker-tocken.ts'

import Oracle, { USDC } from './oracle.ts'
import { randomString } from './utils.ts'

test('can create order via oracle', async () => {
	const symbol = randomString()
	
	await TickerToken.createTicker(symbol)
	const ticker = await TickerToken.ticker(symbol)

	const { payload, signature } = await Oracle.order(symbol, 'buy', 10)
	
	const user = Keypair.generate()
	await USDC.mintTo(user.publicKey, 1000)

	console.log(await TickerToken.createOrder(payload, signature))

})