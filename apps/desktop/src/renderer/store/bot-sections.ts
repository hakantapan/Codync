import type { Bot } from '@shared/models'
import type { HostClient } from '../client/host-client'
import { placedBots, restoredBots, sectionMembers, sectionRename } from '../lib/roster-sections'

/** A host call that, when refused, runs `undo` before the error shows; resolves to whether it went through. */
type Perform = (op: (c: HostClient) => Promise<unknown>, undo: () => void) => Promise<boolean>

/** One computer's roster sections: moves and renames shown at once, then sent; a refusal takes them back. */
export class BotSections {
  private bots: () => Map<string, Bot>
  private changed: () => void
  private perform: Perform
  constructor(bots: () => Map<string, Bot>, changed: () => void, perform: Perform) {
    this.bots = bots
    this.changed = changed
    this.perform = perform
  }

  /** Puts the bot in roster section `name`, or takes it out of its section (null). Sends only the section, so a setting changed on another device meanwhile stays. */
  move(bot: Bot, name: string | null) {
    const section = name?.trim() || null
    void this.moveBots([bot.id], section ?? undefined, (c) => c.call('updateBot', { id: bot.id, section }))
  }

  /** Renames a section on this computer (onto another one's name merges them); null removes it, its bots stay. Resolves to whether the host did it. */
  rename(from: string, to: string | null): Promise<boolean> {
    const rename = sectionRename(from, to)
    return this.moveBots(sectionMembers(this.bots().values(), from), rename.to ?? undefined, (c) => c.call('renameSection', rename))
  }

  /**
   * Shows bots `ids` in section `to` (none: undefined) before the host's bot events confirm it, then
   * sends `op`; a refusal takes back the moves nothing changed since. Resolves to whether the host accepted.
   */
  private moveBots(ids: string[], to: string | undefined, op: (c: HostClient) => Promise<unknown>): Promise<boolean> {
    const moves = ids.map((id) => ({ id, from: this.bots().get(id)?.section, to }))
    this.place(placedBots(this.bots(), ids, to))
    return this.perform(op, () => this.place(restoredBots(this.bots(), moves)))
  }

  private place(placed: [string, Bot][]) {
    const bots = this.bots()
    for (const [id, bot] of placed) bots.set(id, bot)
    this.changed()
  }
}
