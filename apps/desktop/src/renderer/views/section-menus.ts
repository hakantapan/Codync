import type { Bot } from '@shared/models'
import type { MenuItem } from '../components/Overlay'
import { sectionNames } from '../lib/roster-sections'
import type { BotStore } from '../store/bot-store'
import type { ActionItem } from './PanelMenus'

/** An open section name dialog: its title, the name it starts with, and what Save does with the typed one. */
export interface SectionNaming {
  title: string
  name: string
  save: (name: string) => void
}

/** A bot row's "Move to section"; none while its computer runs a host without sections. */
export function moveToSectionItems(bot: Bot, store: BotStore, askName: (naming: SectionNaming) => void): ActionItem[] {
  if (!store.hasRosterSections) return []
  const others = sectionNames(store.bots.values()).filter((name) => name !== bot.section)
  const submenu: MenuItem[] = [
    ...others.map((name) => ({ title: name, icon: 'tray', action: () => store.setSection(bot, name) })),
    { title: 'New section…', icon: 'plus', divider: others.length > 0, action: () => askName({ title: 'New section', name: '', save: (name) => store.setSection(bot, name) }) },
    ...(bot.section ? [{ title: 'Remove from section', icon: 'minus.circle', action: () => store.setSection(bot, null) }] : []),
  ]
  return [{ title: 'Move to section', icon: 'tray', submenu }]
}
