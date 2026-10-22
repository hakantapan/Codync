/** What placing a bot in the roster reads: its pin, the section the user put it in, its unread count. */
interface Placed {
  pinned: boolean
  section?: string
  unread: number
}

/** One line of a computer's roster: a bot's row, or a section header holding its rows. */
export type RosterLine<T> =
  | { kind: 'row'; item: T }
  | { kind: 'section'; name: string; count: number; unread: number; folded: boolean; items: T[] }

/**
 * A computer's roster (already in roster order, hidden bots left out) as lines: pinned rows, then
 * each section by name with its rows, then rows without a section. A pinned bot shows only among
 * the pinned rows. A folded section still holds its rows, so its fold can animate.
 */
export function rosterLines<T extends { bot: Placed }>(items: T[], folded: readonly string[]): RosterLine<T>[] {
  const row = (item: T): RosterLine<T> => ({ kind: 'row', item })
  const unpinned = items.filter((item) => !item.bot.pinned)
  const sections = sectionNames(unpinned.map((item) => item.bot)).map((name): RosterLine<T> => {
    const rows = unpinned.filter((item) => item.bot.section === name)
    const unread = rows.reduce((sum, item) => sum + item.bot.unread, 0)
    return { kind: 'section', name, count: rows.length, unread, folded: folded.includes(name), items: rows }
  })
  return [...items.filter((item) => item.bot.pinned).map(row), ...sections, ...unpinned.filter((item) => !item.bot.section).map(row)]
}

/** The rows on screen, top to bottom: a folded section's rows are left out. */
export function shownRows<T>(lines: RosterLine<T>[]): T[] {
  return lines.flatMap((line) => (line.kind === 'row' ? [line.item] : line.folded ? [] : line.items))
}

/** Whether a drag (or Alt+↑/↓) may move one bot past another: same pin state and, unpinned, the same section. */
export function sameRosterGroup(a: Omit<Placed, 'unread'>, b: Omit<Placed, 'unread'>): boolean {
  return a.pinned === b.pinned && (a.pinned || (a.section || null) === (b.section || null))
}

/** Every section among `bots` once, by name as the user's language sorts it ("İş" too). */
export function sectionNames(bots: Iterable<{ section?: string }>): string[] {
  const names = new Set<string>()
  for (const bot of bots) if (bot.section) names.add(bot.section)
  return [...names].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }))
}
