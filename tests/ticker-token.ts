import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'
import { TickerToken } from '~/target/types/ticker_token'

import test from 'node:test'
import assert from 'node:assert'

anchor.setProvider(anchor.AnchorProvider.env())
const program = anchor.workspace.tickerToken as Program<TickerToken>

test('Is initialized!', async () => {
	const tx = await program.methods.initialize().rpc()
	console.log('TX:', tx)
	assert.ok(tx)
})