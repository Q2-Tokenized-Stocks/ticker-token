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
		const user = await createUser()
		const { payload, message, signature } = await fake.payload(
			TickerToken.program.programId, user.publicKey, symbol, 10
		)

		assert.rejects(TickerToken.connect(user).buy(payload, { message, signature }))
	})

	await test('Payer must be the order maker', async () => {
		const symbol = randomString()
		await TickerToken.createTicker(symbol)
		const user = await createUser()
		const { payload, message, signature } = await oracle.payload(
			TickerToken.program.programId, user.publicKey, symbol, 10
		)

		const otherUser = await createUser()
		await assert.rejects(
			TickerToken.connect(otherUser).buy(payload, { message, signature }),
			'Payer must be the order maker'
		)
	})

	let buyOrderId
	let orderMaker = await createUser()
	let token

	const symbol = randomString()

	await test('Buy order creating', async () => {
		await TickerToken.createTicker(symbol)

		const { payload, message, signature, _paymentToken } = await oracle.payload(
			TickerToken.program.programId, orderMaker.publicKey, symbol, 10
		)
		
		token = _paymentToken
		await _paymentToken.mintTo(orderMaker.publicKey, 1e18, orderMaker)

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
			assert.equal(Object.keys(order.side)[0], 'buy', 'Order side should be buy')
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

	await test('Execute order', async () => {
		await test('Processing order by non-authority fails', async () => {
			assert.rejects(TickerToken.connect(orderMaker).process(orderMaker.publicKey, buyOrderId))
		})

		await test('Order status sets to processing', async () => {
			await TickerToken.process(orderMaker.publicKey, buyOrderId)
		
			const order = await TickerToken.order(orderMaker.publicKey, buyOrderId)
			assert.equal(Object.keys(order.status)[0], 'processing', 'Order status should be processing')
		})

		await test('Executing buy order', async () => {
			const proof = await oracle.cid(buyOrderId)
			const order = await TickerToken.order(orderMaker.publicKey, buyOrderId)
			const spent = BigInt(order.amount) * BigInt(order.price * .8 >> 0)
			const tx = await TickerToken.execute(
				orderMaker.publicKey, buyOrderId, spent, Array.from(proof)
			)

			const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
			const events = await parser.parseLogs(tx.meta.logMessages)

			for (const event of events) {
				if (event.name !== 'orderExecuted') continue

				const data = event.data as any
				assert.equal(Array.from(data.proofCid).toString(), proof.toString(), 'Proof CID mismatch')

				delete data.proofCid
				delete data.timestamp
				delete order.status
				delete order.expiresAt

				//console.log({ data, order })

				assert.deepEqual(data, order, 'Order data mismatch in event')
			}

			const poolPDA = TickerToken.pda(['pool', order.tickerMint.toBuffer(), order.paymentMint.toBuffer()])
			const { amount: poolBalance } = await getAccount(TickerToken.provider.connection, poolPDA)

			const expectedBalance = spent + BigInt(order.fee)
			assert.equal(
				poolBalance, expectedBalance,
				'Pool balance mismatch after order execution'
			)

			const tickerBalance = await TickerToken.balance(symbol, orderMaker.publicKey)
			assert.equal(
				tickerBalance, BigInt(order.amount),
				'Ticker balance mismatch after order execution'
			)
			assert.rejects(TickerToken.order(orderMaker.publicKey, buyOrderId), 'Order should not exist after execution')
		})
	})

	let sellOrderId
	await test('Sell order creating', async () => {
		const { payload, message, signature } = await oracle.payload(
			TickerToken.program.programId, orderMaker.publicKey, symbol, OrderSide.Sell, 1
		)
		const makerTokenBalanceBefore = await TickerToken.balance(symbol, orderMaker.publicKey)

		const tx = await TickerToken.connect(orderMaker).sell(payload, { message, signature })

		const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
		const events = await parser.parseLogs(tx.meta.logMessages)

		for (const event of events) {
			if (event.name !== 'orderCreated') continue

			const { id, maker } = event.data
			const order = await TickerToken.order(maker, id.toString())

			assert.equal(order.id.toString(), payload.id.toString(), 'Order ID mismatch')
			assert.equal(Object.keys(order.side)[0], 'sell', 'Order side should be sell')
			assert.equal(order.maker.toString(), orderMaker.publicKey.toString(), 'Order maker mismatch')
			assert.equal(order.tickerMint.toString(), payload.tickerMint.toString(), 'Ticker mint mismatch')
			assert.equal(order.amount.toString(), payload.amount.toString(), 'Order amount mismatch')
			assert.equal(order.paymentMint.toString(), payload.paymentMint.toString(), 'Payment mint mismatch')
			assert.equal(order.price.toString(), payload.price.toString(), 'Order price mismatch')
			assert.equal(order.fee.toString(), payload.fee.toString(), 'Order fee mismatch')
		}

		assert.equal(
			await TickerToken.balance(symbol, orderMaker.publicKey), makerTokenBalanceBefore - BigInt(payload.amount),
			'Maker ticker balance mismatch after order creation'
		)

		sellOrderId = payload.id

		await test('Execute sell order', async () => {
			await TickerToken.process(orderMaker.publicKey, sellOrderId)
			const proof = await oracle.cid(sellOrderId)
			const order = await TickerToken.order(orderMaker.publicKey, sellOrderId)

			const poolPDA = TickerToken.pda(['pool', order.tickerMint.toBuffer(), order.paymentMint.toBuffer()])
			//const { amount: poolBalanceBefore } = await getAccount(TickerToken.provider.connection, poolPDA)
			const { amount: makerPaymentBalanceBefore } = await token.account(orderMaker.publicKey)
			const supplyBefore = await TickerToken.supply(symbol)
			const spent = BigInt(order.amount) * BigInt(order.price * .8 >> 0) // simulate partial fill

			const { amount: poolBalanceBefore } = await getAccount(TickerToken.provider.connection, poolPDA)

			const tx = await TickerToken.execute(
				orderMaker.publicKey, sellOrderId, spent, Array.from(proof)
			)

			const parser = new EventParser(TickerToken.program.programId, TickerToken.program.coder)
			const events = await parser.parseLogs(tx.meta.logMessages)

			for (const event of events) {
				if (event.name !== 'orderExecuted') continue
				const data = event.data as any
				assert.equal(Array.from(data.proofCid).toString(), proof.toString(), 'Proof CID mismatch')

				delete data.proofCid
				delete data.timestamp
				delete order.status
				delete order.expiresAt

				assert.deepEqual(data, order, 'Order data mismatch in event')
			}

			const { amount: poolBalanceAfter } = await getAccount(TickerToken.provider.connection, poolPDA)
			assert.equal(
				poolBalanceAfter, poolBalanceBefore - spent + BigInt(order.fee),
				'Pool balance mismatch after sell order execution'
			)

			const { amount: makerPaymentBalanceAfter } = await token.account(orderMaker.publicKey)
			assert.equal(
				makerPaymentBalanceAfter, makerPaymentBalanceBefore + spent,
				'Maker payment balance mismatch after sell order execution'
			)

			const supplyAfter = await TickerToken.supply(symbol)
			assert.equal(
				supplyAfter, supplyBefore - BigInt(order.amount),
				'Supply mismatch after sell order execution'
			)
		})
	})

	await test('Cancel order', async () => {
		await test('Cancel executed order fails', async () => {
			await assert.rejects(
				TickerToken.connect(orderMaker).cancel(buyOrderId),
				'Cannot cancel executed order'
			)
		})
		await test('Cancel order by non-maker fails', async () => {
			const otherUser = await createUser({ tokens: [{ token, balance: 1e18 }] })
			await assert.rejects(
				TickerToken.connect(otherUser).cancel(buyOrderId),
				'Only the order maker can cancel the order'
			)
		})

		await test('Cancel buy order', async () => {
			const { payload, message, signature } = await oracle.payload(TickerToken.program.programId, orderMaker.publicKey, symbol, OrderSide.Buy, 1)
			await TickerToken.connect(orderMaker).buy(payload, { message, signature })
			const order = await TickerToken.order(orderMaker.publicKey, payload.id)

			const { amount: makerPaymentBalanceBefore } = await token.account(orderMaker.publicKey)
			const total = BigInt(order.amount) * BigInt(order.price) + BigInt(order.fee)

			await TickerToken.connect(orderMaker).cancel(order.id)

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
			assert.rejects(
				TickerToken.order(orderMaker.publicKey, order.id),
				'Order should not exist after cancellation'
			)
		})

		await test('Cancel sell order', async () => {
			const { payload, message, signature } = await oracle.payload(TickerToken.program.programId, orderMaker.publicKey, symbol, OrderSide.Sell, 1)
			await TickerToken.connect(orderMaker).sell(payload, { message, signature })
			const order = await TickerToken.order(orderMaker.publicKey, payload.id)

			const makerTickerBalanceBefore = await TickerToken.balance(symbol, orderMaker.publicKey)

			await TickerToken.connect(orderMaker).cancel(order.id)

			const makerTickerBalanceAfter = await TickerToken.balance(symbol, orderMaker.publicKey)
			assert.equal(
				makerTickerBalanceAfter, makerTickerBalanceBefore + BigInt(order.amount),
				'Maker ticker balance mismatch after sell order cancellation'
			)

			const orderPDA = TickerToken.pda(['order', orderMaker.publicKey.toBuffer(), new BN(order.id).toArrayLike(Buffer, 'le', 8)])
			const escrowPDA = TickerToken.pda(['escrow', orderPDA.toBuffer()])
			assert.rejects(
				getAccount(TickerToken.provider.connection, escrowPDA),
				'Escrow account should be closed after sell order cancellation'
			)
			assert.rejects(
				TickerToken.order(orderMaker.publicKey, order.id),
				'Order should not exist after cancellation'
			)
		})
	})		
})
