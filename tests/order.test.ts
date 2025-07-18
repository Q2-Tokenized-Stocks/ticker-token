import { test } from 'node:test'
import assert from 'node:assert/strict'

import BN from 'bn.js'
import { EventParser, web3  } from '@coral-xyz/anchor'

import TickerToken, { Ticker } from '../lib/ticker-tocken.ts'

import { Oracle, OrderSide } from '../lib/oracle.ts'
import { createUser, randomString } from '../lib/utils.ts'
import { getAccount } from '@solana/spl-token'

const oracle = new Oracle(TickerToken.signer.secretKey)

test('[TickerToken] Order', async () => {
	await test('fake oracle signature fails', async () => {
		const symbol = randomString()
		await TickerToken.createTicker(symbol)
		const fake = new Oracle(web3.Keypair.generate().secretKey)

		const { payload, message, signature, _paymentToken: token } = await fake.order(
			TickerToken.program.programId, symbol, OrderSide.Buy, 10
		)
		const user = await createUser({ tokens: [{ token, balance: 1e18 }] })

		assert.rejects(TickerToken.connect(user).buy(payload, { message, signature }))
	})

	let buyOrderId
	let orderMaker
	let token
	await test('Buy order creating', async () => {
		const symbol = randomString()
		await TickerToken.createTicker(symbol)

		const { payload, message, signature, _paymentToken } = await oracle.order(
			TickerToken.program.programId, symbol, OrderSide.Buy, 10
		)
		token = _paymentToken
		orderMaker = await createUser({ tokens: [{ token, balance: 1e18 }] })
		const { amount: makerPaymentBalance } = await token.account(orderMaker.publicKey)

		assert.equal(makerPaymentBalance, BigInt(1e18))
		
		const tx = await TickerToken.connect(orderMaker).buy(payload, { message, signature })

		//console.log(tx.meta.logMessages?.join('\n'))

		const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
		const events = await parser.parseLogs(tx.meta.logMessages)

		for (const event of events) {
			if (event.name !== 'orderCreated') continue

			const { id, maker } = event.data
			const order = await TickerToken.order(maker, id.toString())

			//console.log(order)

			assert.equal(order.id.toString(), payload.id.toString(), 'Order ID mismatch')
			assert.equal(order.maker.toString(), orderMaker.publicKey.toString(), 'Order maker mismatch')
			assert.equal(order.tickerMint.toString(), payload.tickerMint.toString(), 'Ticker mint mismatch')
			assert.equal(order.amount.toString(), payload.amount.toString(), 'Order amount mismatch')
			assert.equal(order.paymentMint.toString(), payload.paymentMint.toString(), 'Payment mint mismatch')
			assert.equal(order.price.toString(), payload.price.toString(), 'Order price mismatch')
			assert.equal(order.fee.toString(), payload.fee.toString(), 'Order fee mismatch')
		}

		const spent = BigInt(payload.amount) * BigInt(payload.price) + BigInt(payload.fee)
		const { amount: makerPaymentBalanceAfter } = await token.account(orderMaker.publicKey)
		assert.equal(
			makerPaymentBalanceAfter, makerPaymentBalance - spent,
			'Maker payment balance mismatch after order creation'
		)

		const orderPDA = TickerToken.pda(['order', orderMaker.publicKey.toBuffer(), new BN(payload.id).toArrayLike(Buffer, 'le', 8)])
		const escrowPDA = TickerToken.pda(['escrow', orderPDA.toBuffer()])
		const { amount: escrowPaymentBalance } = await getAccount(TickerToken.provider.connection, escrowPDA)
		assert.equal(
			escrowPaymentBalance, spent,
			'Escrow payment balance mismatch after order creation'
		)

		buyOrderId = payload.id
	})

	await test('Cancel order', async () => {
		const order = await TickerToken.order(orderMaker.publicKey, buyOrderId)
		const total = BigInt(order.amount) * BigInt(order.price) + BigInt(order.fee)
		const { amount: makerPaymentBalanceBefore } = await token.account(orderMaker.publicKey)

		await TickerToken.connect(orderMaker).cancel(buyOrderId)

		const { amount: makerPaymentBalanceAfter } = await token.account(orderMaker.publicKey)
		assert.equal(
			makerPaymentBalanceAfter, makerPaymentBalanceBefore + total,
			'Maker payment balance mismatch after order cancellation'
		)

		const orderPDA = TickerToken.pda(['order', orderMaker.publicKey.toBuffer(), new BN(order.id).toArrayLike(Buffer, 'le', 8)])
		const escrowPDA = TickerToken.pda(['escrow', orderPDA.toBuffer()])

		assert.rejects(
			getAccount(TickerToken.provider.connection, escrowPDA),
			'Escrow account should be closed after order cancellation'
		)

	})
		
})