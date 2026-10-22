package com.codync.android

import com.codync.android.core.Bot
import java.text.Collator

/** One line of the roster as it shows: a bot, or a section's header (docs/plans/roster-sections.md). */
internal sealed interface RosterLine {
    val key: String

    data class Row(val bot: Bot) : RosterLine {
        override val key: String get() = bot.id
    }

    /** [count] and [unread] cover the section's unpinned bots, the ones shown under it. */
    data class Header(val name: String, val count: Int, val unread: Int, val folded: Boolean) : RosterLine {
        override val key: String get() = "section:$name"
    }
}

/**
 * The sorted [roster] as it shows: pinned bots, then each section by name (its header, then its bots
 * in roster order unless it's [folded]), then bots without a section. A pinned bot shows only among the pinned.
 */
internal fun rosterLines(roster: List<Bot>, folded: Set<String>): List<RosterLine> = buildList {
    val (pinned, unpinned) = roster.partition(Bot::pinned)
    val sections = unpinned.groupBy(Bot::section)
    pinned.forEach { add(RosterLine.Row(it)) }
    for (name in sectionNames(unpinned)) {
        val bots = sections.getValue(name)
        val header = RosterLine.Header(name, bots.size, bots.sumOf { it.unread }, name in folded)
        add(header)
        if (!header.folded) bots.forEach { add(RosterLine.Row(it)) }
    }
    sections[null].orEmpty().forEach { add(RosterLine.Row(it)) }
}

/** The sections [bots] are in, sorted the way the phone's language sorts ("İş" where Turkish puts it). */
internal fun sectionNames(bots: List<Bot>): List<String> = bots.mapNotNull(Bot::section).distinct().sortedWith(Collator.getInstance())
