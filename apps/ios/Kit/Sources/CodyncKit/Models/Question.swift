import Foundation

/// One field of an agent's question form (a `question` entry): pick one option, pick several, or type.
public struct QuestionField: Codable, Hashable, Sendable, Identifiable {
    public var key: String
    /// single | multi | text
    public var kind: String
    public var title: String?
    public var description: String?
    public var options: [QuestionOption]?
    public var id: String { key }

    public init(key: String, kind: String, title: String? = nil, description: String? = nil, options: [QuestionOption]) {
        self.key = key
        self.kind = kind
        self.title = title
        self.description = description
        self.options = options
    }
}

/// One choice of a question card's field.
public struct QuestionOption:Codable, Hashable, Sendable, Identifiable {
    public var value: String
    public var label: String
    public var description: String?
    public var id: String { value }

    public init(value: String, label: String, description: String? = nil) {
        self.value = value
        self.label = label
        self.description = description
    }
}

/// What the user picked on a question card, by field key: chosen option values, or the typed text.
public typealias QuestionAnswers = [String: [String]]

/// A question card's rules, shared by its view and its tests.
public enum QuestionForm {
    /// Whether one tap on an option answers the card: a single one-choice question with nothing typed.
    public static func sendsOnTap(_ fields: [QuestionField], _ answers: QuestionAnswers) -> Bool {
        let choices = fields.filter { $0.kind != "text" }
        let typed = fields.contains { $0.kind == "text" && !(answers[$0.key]?.first ?? "").trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
        return choices.count == 1 && choices[0].kind == "single" && !typed
    }

    /// Whether the text boxes start open: a text field collapses to an "Other…" row only beside choices; alone it is the question.
    public static func startsOpen(_ fields: [QuestionField]) -> Bool {
        fields.allSatisfy { $0.kind == "text" }
    }

    /// `answers` after tapping `value` on `field`: a single choice replaces, a multi choice toggles.
    public static func pick(_ answers: QuestionAnswers, _ field: QuestionField, _ value: String) -> QuestionAnswers {
        var next = answers
        let current = answers[field.key] ?? []
        next[field.key] = field.kind != "multi" ? [value]
            : current.contains(value) ? current.filter { $0 != value } : current + [value]
        return next
    }

    /// Whether Submit has anything to send.
    public static func hasAnswer(_ answers: QuestionAnswers) -> Bool {
        answers.values.contains { $0.contains { !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty } }
    }

    /// The answered card's line: what was picked or typed, or how it ended.
    public static func outcome(status: String?, answer: [String]?) -> String {
        switch status {
        case "answered":
            let lines = (answer ?? []).map { "✓ \($0)" }.joined(separator: "\n")
            return lines.isEmpty ? "Answered" : lines
        case "declined": return "Skipped"
        case "cancelled": return "Cancelled"
        default: return "Expired — the agent moved on"
        }
    }
}
