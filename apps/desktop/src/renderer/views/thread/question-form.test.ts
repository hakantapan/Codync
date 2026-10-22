import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { QuestionField } from '../../../shared/models.ts'
import { hasAnswer, pick, questionOutcome, sendsOnTap, startsOpen } from './question-form.ts'

const db: QuestionField = { key: 'question_0', kind: 'single', title: 'Database', options: [{ value: 'PostgreSQL', label: 'PostgreSQL' }, { value: 'SQLite', label: 'SQLite' }] }
const other: QuestionField = { key: 'question_0_custom', kind: 'text', title: 'Other', options: [] }
const extras: QuestionField = { key: 'question_1', kind: 'multi', title: 'Extras', options: [{ value: 'Cache', label: 'Cache' }, { value: 'Queue', label: 'Queue' }] }

test('one single-choice question sends on tap until something is typed', () => {
  assert.equal(sendsOnTap([db, other], {}), true)
  assert.equal(sendsOnTap([db, other], { question_0_custom: ['  '] }), true)
  assert.equal(sendsOnTap([db, other], { question_0_custom: ['MySQL'] }), false)
})

test('several questions or several picks wait for Submit', () => {
  assert.equal(sendsOnTap([db, other, extras], {}), false)
  assert.equal(sendsOnTap([extras], {}), false)
})

test('a text field starts collapsed only beside choices', () => {
  assert.equal(startsOpen([db, other]), false)
  assert.equal(startsOpen([other]), true)
  assert.equal(startsOpen([other, { ...other, key: 'question_1_custom' }]), true)
})

test('a single choice replaces, a multi choice toggles', () => {
  assert.deepEqual(pick({ question_0: ['SQLite'] }, db, 'PostgreSQL'), { question_0: ['PostgreSQL'] })
  const once = pick({}, extras, 'Cache')
  assert.deepEqual(pick(once, extras, 'Queue'), { question_1: ['Cache', 'Queue'] })
  assert.deepEqual(pick(once, extras, 'Cache'), { question_1: [] })
})

test('Submit needs a pick or typed text', () => {
  assert.equal(hasAnswer({}), false)
  assert.equal(hasAnswer({ question_1: [], question_0_custom: [' '] }), false)
  assert.equal(hasAnswer({ question_1: ['Cache'] }), true)
})

test('the answered card shows each answer', () => {
  assert.equal(questionOutcome('answered', ['PostgreSQL', 'Cache, Queue']), '✓ PostgreSQL\n✓ Cache, Queue')
  assert.equal(questionOutcome('declined', null), 'Skipped')
  assert.equal(questionOutcome('cancelled', null), 'Cancelled')
  assert.equal(questionOutcome('expired', null), 'Expired — the agent moved on')
})
