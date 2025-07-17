import { test } from 'node:test'
import assert from 'node:assert/strict'

import BN from 'bn.js'
import { EventParser, web3  } from '@coral-xyz/anchor'

import TickerToken from './ticker-tocken.ts'

import oracle, { Oracle, OrderSide } from './oracle.ts'
import { ata, createUser, pda, randomString } from './utils.ts'
import { getAccount, getAssociatedTokenAddress } from '@solana/spl-token'

test('fake oracle signature fails', async () => {
	const symbol = randomString()
	await TickerToken.createTicker(symbol)
	const fake = new Oracle(web3.Keypair.generate().secretKey)

	const { payload, message, signature, _paymentToken: token } = await fake.order(symbol, OrderSide.Buy, 10)
	const user = await createUser({ tokens: [{ token, balance: 1e18 }] })

	assert.rejects(TickerToken.connect(user).buy(payload, { message, signature }))
})

test('order creating', async () => {
	const symbol = randomString()
	await TickerToken.createTicker(symbol)

	const { payload, message, signature, _paymentToken: token } = await oracle.order(symbol, OrderSide.Buy, 10)
	const payer = await createUser({ tokens: [{ token, balance: 1e18 }] })
	const { amount: makerPaymentBalance } = await token.account(payer.publicKey)

	assert.equal(makerPaymentBalance, BigInt(1e18))
	
	const tx = await TickerToken.connect(payer).buy(payload, { message, signature })

	const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
	const events = await parser.parseLogs(tx.meta.logMessages)

	for (const event of events) {
		if (event.name !== 'orderCreated') continue

		const { id, maker } = event.data
		const order = await TickerToken.order(maker, id.toString())

		//console.log(order)

		assert.equal(order.id.toString(), payload.id.toString(), 'Order ID mismatch')
		assert.equal(order.maker.toString(), payer.publicKey.toString(), 'Order maker mismatch')
		assert.equal(order.tickerMint.toString(), payload.tickerMint.toString(), 'Ticker mint mismatch')
		assert.equal(order.amount.toString(), payload.amount.toString(), 'Order amount mismatch')
		assert.equal(order.paymentMint.toString(), payload.paymentMint.toString(), 'Payment mint mismatch')
		assert.equal(order.price.toString(), payload.price.toString(), 'Order price mismatch')
		assert.equal(order.fee.toString(), payload.fee.toString(), 'Order fee mismatch')
	}

	const spent = BigInt(payload.amount) * BigInt(payload.price) + BigInt(payload.fee)
	const { amount: makerPaymentBalanceAfter } = await token.account(payer.publicKey)
	assert.equal(
		makerPaymentBalanceAfter, makerPaymentBalance - spent,
		'Maker payment balance mismatch after order creation'
	)

	const [escrowPDA] = pda(['payment_escrow', payer.publicKey.toBuffer(), payload.paymentMint.toBuffer()])
	const { amount: escrowPaymentBalance } = await getAccount(TickerToken.provider.connection, escrowPDA)
	assert.equal(
		escrowPaymentBalance, spent,
		'Escrow payment balance mismatch after order creation'
	)
})

