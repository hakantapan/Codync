import type { QuestionAnswers, QuestionField } from '../../../shared/question.ts'

const typed = (fields: QuestionField[], answers: QuestionAnswers) =>
  fields.some((f) => f.kind === 'text' && (answers[f.key]?.[0] ?? '').trim() !== '')

/** Whether one tap on an option answers the card: a single one-choice question with nothing typed. */
export function sendsOnTap(fields: QuestionField[], answers: QuestionAnswers): boolean {
  const choices = fields.filter((f) => f.kind !== 'text')
  return choices.length === 1 && choices[0]!.kind === 'single' && !typed(fields, answers)
}

/** Whether the text boxes start open: a text field collapses to an "Other…" row only beside choices; alone it is the question. */
export const startsOpen = (fields: QuestionField[]) => fields.every((f) => f.kind === 'text')

/** `answers` after tapping `value` on `field`: a single choice replaces, a multi choice toggles. */
export function pick(answers: QuestionAnswers, field: QuestionField, value: string): QuestionAnswers {
  const current = answers[field.key] ?? []
  const next = field.kind !== 'multi' ? [value] : current.includes(value) ? current.filter((v) => v !== value) : [...current, value]
  return { ...answers, [field.key]: next }
}

/** Whether Submit has anything to send. */
export const hasAnswer = (answers: QuestionAnswers) => Object.values(answers).some((v) => v.some((s) => s.trim() !== ''))

/** Whether the card shows Submit: always, except on a one-tap card with nothing typed. */
export const showsSubmit = (fields: QuestionField[], answers: QuestionAnswers) => !sendsOnTap(fields, answers) || hasAnswer(answers)

/** The answered card's line: what was picked or typed, or how it ended. */
export function questionOutcome(status: string | undefined, answer: string[] | null | undefined): string {
  switch (status) {
    case 'answered':
      return (answer ?? []).map((a) => `✓ ${a}`).join('\n') || 'Answered'
    case 'declined':
      return 'Skipped'
    case 'cancelled':
      return 'Cancelled'
    default:
      return 'Expired — the agent moved on'
  }
}
