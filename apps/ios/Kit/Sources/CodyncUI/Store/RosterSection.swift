import CodyncKit
import Foundation

/// A user-made section's heading in a computer's roster ("Work"): how many of its bots it lists,
/// their unread messages (shown while it's folded) and whether it's folded on this device.
public struct RosterSection: Sendable {
    public let name: String
    public let count: Int
    public let unread: Int
    public let folded: Bool

    /// Section names as the roster lists them: the way the user's language sorts ("İş" in Turkish),
    /// "Work" before "work" so two names that differ only in case keep one order.
    static func sorted(_ names: some Sequence<String>) -> [String] {
        names.sorted {
            let order = $0.localizedStandardCompare($1)
            return order == .orderedSame ? $0 < $1 : order == .orderedAscending
        }
    }

    /// A computer's roster (in `BotStore.roster` order) as the rows on screen: pinned bots, then
    /// each section by name with its bots (none while folded), then the bots in no section. A
    /// pinned bot shows only on top, whatever its section; every part keeps the roster's order.
    public static func lines<Item>(_ roster: [Item], folded: Set<String>, bot: (Item) -> Bot) -> [RosterSectionLine<Item>] {
        var pinned: [Item] = [], loose: [Item] = [], bySection: [String: [Item]] = [:]
        for item in roster {
            let placed = bot(item)
            if placed.pinned {
                pinned.append(item)
            } else if let name = placed.section {
                bySection[name, default: []].append(item)
            } else {
                loose.append(item)
            }
        }
        var lines = pinned.map { RosterSectionLine.bot($0) }
        for name in sorted(bySection.keys) {
            let items = bySection[name] ?? []
            let isFolded = folded.contains(name)
            let unread = items.reduce(0) { $0 + bot($1).unread }
            lines.append(.header(RosterSection(name: name, count: items.count, unread: unread, folded: isFolded)))
            if !isFolded { lines += items.map { RosterSectionLine.bot($0) } }
        }
        return lines + loose.map { RosterSectionLine.bot($0) }
    }
}

/// One row of a computer's roster split into sections.
public enum RosterSectionLine<Item> {
    case header(RosterSection)
    case bot(Item)
}
