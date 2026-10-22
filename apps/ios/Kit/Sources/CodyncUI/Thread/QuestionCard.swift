import CodyncKit
import SwiftUI

/// An agent's question (Claude Code's AskUserQuestion) in the approval card's style: one tap answers a single
/// question; several questions, several picks or a typed answer go with Submit.
struct QuestionCard: View {
    let entry: Entry
    /// Its answers are on their way to the computer.
    var answering: Bool
    let respond: (QuestionAnswers?) -> Void
    @State private var answers: QuestionAnswers = [:]
    @State private var opened: Set<String> = []
    /// The key of the text field being typed in.
    @FocusState private var focused: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var d: EntryData { entry.data }
    private var fields: [QuestionField] { d.fields ?? [] }
    private var pending: Bool { d.status == "pending" }
    /// A single choice with nothing picked yet answers on its tap; Submit would only get in the way.
    private var showsSubmit: Bool { !(QuestionForm.sendsOnTap(fields, answers) && !QuestionForm.hasAnswer(answers)) }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Text(d.title ?? "A question for you").font(.headline).foregroundStyle(Palette.text)
                if pending {
                    Circle().fill(Palette.warning).frame(width: 7, height: 7)
                }
            }
            if pending {
                ForEach(fields) { field in
                    if field.kind == "text" { textAnswer(field) } else { choices(field) }
                }
                actions
            } else {
                Text(QuestionForm.outcome(status: d.status, answer: d.answer))
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Palette.secondary)
            }
        }
        .padding(16)
        .frame(maxWidth: 420, alignment: .leading)
        .background(Palette.bubbleAgent, in: RoundedRectangle(cornerRadius: 22, style: .continuous))
        .padding(.trailing, 40)
    }

    private func choices(_ field: QuestionField) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            if let title = field.title {
                Text(title).font(.caption.weight(.semibold)).foregroundStyle(Palette.secondary)
            }
            if let question = field.description {
                Text(question).font(.subheadline).foregroundStyle(Palette.text)
            }
            VStack(spacing: 0) {
                ForEach(Array((field.options ?? []).enumerated()), id: \.element.id) { i, option in
                    let isChosen = answers[field.key]?.contains(option.value) == true
                    if i > 0 { Rectangle().fill(Palette.border).frame(height: 0.5) }
                    Button { tap(field, option.value) } label: {
                        HStack(spacing: 8) {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(option.label).font(.body).foregroundStyle(Palette.text)
                                if let detail = option.description {
                                    Text(detail).font(.caption).foregroundStyle(Palette.secondary)
                                }
                            }
                            Spacer(minLength: 0)
                            if isChosen {
                                Image(systemName: "checkmark").font(.footnote.weight(.semibold)).foregroundStyle(Palette.accent)
                            }
                        }
                        .frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 6)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityAddTraits(isChosen ? .isSelected : [])
                    .disabled(answering)
                    .opacity(answering ? 0.4 : 1)
                }
            }
            .background(Palette.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    /// A typed answer: a "<title>…" row beside choices (focus moves into the box once it opens), the open box alone.
    @ViewBuilder private func textAnswer(_ field: QuestionField) -> some View {
        let title = field.title ?? "Answer"
        if QuestionForm.startsOpen(fields) || opened.contains(field.key) {
            TextField(field.description ?? title, text: typed(field), axis: .vertical)
                .lineLimit(2...6)
                .textFieldStyle(.plain)
                .focused($focused, equals: field.key)
                .disabled(answering)
                .accessibilityLabel(title)
                .padding(10)
                .background(Palette.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                .transition(.opacity.combined(with: .move(edge: .top)))
        } else {
            Button {
                Motion.animate { _ = opened.insert(field.key) }
                focused = field.key
            } label: {
                Text("\(title)…").font(.body).foregroundStyle(Palette.secondary)
            }
            .buttonStyle(.plain)
            .disabled(answering)
        }
    }

    private var actions: some View {
        HStack(spacing: 8) {
            Spacer(minLength: 0)
            if answering {
                Spinner(size: 14)
            } else {
                Button("Skip") { respond(nil) }.buttonStyle(SecondaryButtonStyle())
                if showsSubmit {
                    Button("Submit") { respond(answers) }
                        .buttonStyle(PrimaryButtonStyle())
                        .disabled(!QuestionForm.hasAnswer(answers))
                }
            }
        }
        // Typing the first character turns a one-tap card into one with Submit.
        .animation(Motion.reduced(Motion.layout, reduceMotion), value: showsSubmit)
    }

    private func typed(_ field: QuestionField) -> Binding<String> {
        Binding { answers[field.key]?.first ?? "" } set: { answers[field.key] = [$0] }
    }

    private func tap(_ field: QuestionField, _ value: String) {
        if QuestionForm.sendsOnTap(fields, answers) {
            respond(answers.merging([field.key: [value]]) { $1 })
        } else {
            Motion.animate { answers = QuestionForm.pick(answers, field, value) }
        }
    }
}
