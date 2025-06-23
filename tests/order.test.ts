import { test } from 'node:test'
import assert from 'node:assert/strict'

import TickerToken, { Ticker } from './ticker-tocken.ts'

import Oracle from './oracle.ts'
import { createUser, randomString } from './utils.ts'

test('order creating', async () => {
	const symbol = randomString()
	await TickerToken.createTicker(symbol)

	const { payload, signature, _paymentToken: token } = await Oracle.order(symbol, 'buy', 10)
	await TickerToken.setOracle(Oracle.signer)

	const user = await createUser({ tokens: [{ token, balance: 1e18 }] })
	
	console.log(await TickerToken.connect(user).createOrder(payload, signature))

})