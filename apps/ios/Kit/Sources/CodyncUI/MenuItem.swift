import SwiftUI

/// One row in a Codync menu: an action, or a choice when `selected` is set.
public struct MenuItem: Identifiable {
    public let id = UUID()
    public var title: String
    public var icon: String?
    /// A picture in the icon's place (an agent's logo).
    public var image: Image?
    public var selected: Bool?
    public var destructive: Bool
    public var divider: Bool
    public var action: () -> Void

    public init(_ title: String, icon: String? = nil, image: Image? = nil, selected: Bool? = nil, destructive: Bool = false,
                divider: Bool = false, action: @escaping () -> Void) {
        self.title = title
        self.icon = icon
        self.image = image
        self.selected = selected
        self.destructive = destructive
        self.divider = divider
        self.action = action
    }
}

/// A row of emoji above a message's menu (Slack's quick reactions).
public struct ReactionPick {
    public var emoji: [String]
    public var chosen: [String]
    public var toggle: (String) -> Void

    public init(emoji: [String], chosen: [String], toggle: @escaping (String) -> Void) {
        self.emoji = emoji
        self.chosen = chosen
        self.toggle = toggle
    }
}

public extension View {
    /// Long-press opens the system context menu; `reactions` puts a quick-reaction row on top.
    func contextActions(reactions: ReactionPick? = nil, _ items: @escaping () -> [MenuItem]) -> some View {
        modifier(ContextActions(items: items, reactions: reactions))
    }
}

private struct ContextActions: ViewModifier {
    let items: () -> [MenuItem]
    let reactions: ReactionPick?

    func body(content: Content) -> some View {
        content.contextMenu {
            if let reactions {
                ControlGroup {
                    ForEach(reactions.emoji, id: \.self) { emoji in
                        let chosen = reactions.chosen.contains(emoji)
                        Toggle(isOn: Binding(get: { chosen }, set: { _ in reactions.toggle(emoji) })) { Text(emoji) }
                    }
                }
                .controlGroupStyle(.palette)
            }
            ForEach(items()) { item in
                if item.divider { Divider() }
                systemMenuRow(item)
            }
        }
    }
}

/// A `MenuItem` inside a system `Menu` or `contextMenu`: choices as checkmarked toggles.
@ViewBuilder private func systemMenuRow(_ item: MenuItem) -> some View {
    let role: ButtonRole? = item.destructive ? .destructive : nil
    if let selected = item.selected {
        // The binding's setter must be Sendable, but SwiftUI only calls it from the menu, on the
        // main thread where the action was made.
        nonisolated(unsafe) let action = item.action
        Toggle(isOn: Binding(get: { selected }, set: { _ in action() })) { menuLabel(item) }
    } else {
        Button(role: role, action: item.action) { menuLabel(item) }
    }
}

@ViewBuilder private func menuLabel(_ item: MenuItem) -> some View {
    if let image = item.image {
        SwiftUI.Label { Text(item.title) } icon: { image }
    } else if let icon = item.icon {
        SwiftUI.Label(item.title, systemImage: icon)
    } else {
        Text(item.title)
    }
}

/// A button that opens the system menu.
public struct DropdownMenu<Label: View>: View {
    let items: () -> [MenuItem]
    let label: Label

    public init(items: @escaping () -> [MenuItem], @ViewBuilder label: () -> Label) {
        self.items = items
        self.label = label()
    }

    public var body: some View {
        // The system menu: it anchors correctly inside sheets and scroll views, where an overlay can't.
        Menu {
            ForEach(items()) { item in
                if item.divider { Divider() }
                systemMenuRow(item)
            }
        } label: { label }
            .buttonStyle(.plain)
            .menuIndicator(.hidden)
    }
}
