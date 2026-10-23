import assert from 'node:assert/strict'
import { test } from 'node:test'
import { limitSectionName, rosterLines, sameRosterGroup, sectionNames, sectionRename, shownRows, type RosterLine } from './roster-sections.ts'

const item = (id: string, section?: string, { pinned = false, unread = 0 } = {}) => ({ id, bot: { pinned, section, unread } })
type Item = ReturnType<typeof item>

/** Each line as text: a row's id, the "Bots" label, or a header's name, count, unread and fold. */
const outline = (lines: RosterLine<Item>[]) =>
  lines.map((line) => (line.kind === 'row' ? line.item.id : line.kind === 'label' ? 'BOTS'
    : `${line.name} ${line.count} ${line.unread}${line.folded ? ' folded' : ''}: ${line.items.map((i) => i.id).join(' ')}`))

test('pinned rows first, then sections by name, then rows without a section under the label', () => {
  const roster = [
    item('pinned-work', 'Work', { pinned: true }),
    item('loose'),
    item('work-1', 'Work', { unread: 2 }),
    item('home', 'personal'),
    item('work-2', 'Work', { unread: 3 }),
    item('apps', 'apps'),
  ]
  assert.deepEqual(outline(rosterLines(roster, [])), [
    'pinned-work',
    'apps 1 0: apps',
    'personal 1 0: home',
    'Work 2 5: work-1 work-2',
    'BOTS',
    'loose',
  ])
})

test('a folded section keeps its rows for the fold, but only its header is on screen', () => {
  const roster = [item('a', 'Work'), item('b', 'Work'), item('c')]
  const lines = rosterLines(roster, ['Work'])
  assert.deepEqual(outline(lines), ['Work 2 0 folded: a b', 'BOTS', 'c'])
  assert.deepEqual(shownRows(lines).map((i) => i.id), ['c'])
  assert.deepEqual(shownRows(rosterLines(roster, [])).map((i) => i.id), ['a', 'b', 'c'])
})

test('a section only its pinned bots are in has no header, and no header means no label', () => {
  assert.deepEqual(outline(rosterLines([item('p', 'Work', { pinned: true }), item('c')], [])), ['p', 'c'])
})

test('the label shows only with a section header and a bot in none', () => {
  assert.deepEqual(outline(rosterLines([item('a'), item('b')], [])), ['a', 'b'])
  assert.deepEqual(outline(rosterLines([item('p', undefined, { pinned: true }), item('a', 'Work')], [])), ['p', 'Work 1 0: a'])
})

test('a bot moves past bots with the same pin state and section; pinned bots ignore sections', () => {
  const bot = (section?: string, pinned = false) => ({ pinned, section })
  assert.equal(sameRosterGroup(bot('Work'), bot('Work')), true)
  assert.equal(sameRosterGroup(bot('Work'), bot('Home')), false)
  assert.equal(sameRosterGroup(bot('Work'), bot()), false)
  assert.equal(sameRosterGroup(bot(), bot('')), true)
  assert.equal(sameRosterGroup(bot('Work', true), bot('Home', true)), true)
  assert.equal(sameRosterGroup(bot('Work', true), bot('Work')), false)
})

test('every section once, sorted by name', () => {
  assert.deepEqual(sectionNames([{ section: 'Work' }, {}, { section: 'apps' }, { section: 'Work' }, { section: '' }]), ['apps', 'Work'])
})

test('removing a section sends `to` as null, which JSON keeps', () => {
  assert.equal(JSON.stringify(sectionRename('Work', null)), '{"from":"Work","to":null}')
  assert.equal(JSON.stringify(sectionRename('Work', '  ')), '{"from":"Work","to":null}')
  assert.deepEqual(sectionRename('Work', ' Jobs '), { from: 'Work', to: 'Jobs' })
})

test('a name field stops at 40 characters as the host counts them, never inside an emoji', () => {
  const forty = 'a'.repeat(40)
  assert.equal(limitSectionName(forty + 'b'), forty)
  assert.equal(limitSectionName('😀'.repeat(41)), '😀'.repeat(40))
  assert.equal(limitSectionName('a'.repeat(39) + '😀b'), 'a'.repeat(39) + '😀')
})
