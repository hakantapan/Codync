import Foundation
import Testing
@testable import CodyncKit

private let db = QuestionField(key: "question_0", kind: "single", title: "Database", options: [
    QuestionOption(value: "PostgreSQL", label: "PostgreSQL"), QuestionOption(value: "SQLite", label: "SQLite"),
])
private let other = QuestionField(key: "question_0_custom", kind: "text", title: "Other", options: [])
private let extras = QuestionField(key: "question_1", kind: "multi", title: "Extras", options: [
    QuestionOption(value: "Cache", label: "Cache"), QuestionOption(value: "Queue", label: "Queue"),
])

@Test func questionEntryDecodesAndShowsInTheChat() throws {
    let wire = #"{"id":"q","seq":1,"botId":"bot","rev":1,"kind":"question","turn":1,"createdAt":0,"updatedAt":0,"data":{"title":"Which database?","status":"answered","answer":["PostgreSQL"],"fields":[{"key":"question_0","kind":"single","title":"Database","description":null,"options":[{"value":"PostgreSQL","label":"PostgreSQL","description":"Relational"}]},{"key":"question_0_custom","kind":"text","title":"Other","description":null,"options":[]}]}}"#
    let entry = try JSONDecoder().decode(Entry.self, from: Data(wire.utf8))
    #expect(entry.isChat)
    #expect(!entry.isReplyable)
    #expect(entry.data.fields?.map(\.kind) == ["single", "text"])
    #expect(entry.data.fields?.first?.options?.first?.description == "Relational")
    #expect(entry.data.answer == ["PostgreSQL"])
}

@Test func aPendingQuestionDecodesWithNoAnswerTitleOrOptions() throws {
    let wire = #"{"id":"q","seq":1,"botId":"bot","rev":1,"kind":"question","turn":1,"createdAt":0,"updatedAt":0,"data":{"title":"Which database?","status":"pending","answer":null,"fields":[{"key":"question_0_custom","kind":"text","title":null,"description":null,"options":[]}]}}"#
    let entry = try JSONDecoder().decode(Entry.self, from: Data(wire.utf8))
    #expect(entry.isChat)
    #expect(entry.data.answer == nil)
    #expect(entry.data.fields?.first?.options?.isEmpty == true)
    #expect(QuestionForm.outcome(status: "answered", answer: []) == "Answered")
}

@Test func oneSingleChoiceQuestionSendsOnTapUntilSomethingIsTyped() {
    #expect(QuestionForm.sendsOnTap([db, other], [:]))
    #expect(QuestionForm.sendsOnTap([db, other], ["question_0_custom": ["  "]]))
    #expect(!QuestionForm.sendsOnTap([db, other], ["question_0_custom": ["MySQL"]]))
    #expect(!QuestionForm.sendsOnTap([db, other, extras], [:]))
    #expect(!QuestionForm.sendsOnTap([extras], [:]))
}

@Test func aTextFieldStartsOpenOnlyWhenThereAreNoChoices() {
    #expect(!QuestionForm.startsOpen([db, other]))
    #expect(QuestionForm.startsOpen([other]))
    #expect(QuestionForm.startsOpen([other, QuestionField(key: "question_1_custom", kind: "text", title: "Other", options: [])]))
}

@Test func aSingleChoiceReplacesAndAMultiChoiceToggles() {
    #expect(QuestionForm.pick(["question_0": ["SQLite"]], db, "PostgreSQL") == ["question_0": ["PostgreSQL"]])
    let once = QuestionForm.pick([:], extras, "Cache")
    #expect(QuestionForm.pick(once, extras, "Queue") == ["question_1": ["Cache", "Queue"]])
    #expect(QuestionForm.pick(once, extras, "Cache") == ["question_1": []])
}

@Test func submitNeedsAPickOrTypedText() {
    #expect(!QuestionForm.hasAnswer([:]))
    #expect(!QuestionForm.hasAnswer(["question_1": [], "question_0_custom": [" "]]))
    #expect(QuestionForm.hasAnswer(["question_1": ["Cache"]]))
}

@Test func submitShowsExceptOnAOneTapCardWithNothingTyped() {
    #expect(!QuestionForm.showsSubmit([db, other], [:]))
    #expect(QuestionForm.showsSubmit([db, other], ["question_0_custom": ["MySQL"]]))
    #expect(QuestionForm.showsSubmit([db, other, extras], [:]))
    #expect(QuestionForm.showsSubmit([other], [:]))
}

@Test func theAnsweredCardShowsEachAnswer() {
    #expect(QuestionForm.outcome(status: "answered", answer: ["PostgreSQL", "Cache, Queue"]) == "✓ PostgreSQL\n✓ Cache, Queue")
    #expect(QuestionForm.outcome(status: "declined", answer: nil) == "Skipped")
    #expect(QuestionForm.outcome(status: "cancelled", answer: nil) == "Cancelled")
    #expect(QuestionForm.outcome(status: "expired", answer: nil) == "Expired — the agent moved on")
}
