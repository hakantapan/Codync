import { useState } from 'react'
import type { Entry } from '@shared/models'
import type { QuestionAnswers, QuestionField } from '@shared/question'
import { Button, Spinner } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { Field } from '../marketplace/pieces'
import { hasAnswer, pick, questionOutcome, sendsOnTap, showsSubmit, startsOpen } from './question-form'
import './permission-card.css'
import './question-card.css'

/** An agent's question (Claude Code's AskUserQuestion) in the approval card's style: one tap answers a single
 *  question; several questions, several picks or a typed answer go with Submit. */
export function QuestionCard({ entry, answering, respond }: { entry: Entry; answering: boolean; respond: (answers: QuestionAnswers | null) => void }) {
  const [answers, setAnswers] = useState<QuestionAnswers>({})
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set())
  const d = entry.data
  const fields = d.fields ?? []
  const pending = d.status === 'pending'
  const textOpen = startsOpen(fields)
  const tap = (field: QuestionField, value: string) =>
    sendsOnTap(fields, answers) ? respond({ ...answers, [field.key]: [value] }) : setAnswers((a) => pick(a, field, value))
  return (
    <div className="agent-inset">
      <VStack gap={10} className="permission-card">
        <HStack gap={8}>
          <Text size="headline" tone="text">{d.title ?? 'A question for you'}</Text>
          {pending ? <span className="permission-card-dot" /> : null}
        </HStack>
        {pending ? (
          <>
            {fields.map((f) =>
              f.kind === 'text' ? (
                <TextAnswer
                  key={f.key}
                  field={f}
                  open={textOpen || opened.has(f.key)}
                  autoFocus={opened.has(f.key)}
                  disabled={answering}
                  value={answers[f.key]?.[0] ?? ''}
                  onOpen={() => setOpened((s) => new Set(s).add(f.key))}
                  onChange={(v) => setAnswers((a) => ({ ...a, [f.key]: [v] }))}
                />
              ) : (
                <Choices key={f.key} field={f} chosen={answers[f.key] ?? []} disabled={answering} onTap={(v) => tap(f, v)} />
              ),
            )}
            <HStack gap={8} className="question-card-actions">
              {answering ? <Spinner size={14} /> : <>
                <Button kind="secondary" onClick={() => respond(null)}>Skip</Button>
                {showsSubmit(fields, answers) ? <Button disabled={!hasAnswer(answers)} onClick={() => respond(answers)}>Submit</Button> : null}
              </>}
            </HStack>
          </>
        ) : (
          <Text as="div" size="caption" weight="medium" tone="secondary" className="question-card-outcome">{questionOutcome(d.status, d.answer)}</Text>
        )}
      </VStack>
    </div>
  )
}

function Choices({ field, chosen, disabled, onTap }: { field: QuestionField; chosen: string[]; disabled: boolean; onTap: (value: string) => void }) {
  return (
    <VStack gap={6}>
      {field.title ? <Text size="caption" weight="semibold" tone="secondary">{field.title}</Text> : null}
      {field.description ? <Text as="div" size="subheadline" tone="text">{field.description}</Text> : null}
      <div className="permission-card-options" role="group" aria-label={field.title ?? undefined}>
        {field.options.map((o, i) => (
          <div key={o.value}>
            {i > 0 ? <div className="thread-hairline" /> : null}
            <button
              className={disabled ? 'permission-option question-option dimmed' : 'permission-option question-option'}
              disabled={disabled}
              aria-pressed={chosen.includes(o.value)}
              onClick={() => onTap(o.value)}
            >
              <VStack gap={2} className="permission-option-label">
                <Text size="body" tone="text">{o.label}</Text>
                {o.description ? <Text size="caption" tone="secondary">{o.description}</Text> : null}
              </VStack>
              {chosen.includes(o.value) ? <Icon name="checkmark" size={12} weight="semibold" /> : null}
            </button>
          </div>
        ))}
      </div>
    </VStack>
  )
}

type TextAnswerProps = { field: QuestionField; open: boolean; autoFocus: boolean; disabled: boolean; value: string; onOpen: () => void; onChange: (v: string) => void }

/** A typed answer: a collapsed "<title>…" row beside choices (focus moves into the box once it opens), the open box alone. */
function TextAnswer({ field, open, autoFocus, disabled, value, onOpen, onChange }: TextAnswerProps) {
  const title = field.title ?? 'Answer'
  if (!open) {
    return (
      <button className="question-text-row" disabled={disabled} onClick={onOpen}>
        <Text size="body" tone="secondary">{`${title}…`}</Text>
      </button>
    )
  }
  return (
    <div className="question-text-field">
      <Field value={value} onChange={onChange} label={title} placeholder={field.description ?? title} lines={[2, 6]} autoFocus={autoFocus} disabled={disabled} spellCheck />
    </div>
  )
}
