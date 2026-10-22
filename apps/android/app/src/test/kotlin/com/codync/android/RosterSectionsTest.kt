package com.codync.android

import com.codync.android.core.Bot
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
}
