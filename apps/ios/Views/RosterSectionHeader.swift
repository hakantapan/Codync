import CodyncKit
import CodyncUI
import SwiftUI

/// A user-made section's heading in the roster, in `ComputerSection`'s look (name, bot count, a
/// trailing chevron): tap folds its bots away on this device; folded, it shows how many unread
/// messages its bots hold.
struct RosterSectionHeader: View {
    let section: RosterSection
    let toggle: () -> Void
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var showsUnread: Bool { section.folded && section.unread > 0 }

    var body: some View {
        HStack(spacing: 6) {
            // The name gives way first: the count, badge and chevron keep their room.
            Text(section.name)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Palette.secondary)
                .lineLimit(1)
            Text("\(section.count)")
                .font(.footnote)
                .foregroundStyle(Palette.tertiary)
                .layoutPriority(1)
            Spacer(minLength: 8)
            // Fades however it changes: a message arriving while folded isn't animated by the store.
            ZStack {
                if showsUnread { UnreadBadge(count: section.unread).transition(.opacity) }
            }
            .animation(Motion.reduced(Motion.fade, reduceMotion), value: showsUnread)
            .layoutPriority(1)
            Image(systemName: "chevron.down")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Palette.tertiary)
                .rotationEffect(.degrees(section.folded ? -90 : 0))
                .accessibilityHidden(true)
        }
        .padding(.top, 12)
        .padding(.bottom, 4)
        .contentShape(Rectangle())
        .onTapGesture(perform: toggle)
        .accessibilityElement(children: .ignore)
        .accessibilityAddTraits([.isHeader, .isButton])
        .accessibilityLabel(spoken)
        .accessibilityValue(section.folded ? "Collapsed" : "Expanded")
        .accessibilityHint("Double-tap to fold or expand. Touch and hold to rename or remove the section.")
        .accessibilityAction(named: section.folded ? "Expand" : "Collapse", toggle)
        .accessibilityAction(.default) { toggle() }
    }

    /// "Work, 2 bots, 3 unread": what the count and the badge stand for.
    private var spoken: String {
        let bots = section.count == 1 ? "1 bot" : "\(section.count) bots"
        let unread = showsUnread ? ", \(section.unread) unread" : ""
        return "\(section.name), \(bots)\(unread)"
    }
}
