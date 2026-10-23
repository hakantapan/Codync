// Wire types for question cards (`question` entries, `respondQuestion`); see host/src/agent/bot/question_form.rs.

/** One choice of a question card's field. */
export interface QuestionOption {
  value: string
  label: string
  description?: string | null
}

/** One field of an agent's question form (`question` entries). */
export interface QuestionField {
  key: string
  /** single | multi | text */
  kind: string
  title?: string | null
  description?: string | null
  options: QuestionOption[]
}

/** What the user picked on a question card, by field key: chosen option values, or the typed text. */
export type QuestionAnswers = Record<string, string[]>
