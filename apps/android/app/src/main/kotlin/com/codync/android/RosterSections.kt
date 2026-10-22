package com.codync.android

import android.content.Context
import androidx.core.content.edit
import com.codync.android.core.AppVersion
import com.codync.android.core.Bot
import java.text.Collator
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

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

/** The first host version that keeps sections (`section` on bots, `renameSection`). */
private const val SECTIONS_HOST = "2.23.0"

/** Section controls show only for a host that keeps sections; before its `hello` (offline) they stay hidden. */
internal val AppState.supportsSections: Boolean
    get() = hello?.text("version")?.let { !AppVersion.isBelow(it, SECTIONS_HOST) } == true

/** `updateBot`'s section patch: null goes as JSON null, which clears it (a missing key keeps it). */
internal fun sectionUpdate(id: String, section: String?): JsonObject = buildJsonObject { put("id", id); put("section", section) }

/** `renameSection`: every bot in [from] moves to [to], merging into a section of that name; null removes the section. */
internal fun sectionRename(from: String, to: String?): JsonObject = buildJsonObject { put("from", from); put("to", to) }

/** The sections folded on this device (iOS `@AppStorage("collapsedRosterSections")`); not synced. */
internal object FoldedSections {
    private const val KEY = "collapsedRosterSections"
    private fun prefs(context: Context) = context.getSharedPreferences("codync-roster", Context.MODE_PRIVATE)
    fun load(context: Context): Set<String> = prefs(context).getStringSet(KEY, null).orEmpty().toSet()
    fun save(context: Context, names: Set<String>) { prefs(context).edit { putStringSet(KEY, names) } }
}
