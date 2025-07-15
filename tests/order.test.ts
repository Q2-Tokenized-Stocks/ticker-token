import { test } from 'node:test'
import assert from 'node:assert/strict'

import { SendTransactionError } from '@solana/web3.js'
import { EventParser, web3  } from '@coral-xyz/anchor'

import TickerToken, { Ticker } from './ticker-tocken.ts'

import oracle, { Oracle } from './oracle.ts'
import { createUser, randomString } from './utils.ts'


test('order creating', async () => {
	const symbol = randomString()
	await TickerToken.createTicker(symbol)

	const { payload, message, signature, _paymentToken: token } = await oracle.order(symbol, 'buy', 10)
	const user = await createUser({ tokens: [{ token, balance: 1e18 }] })
	
	const tx = await TickerToken.connect(user).buy(payload, { message, signature })

	const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
	const events = await parser.parseLogs(tx.meta.logMessages)

	for (const event of events) {
		if (event.name !== 'orderCreated') continue
		
		const { id, maker } = event.data
		const order = await TickerToken.order(user.publicKey, id.toString())

		assert.equal(order.id.toString(), payload.id.toString())
		assert.equal(order.maker.toString(), maker.toString())
		assert.equal(order.tickerMint.toString(), payload.tickerMint.toString())
		assert.equal(order.amount.toString(), payload.amount.toString())
		assert.equal(order.paymentMint.toString(), payload.paymentMint.toString())
		assert.equal(order.price.toString(), payload.price.toString())
		assert.equal(order.fee.toString(), payload.fee.toString())
	}
})

test('fake oracle signature fails', async () => {
	const symbol = randomString()
	await TickerToken.createTicker(symbol)
	const fake = new Oracle(web3.Keypair.generate().secretKey)

	const { payload, message, signature, _paymentToken: token } = await fake.order(symbol, 'buy', 10)
	const user = await createUser({ tokens: [{ token, balance: 1e18 }] })

	assert.rejects(TickerToken.connect(user).buy(payload, { message, signature }))
})