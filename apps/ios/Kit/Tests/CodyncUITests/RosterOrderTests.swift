import Foundation
import Testing
@testable import CodyncKit
@testable import CodyncUI

@MainActor @Test func reorderingKeepsPinnedBotsOnTop() throws {
    let suite = "RosterOrderTests.\(UUID().uuidString)"
    defer { UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite) }
    let storage = SharedStore.Context(accountID: UUID().uuidString, suite: suite)
    let store = BotStore(computer: Computer(id: UUID().uuidString, name: "Mac", signKey: "test"), storage: storage) { _ in
        FakeRemote(.connecting)
    }
    defer { store.retire() }
    for (id, pinned, lastAt) in [("p", true, 0), ("a", false, 3), ("b", false, 2), ("c", false, 1)] {
        let json = #"{"id":"\#(id)","pinned":\#(pinned),"lastAt":\#(lastAt)}"#
        store.bots[id] = try JSONDecoder().decode(Bot.self, from: Data(json.utf8))
    }
    let order = { store.roster.map(\.id) }
    #expect(order() == ["p", "a", "b", "c"]) // Never arranged: recent activity.
    store.reorder(order()) // A drop in place: nothing to save or send.
    #expect(store.bots.values.allSatisfy { $0.position == 0 })
    store.reorder(["p", "c", "a", "b"])
    #expect(order() == ["p", "c", "a", "b"])
    store.move("a", by: -1)
    #expect(order() == ["p", "a", "c", "b"])
    store.move("a", by: -1) // An unpinned bot stays below the pinned ones.
    #expect(order() == ["p", "a", "c", "b"])
    store.reorder(["c", "p", "a", "b"]) // Dropped above a pinned bot: still listed below it.
    #expect(order() == ["p", "c", "a", "b"])
}

/// A bot as the host sends it.
private func rosterBot(_ id: String, pinned: Bool = false, hidden: Bool = false, section: String? = nil, unread: Int = 0,
                       lastAt: Int = 0) throws -> Bot {
    var json: [String: Any] = ["id": id, "pinned": pinned, "hidden": hidden, "unread": unread, "lastAt": lastAt]
    if let section { json["section"] = section }
    return try JSONDecoder().decode(Bot.self, from: JSONSerialization.data(withJSONObject: json))
}

/// A store holding `bots` that never connects; `done` retires it and drops its defaults.
@MainActor private func rosterStore(_ bots: [Bot]) -> (store: BotStore, done: () -> Void) {
    let suite = "RosterOrderTests.\(UUID().uuidString)"
    let storage = SharedStore.Context(accountID: UUID().uuidString, suite: suite)
    let store = BotStore(computer: Computer(id: UUID().uuidString, name: "Mac", signKey: "test"), storage: storage) { _ in
        FakeRemote(.connecting)
    }
    for item in bots { store.bots[item.id] = item }
    return (store, {
        store.retire()
        UserDefaults(suiteName: suite)?.removePersistentDomain(forName: suite)
    })
}

@Test func sectionsListPinnedBotsThenSectionsByNameThenTheRest() throws {
    let roster = try [
        rosterBot("p", pinned: true, section: "Work"),
        rosterBot("w1", section: "Work", unread: 2),
        rosterBot("loose"),
        rosterBot("i", section: "İş"),
        rosterBot("h", section: "home"),
        rosterBot("w2", section: "Work", unread: 3),
    ]
    func shown(_ folded: Set<String>) -> [String] {
        RosterSection.lines(roster, folded: folded) { $0 }.map { line in
            switch line {
            case let .header(section): "\(section.folded ? "▸" : "▾") \(section.name) \(section.count) \(section.unread)"
            case let .bot(item): item.id
            }
        }
    }
    // A pinned bot shows only on top; its section counts the rest.
    #expect(shown([]) == ["p", "▾ home 1 0", "h", "▾ İş 1 0", "i", "▾ Work 2 5", "w1", "w2", "loose"])
    #expect(shown(["Work"]) == ["p", "▾ home 1 0", "h", "▾ İş 1 0", "i", "▸ Work 2 5", "loose"])
    // A section only pinned bots are in has no heading.
    #expect(RosterSection.lines([roster[0]], folded: []) { $0 }.count == 1)
}

@MainActor @Test func sectionMenuListsEverySectionOnTheComputer() throws {
    let (store, done) = try rosterStore([
        rosterBot("a", section: "Work"), rosterBot("p", pinned: true, section: "Pinned"),
        rosterBot("h", hidden: true, section: "Hidden"), rosterBot("b", section: "Work"), rosterBot("c"),
    ])
    defer { done() }
    #expect(store.sections == ["Hidden", "Pinned", "Work"]) // Hidden and pinned bots keep theirs.
}
