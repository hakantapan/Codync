# Question cards: one-tap answers to agent questions

Updated: 2026-10-22. Design, not built yet.

When an agent asks the user a multiple-choice question (Claude Code's `AskUserQuestion`), the chat shows the options as buttons, the way the Claude Code VS Code extension does. One tap answers a single question; multi-select and multi-question forms answer with **Submit**.

Scope: host, desktop app and iPhone app, the two clients this fork's owner uses (their call, 2026-10-22). Android and the TUI are deliberately left for a follow-up and keep a working fallback: the user answers with a normal message (see [Text fallback](#text-fallback)).

## Why questions arrive as plain text today

Codync's `initialize` ([launch.rs](../../host/src/agent/bot/launch.rs)) declares no `elicitation` client capability. `@agentclientprotocol/claude-agent-acp` then adds `AskUserQuestion` to `disallowedTools`, so Claude writes its questions as text. With `"elicitation": {"form": {}}` declared, the adapter sends each question as an ACP `elicitation/create` request instead, which [updates.rs](../../host/src/agent/bot/updates.rs) answers today with `-32601`.

## Protocol from the agent

`elicitation/create` request (built by the adapter's `askUserQuestionsToCreateRequest`):

```json
{"mode": "form", "sessionId": "…", "toolCallId": "…",
 "message": "Which database should we use?",
 "requestedSchema": {"type": "object", "properties": {
   "question_0": {"type": "string", "title": "Database",
                  "oneOf": [{"const": "PostgreSQL", "title": "PostgreSQL", "description": "Relational"}]},
   "question_0_custom": {"type": "string", "title": "Other", "description": "Type your own answer…"}}}}
```

- A single question puts its text in `message`; several put `"Please answer the following questions."` there and each question's text in its field's `description`.
- Multi-select: `{"type": "array", "items": {"anyOf": [...]}}`.
- Reply: `{"action": "accept", "content": {"question_0": "PostgreSQL"}}` (an array of labels for multi-select), `{"action": "decline"}` (the model is told the user skipped), or `{"action": "cancel"}` (the tool call is aborted).

Declaring the capability also forwards MCP servers' form elicitations through the same request; they get the same card when their fields fit the supported kinds.

## Host

1. **Capability.** `launch.rs` adds `"elicitation": {"form": {}}` to `clientCapabilities`.
2. **Request.** `on_incoming` routes `elicitation/create` during a turn to a new `on_elicit`. It is never auto-answered, including under `Permission::Auto`: a question has no safe default. Outside a turn it keeps the `-32601` reply. A routine run or an ask whose reply goes back to another bot has nobody to answer, so its question is declined at once and posts no card; group turns, and bot requests that report in the recipient's own chat, still get cards, since the user reads them there (as approval cards already go to the user).
3. **Schema → fields.** A new `host/src/agent/bot/question_form.rs` turns `requestedSchema` into a flat list, so no client parses JSON Schema:

   | Schema property | `kind` |
   |---|---|
   | `type: string` with `oneOf` (or `enum`) | `single` |
   | `type: array` with `items.anyOf` (or `items.enum`) | `multi` |
   | `type: string`, no choices | `text` |

   Options become `{value: const, label: title ?? const, description}`. Property order is kept (the adapter emits each question before its `_custom` field). A `url` mode request, or any property of another type (number, integer, boolean, object), gets `{"action": "decline"}` and a log line: Claude then asks in text.
4. **Entry.** A new `EntryKind::Question` (wire `"question"`) with data:

   ```json
   {"title": "Which database should we use?", "status": "pending", "answer": null,
    "fields": [{"key": "question_0", "kind": "single", "title": "Database", "description": null,
                "options": [{"value": "PostgreSQL", "label": "PostgreSQL", "description": "Relational"}]},
               {"key": "question_0_custom", "kind": "text", "title": "Other", "description": "…", "options": []}]}
   ```

   It is posted with the existing `show_card`, so the bot shows **Needs you**, the push alert fires and the card sits in the turn's lane. `Answer` gains `Elicit(rpc_id)` beside `Agent` and `Codync`.
5. **Answer.** New API method `respondQuestion {entryId, answers?}` (`answers`: `{field key: [values]}`, chosen option values or the typed text; none skips) (`host/src/api/bots.rs`; like `respondPermission`, the entry names its bot, `data.author` in groups) → `Cmd::Question` → `answer_question`. It replies `{action, content}` to the agent and sets the entry's `status` to `answered`, `declined` or `cancelled` and `answer` to one display line per answered field. A second answer for the same entry finds no pending `Answer` and does nothing.
6. **Cancel.** `answer_permission(id, None)` on an `Elicit` entry (`cancel_turn`, Stop, new session) replies `{"action": "cancel"}` and marks the card `cancelled`.

### Text fallback

A `Cmd::Send` in the bot's own chat while the running turn has a pending question card answers that card (the oldest, if several wait) instead of queueing a new turn: `{"action": "accept", "content": {<first text field>: text}}`. The message still shows as the user's bubble, in the running turn. With several questions only the first question's **Other** gets the text. Only the user's new words answer: a note the host wrote (`from: host`) or a message typed before the question (a held sign-in message sent again) leaves the card waiting. A card with no `text` field, or a message with files (the agent would get only the text), gets `decline` and the message queues as usual. In a room the fallback does not apply: room messages travel as group turns, not `Cmd::Send`, so a question card there is answered on the card, or the room's ask times out. This keeps Android, the TUI and older app versions from leaving a bot blocked on a card they can't show.

## Clients (desktop and iPhone)

New `QuestionCard` on each, beside `PermissionCard`, using the same layout, `answering` spinner and outcome line:

- The title, then for each field: its `title` as a small header, its `description` as the question text, and the options as full-width rows (label, description underneath).
- **One tap sends** when the form has exactly one `single` field and every other field is `text` and empty. Otherwise a tap toggles the choice (radio for `single`, checkbox for `multi`) and **Submit** sends all fields at once; it is enabled once any field has a value.
- `text` fields start collapsed as an `<title>…` row (`Other…`) that opens a text box, and focus moves into the box when a tap opens it, but only when the form also has choice fields. A form with no choice fields shows its text boxes open, unfocused. Typing in one makes **Submit** appear even on a one-tap form.
- **Skip** sends `decline`.
- After the answer, the card collapses to the title and the chosen values (`✓ PostgreSQL`), or `Skipped` / `Cancelled`. Every device follows the entry's `status`, so a card answered on one closes on the others.
- Strings are English like the rest of the apps.

Files:

- Desktop: `apps/desktop/src/renderer/views/thread/QuestionCard.tsx`; a `"question"` case in `ChatRows.tsx`; `respondQuestion` in `store/bot-store.ts` and the host client; `QuestionField` and the entry data fields in `src/shared/models.ts`.
- iPhone: `apps/ios/Kit/Sources/CodyncUI/Thread/QuestionCard.swift`; a `"question"` case in `Thread/ChatRows.swift`; `respondQuestion` in `Store/BotStore+Actions.swift` and `CodyncKit/Client/HostClient.swift`; `fields` and `answer` on `EntryData`.

Clients that don't know `"question"` skip the entry (kinds are plain strings everywhere) and the user answers with a message.

## Not in scope

- Android and TUI cards.
- Option `preview` (`_meta` on an enum option).
- Answering from the push notification.
- Answering on the desktop notch island, which shows "Needs you" from the bot status and opens the chat.
- Voice calls naming question cards: they announce "needs your approval" today. A follow-up for both apps.
- Showing `ExitPlanMode`'s descriptive option names on permission cards (they arrive as normal `session/request_permission` and the cards replace names by `kind`); a separate fix.

## Tests

- Host: `host/tests/fixtures/question_agent.py` sends `elicitation/create`. Integration tests check the card is posted, `respondQuestion` returns `{action: "accept", content}` to the agent, `Permission::Auto` doesn't answer it, the text fallback answers it, and Stop cancels it. Unit tests for the schema → fields mapping, including the decline cases.
- Desktop: the one-tap vs Submit rule and the answer payload.
- iPhone: decoding `fields` and `answer` on `EntryData`.
- E2E: on the desktop app and the iPhone, ask a Claude bot to "ask me a multiple-choice question", once single-select and once multi-select with two questions.

## Release

Additive protocol change: no `minApp` bump, minor version bump. Update [chat messages](../features/chat-messages.md) to describe question cards once built.
