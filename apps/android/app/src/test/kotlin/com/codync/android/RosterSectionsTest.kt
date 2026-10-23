package com.codync.android

import com.codync.android.core.Bot
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import org.junit.Assert.*
import org.junit.Test

class RosterSectionsTest {
    private val pinned = Bot("pin", pinned = true, section = "Work", unread = 7)
    private val work1 = Bot("w1", section = "Work", unread = 2)
    private val loose = Bot("loose")
    private val personal = Bot("p1", section = "Personal")
    private val work2 = Bot("w2", section = "Work", unread = 3)
    private val roster = listOf(pinned, work1, loose, personal, work2)

    @Test fun pinnedBotsComeFirstThenEachSectionByNameThenBotsWithoutOne() {
        assertEquals(listOf(
            RosterLine.Row(pinned),
            RosterLine.Header("Personal", count = 1, unread = 0, folded = false), RosterLine.Row(personal),
            RosterLine.Header("Work", count = 2, unread = 5, folded = false), RosterLine.Row(work1), RosterLine.Row(work2),
            RosterLine.Row(loose),
        ), rosterLines(roster, folded = emptySet()))
    }

    @Test fun aFoldedSectionKeepsItsHeaderAndUnreadAndHidesItsBots() {
        assertEquals(listOf(
            RosterLine.Row(pinned),
            RosterLine.Header("Personal", count = 1, unread = 0, folded = false), RosterLine.Row(personal),
            RosterLine.Header("Work", count = 2, unread = 5, folded = true),
            RosterLine.Row(loose),
        ), rosterLines(roster, folded = setOf("Work")))
    }

    @Test fun sectionsSortTheWayTheLanguageDoesNotByCharacterCode() {
        val bots = listOf(Bot("a", section = "fig"), Bot("b", section = "Éclair"), Bot("c", section = "Cherry"), Bot("d", section = "banana"))
        val sorted = listOf("banana", "Cherry", "Éclair", "fig")
        assertEquals(sorted, sectionNames(bots))
        assertEquals(sorted, rosterLines(bots, emptySet()).filterIsInstance<RosterLine.Header>().map { it.name })
    }

    @Test fun withoutSectionsTheRosterShowsAsItIs() {
        val plain = listOf(Bot("pin", pinned = true), Bot("a"), Bot("b"))
        assertEquals(plain.map { RosterLine.Row(it) }, rosterLines(plain, emptySet()))
    }

    @Test fun clearingASectionSendsNullRatherThanLeavingTheKeyOut() {
        assertEquals(buildJsonObject { put("id", "b"); put("section", "Work") }, sectionUpdate("b", "Work"))
        assertEquals(JsonNull, sectionUpdate("b", null)["section"])
        assertEquals(buildJsonObject { put("from", "Work"); put("to", "Jobs") }, sectionRename("Work", "Jobs"))
        assertEquals(JsonNull, sectionRename("Work", null)["to"])
    }

    @Test fun aTypedSectionNameIsTrimmedAndANameOfNothingIsNoName() {
        assertEquals("Work", sectionName("  Work \n"))
        assertEquals("Side projects", sectionName("Side projects"))
        listOf("", "   ", "\t\n", "\u00A0", "\u0000", " \u0000 \t").forEach { assertNull(it, sectionName(it)) }
    }

    @Test fun aNameFieldStopsAtFortyCharactersAsTheHostCountsThemNeverInsideAnEmoji() {
        val forty = "a".repeat(40)
        assertEquals(forty, limitSectionName(forty))
        assertEquals(forty, limitSectionName(forty + "b"))
        // An emoji is two UTF-16 units but one character: 40 of them fit, a 41st is cut whole.
        val emoji = "\uD83D\uDE00"
        assertEquals(emoji.repeat(40), limitSectionName(emoji.repeat(40)))
        assertEquals(emoji.repeat(40), limitSectionName(emoji.repeat(41)))
        assertEquals("a".repeat(39) + emoji, limitSectionName("a".repeat(39) + emoji + "b"))
    }

    @Test fun sectionControlsWaitForAHostThatKeepsSections() {
        fun host(version: String?) = AppState(hello = version?.let { buildJsonObject { put("version", it) } })
        assertFalse(host("2.23.0").supportsSections)
        assertTrue(host("2.24.0").supportsSections)
        assertTrue(host("2.24.1-dev").supportsSections)
        assertFalse(host(null).supportsSections)
    }
}
