# Question Cards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Agents' multiple-choice questions (Claude Code's AskUserQuestion) show as cards on the desktop app and the iPhone, answered with one tap.

**Architecture:** The host declares the ACP `elicitation.form` client capability, so `claude-agent-acp` sends AskUserQuestion as an `elicitation/create` request. The host turns the request's JSON Schema into a flat `fields` list on a new `question` entry, posts it like an approval card, and answers the agent from a new `respondQuestion` API method (or from the user's next typed message). Desktop and iPhone render the entry as a `QuestionCard`.

**Tech Stack:** Rust 2024 host (`serde_json` with `preserve_order`, tokio), Electron + React + strict TypeScript (`node --test`), SwiftUI / Swift 6 (`swift-testing`).

**Spec:** [docs/design/question-cards.md](../design/question-cards.md)

**Work in:** the worktree `.claude/worktrees/question-cards` (branch `feat/question-cards`). Every path below is relative to it. Never `cd` to the main checkout.

---

## Wire contract (all tasks use these names)

Entry `kind: "question"`, data:

```json
{"title": "Which database?", "status": "pending", "answer": null, "author": "<bot id>",
 "fields": [{"key": "question_0", "kind": "single", "title": "Database", "description": null,
             "options": [{"value": "PostgreSQL", "label": "PostgreSQL", "description": "Relational"}]},
            {"key": "question_0_custom", "kind": "text", "title": "Other", "description": null, "options": []}]}
```

- `kind`: `single` | `multi` | `text`. `options` is always an array (empty for `text`).
- `status`: `pending` | `answered` | `declined` | `cancelled` | `expired`.
- `answer`: after answering, one display line per answered field, e.g. `["PostgreSQL"]` or `["Cache, Queue"]`.

API: `POST respondQuestion {entryId, answers?}`. `answers` is `{field key: [values]}`: chosen option `value`s, or `[typed text]` for a `text` field. Missing or `null` `answers` skips the question (`decline`).

## File map

| File | Change |
|---|---|
| `host/src/agent/bot/question_form.rs` | Create. Pure: schema → fields, answers → content + display lines |
| `host/src/agent/bot/question.rs` | Create. Actor: `on_elicit`, `answer_question`, `answer_with_message` |
| `host/src/agent/bot/mod.rs` | Modify. `mod` lines, `Cmd::Question` |
| `host/src/agent/bot/updates.rs` | Modify. Route `elicitation/create`, `Answer::Elicit`, `show_card` picks kind, `after_answer` |
| `host/src/agent/bot/queue.rs` | Modify. Dispatch `Cmd::Question`; `Cmd::Send` tries `answer_with_message` |
| `host/src/agent/bot/launch.rs` | Modify. `"elicitation": {"form": {}}` |
| `host/src/store/model.rs` | Modify. `EntryKind::Question` |
| `host/src/store/entries.rs` | Modify. Restart expires pending question cards |
| `host/src/api/bots.rs` | Modify. `respondQuestion` |
| `host/tests/fixtures/question_agent.py` | Create. Scripted agent that asks through `elicitation/create` |
| `host/tests/question_e2e.rs` | Create. End-to-end tests |
| `apps/desktop/src/shared/models.ts` | Modify. `QuestionField`, `QuestionOption`, `QuestionAnswers`, `fields`, `answer`, `isChat` |
| `apps/desktop/src/renderer/views/thread/question-form.ts` (+ `.test.ts`) | Create. Pure card logic |
| `apps/desktop/src/renderer/views/thread/QuestionCard.tsx`, `question-card.css` | Create. The card |
| `apps/desktop/src/renderer/views/thread/ChatRows.tsx`, `TraceView.tsx` | Modify. Render `question` |
| `apps/desktop/src/renderer/store/bot-store.ts` | Modify. `respondQuestion` |
| `apps/ios/Kit/Sources/CodyncKit/Models/Question.swift` | Create. Types + `QuestionForm` logic |
| `apps/ios/Kit/Sources/CodyncKit/Models/EntryData.swift`, `Models.swift` | Modify. `fields`, `answer`, `isChat` |
| `apps/ios/Kit/Sources/CodyncKit/Client/HostClient.swift` | Modify. `respondQuestion` |
| `apps/ios/Kit/Sources/CodyncUI/Store/BotStore+Actions.swift` | Modify. `respondQuestion` |
| `apps/ios/Kit/Sources/CodyncUI/Thread/QuestionCard.swift` | Create. The card |
| `apps/ios/Kit/Sources/CodyncUI/Thread/ChatRows.swift`, `TraceView.swift` | Modify. Render `question` |
| `apps/ios/Kit/Tests/CodyncKitTests/QuestionFormTests.swift` | Create |
| `docs/design/question-cards.md`, `docs/features/chat-messages.md`, `docs/reference/remote-relay.md` | Modify |

Android (`apps/android/`) and the TUI (`host/src/tui/`) are deliberately untouched (owner's call, recorded in the spec). They keep working through the typed-message fallback in Task 3.

---

### Task 1: Host — form fields and answers (pure)

**Files:**
- Create: `host/src/agent/bot/question_form.rs`
- Modify: `host/src/agent/bot/mod.rs:18-27` (module list)
- Modify: `docs/design/question-cards.md` (answer shape)

- [ ] **Step 1: Register the module**

In `host/src/agent/bot/mod.rs`, the module list is alphabetical. Insert after `mod launch;` / `mod lifecycle;`, before `mod queue;`:

```rust
mod question_form;
```

- [ ] **Step 2: Write the failing tests**

Create `host/src/agent/bot/question_form.rs` with only the tests (the functions don't exist yet):

```rust
//! An agent's form (`elicitation/create`, how Claude Code asks AskUserQuestion) as the fields of a
//! question card, and the card's answers as the form's content.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// What `claude-agent-acp` sends for two questions: one single choice (with its "Other" box), one multi.
    fn ask_user_question() -> Value {
        json!({"type": "object", "properties": {
            "question_0": {"type": "string", "title": "Database", "oneOf": [
                {"const": "PostgreSQL", "title": "PostgreSQL", "description": "Relational"},
                {"const": "SQLite", "title": "SQLite"}]},
            "question_0_custom": {"type": "string", "title": "Other"},
            "question_1": {"type": "array", "title": "Extras", "description": "Which extras?",
                "items": {"anyOf": [{"const": "Cache", "title": "Cache"}, {"const": "Queue", "title": "Queue"}]}}}})
    }

    #[test]
    fn ask_user_question_becomes_fields_in_order() {
        let fields = fields(&ask_user_question()).unwrap();
        let kinds: Vec<&str> = fields.iter().map(|f| f["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["single", "text", "multi"]);
        assert_eq!(
            fields[0]["options"][0],
            json!({"value": "PostgreSQL", "label": "PostgreSQL", "description": "Relational"})
        );
        assert_eq!(fields[0]["options"][1]["description"], Value::Null);
        assert_eq!(fields[1]["options"], json!([]));
        assert_eq!(fields[2]["description"], "Which extras?");
    }

    #[test]
    fn a_bare_enum_is_a_single_choice() {
        let f = fields(&json!({"properties": {"tier": {"type": "string", "enum": ["free", "pro"]}}})).unwrap();
        assert_eq!(f[0]["kind"], "single");
        assert_eq!(f[0]["options"][1], json!({"value": "pro", "label": "pro", "description": null}));
    }

    #[test]
    fn fields_the_card_cannot_ask_refuse_the_form() {
        assert!(fields(&json!({"properties": {"n": {"type": "integer"}}})).is_none());
        assert!(fields(&json!({"properties": {"ok": {"type": "boolean"}}})).is_none());
        assert!(fields(&json!({})).is_none());
    }

    #[test]
    fn answers_become_the_forms_content() {
        let fields = fields(&ask_user_question()).unwrap();
        let answers =
            json!({"question_0": ["SQLite"], "question_0_custom": ["  "], "question_1": ["Cache", "Queue", "Made up"]});
        let (content, lines) = content(&fields, &answers);
        assert_eq!(content, json!({"question_0": "SQLite", "question_1": ["Cache", "Queue"]}));
        assert_eq!(lines, ["SQLite", "Cache, Queue"]);
    }

    #[test]
    fn typed_text_is_kept_as_typed() {
        let fields = fields(&ask_user_question()).unwrap();
        let (content, lines) = content(&fields, &json!({"question_0_custom": ["MySQL, please"]}));
        assert_eq!(content, json!({"question_0_custom": "MySQL, please"}));
        assert_eq!(lines, ["MySQL, please"]);
    }

    #[test]
    fn no_answers_is_empty() {
        let fields = fields(&ask_user_question()).unwrap();
        let (content, lines) = content(&fields, &json!({}));
        assert_eq!(content, json!({}));
        assert!(lines.is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to see them fail**

Run: `cd host && cargo test question_form`
Expected: compile errors `cannot find function 'fields'` and `cannot find function 'content'`.

- [ ] **Step 4: Implement**

Put this above the `#[cfg(test)]` block in `host/src/agent/bot/question_form.rs`:

```rust
use serde::Serialize;
use serde_json::{Map, Value, json};

/// How the card asks for one field (wire values `single` / `multi` / `text`).
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FieldKind {
    /// One of its options.
    Single,
    /// Any of its options.
    Multi,
    /// Free text, such as AskUserQuestion's "Other" box.
    Text,
}

/// The card's fields for a form's `requestedSchema`, in its order: `{key, kind, title, description, options}`.
/// `None` when a field asks for something the card can't (a number, a yes/no, an object): the form is declined.
pub(super) fn fields(schema: &Value) -> Option<Vec<Value>> {
    schema["properties"].as_object()?.iter().map(|(key, p)| field(key, p)).collect()
}

fn field(key: &str, p: &Value) -> Option<Value> {
    let (kind, options) = match p["type"].as_str()? {
        "string" => options(p).map_or((FieldKind::Text, Vec::new()), |o| (FieldKind::Single, o)),
        "array" => (FieldKind::Multi, options(&p["items"])?),
        _ => return None,
    };
    Some(json!({"key": key, "kind": kind, "title": p["title"], "description": p["description"], "options": options}))
}

/// A choice field's options: titled `oneOf` / `anyOf` entries, or a bare `enum`.
fn options(p: &Value) -> Option<Vec<Value>> {
    if let Some(titled) = p["oneOf"].as_array().or_else(|| p["anyOf"].as_array()) {
        return Some(
            titled
                .iter()
                .filter_map(|o| {
                    let value = o["const"].as_str()?;
                    let label = o["title"].as_str().unwrap_or(value);
                    Some(json!({"value": value, "label": label, "description": o["description"]}))
                })
                .collect(),
        );
    }
    let bare = p["enum"].as_array()?;
    Some(bare.iter().filter_map(Value::as_str).map(|v| json!({"value": v, "label": v, "description": null})).collect())
}

/// The form's content for the card's answers (`{field key: [chosen values or typed text]}`), and what the
/// answered card shows: one line per answered field. Blank answers, and values that aren't one of a choice
/// field's options, are left out (the answers come from a client).
pub(super) fn content(fields: &[Value], answers: &Value) -> (Value, Vec<String>) {
    let mut content = Map::new();
    let mut lines = Vec::new();
    for f in fields {
        let Some(key) = f["key"].as_str() else { continue };
        let options = f["options"].as_array().map_or(&[][..], Vec::as_slice);
        let label = |v: &str| options.iter().find(|o| o["value"] == v).and_then(|o| o["label"].as_str());
        let picked: Vec<&str> = answers[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty() && (f["kind"] == "text" || label(v).is_some()))
            .collect();
        let Some(first) = picked.first() else { continue };
        content.insert(key.to_owned(), if f["kind"] == "multi" { json!(picked) } else { json!(first) });
        lines.push(picked.iter().map(|v| label(v).unwrap_or(v)).collect::<Vec<_>>().join(", "));
    }
    (Value::Object(content), lines)
}
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd host && cargo test question_form`
Expected: `6 passed`. A `dead_code` warning for `fields`/`content` is expected until Task 2 uses them.

- [ ] **Step 6: Bring the spec in line with the wire contract**

In `docs/design/question-cards.md` (Host, step 5), make two text replacements with Edit:
1. `` `respondQuestion {entryId, action, content?}` `` → `` `respondQuestion {entryId, answers?}` (`answers`: `{field key: [values]}`, chosen option values or the typed text; none skips) ``
2. `` `answer` to `content` `` → `` `answer` to one display line per answered field ``

- [ ] **Step 7: Don't commit yet**

`fields` and `content` are unused until Task 2, and the repo's pre-commit hook (and CI) run `cargo clippy -- -D warnings`, which rejects the `dead_code` warning. Task 2's commit includes these files.

---

### Task 2: Host — question cards end to end

**Files:**
- Create: `host/src/agent/bot/question.rs`, `host/tests/fixtures/question_agent.py`, `host/tests/question_e2e.rs`
- Modify: `host/src/agent/bot/mod.rs`, `updates.rs`, `queue.rs`, `launch.rs`, `host/src/store/model.rs`, `host/src/store/entries.rs`, `host/src/api/bots.rs`

- [ ] **Step 1: Write the fixture agent**

Create `host/tests/fixtures/question_agent.py`:

```python
# ACP agent that asks the user through `elicitation/create`, the way claude-agent-acp asks
# AskUserQuestion, then replies with JSON: whether the host offered forms, what it answered,
# and how many prompts it got. `number` asks for a field a question card can't show.
import json,sys,threading
def send(m): sys.stdout.write(json.dumps(m)+"\n"); sys.stdout.flush()
mode=sys.argv[1] if len(sys.argv)>1 else "ask"
pending={}; nid=[100]; sid="s1"; caps={}; prompts=[0]
QUESTION={"type":"object","properties":{
  "question_0":{"type":"string","title":"Database","oneOf":[
    {"const":"PostgreSQL","title":"PostgreSQL","description":"Relational"},{"const":"SQLite","title":"SQLite"}]},
  "question_0_custom":{"type":"string","title":"Other","description":"Type your own answer"}}}
NUMBER={"type":"object","properties":{"count":{"type":"integer","title":"How many?"}}}
def upd(u): send({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":sid,"update":u}})
def turn(rid):
    prompts[0]+=1
    nid[0]+=1; pid=nid[0]
    ev=threading.Event(); pending[pid]=ev
    schema=NUMBER if mode=="number" else QUESTION
    send({"jsonrpc":"2.0","id":pid,"method":"elicitation/create","params":{"mode":"form","sessionId":sid,"toolCallId":"t1","message":"Which database?","requestedSchema":schema}})
    ev.wait()
    if ev.answer.get("action")=="cancel":
        send({"jsonrpc":"2.0","id":rid,"result":{"stopReason":"cancelled"}}); return
    reply={"form":"form" in caps.get("elicitation",{}),"answer":ev.answer,"prompts":prompts[0]}
    upd({"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":json.dumps(reply)}})
    send({"jsonrpc":"2.0","id":rid,"result":{"stopReason":"end_turn"}})
for line in sys.stdin:
    m=json.loads(line)
    if "method" not in m:
        ev=pending.pop(m["id"],None)
        if ev: ev.answer=m.get("result") or {"error":m.get("error")}; ev.set()
        continue
    meth=m["method"]
    if meth=="initialize":
        caps.update(m["params"].get("clientCapabilities",{}))
        send({"jsonrpc":"2.0","id":m["id"],"result":{"protocolVersion":1,"agentCapabilities":{"loadSession":False}}})
    elif meth=="session/new": send({"jsonrpc":"2.0","id":m["id"],"result":{"sessionId":sid}})
    elif meth=="session/prompt": threading.Thread(target=turn,args=(m["id"],)).start()
    elif meth=="session/cancel":
        for k,ev in list(pending.items()): ev.answer={"action":"cancel"}; ev.set()
```

- [ ] **Step 2: Write the failing end-to-end tests**

Create `host/tests/question_e2e.rs`:

```rust
//! A real host and a scripted agent asking through `elicitation/create` (Claude Code's AskUserQuestion):
//! the question card, answering and skipping it, Stop, and a form the card can't show.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code: failing loudly is the point

mod common;

use common::host::Host;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn python_missing() -> bool {
    let missing = std::process::Command::new(common::python()).arg("--version").output().is_err();
    if missing {
        eprintln!("skipping: Python not available");
    }
    missing
}

/// Polls the bot's history until `pred` matches an entry.
async fn wait_for(host: &Host, bot: &str, pred: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let h = host.call("history", json!({"botId": bot})).await;
        if let Some(e) = h["entries"].as_array().unwrap().iter().rev().find(|e| pred(e)) {
            return e.clone();
        }
        assert!(Instant::now() < deadline, "timed out; history: {h}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Sends a message and waits for the bot's question card.
async fn ask(host: &Host, bot: &str) -> Value {
    let nonce = uuid::Uuid::new_v4().to_string();
    host.call("send", json!({"botId": bot, "text": "go", "clientNonce": nonce})).await;
    wait_for(host, bot, |e| e["kind"] == "question" && e["data"]["status"] == "pending").await
}

/// What the agent got back, from its final reply.
async fn outcome(host: &Host, bot: &str) -> Value {
    let reply = wait_for(host, bot, |e| e["kind"] == "agent" && e["data"]["final"] == true).await;
    serde_json::from_str(reply["data"]["text"].as_str().unwrap()).unwrap()
}

/// The card's latest state once it's no longer pending.
async fn settled(host: &Host, bot: &str, card: &Value) -> Value {
    let id = card["id"].clone();
    wait_for(host, bot, |e| e["id"] == id && e["data"]["status"] != "pending").await
}

#[tokio::test]
async fn one_tap_answer_reaches_the_agent() {
    if python_missing() {
        return;
    }
    let host = Host::start().await;
    // New bots approve on their own (`Permission::Auto`); a question still waits for the user.
    let bot = host.fixture_bot("Asker", "question_agent.py", "ask").await;
    let card = ask(&host, &bot).await;
    assert_eq!(card["data"]["title"], "Which database?");
    let fields = card["data"]["fields"].as_array().unwrap();
    assert_eq!(fields[0]["kind"], "single");
    assert_eq!(fields[0]["options"][0], json!({"value": "PostgreSQL", "label": "PostgreSQL", "description": "Relational"}));
    assert_eq!(fields[1]["kind"], "text");
    assert_eq!(host.snapshot().await["bots"][0]["status"], "needsInput");

    host.call("respondQuestion", json!({"entryId": card["id"], "answers": {"question_0": ["PostgreSQL"]}})).await;
    let got = outcome(&host, &bot).await;
    assert_eq!(got["form"], true, "the host offers forms at initialize");
    assert_eq!(got["answer"], json!({"action": "accept", "content": {"question_0": "PostgreSQL"}}));
    let card = settled(&host, &bot, &card).await;
    assert_eq!(card["data"]["status"], "answered");
    assert_eq!(card["data"]["answer"], json!(["PostgreSQL"]));
}

#[tokio::test]
async fn skipping_declines_the_question() {
    if python_missing() {
        return;
    }
    let host = Host::start().await;
    let bot = host.fixture_bot("Asker", "question_agent.py", "ask").await;
    let card = ask(&host, &bot).await;
    host.call("respondQuestion", json!({"entryId": card["id"]})).await;
    assert_eq!(outcome(&host, &bot).await["answer"], json!({"action": "decline"}));
    assert_eq!(settled(&host, &bot, &card).await["data"]["status"], "declined");
}

#[tokio::test]
async fn stop_cancels_the_open_question() {
    if python_missing() {
        return;
    }
    let host = Host::start().await;
    let bot = host.fixture_bot("Asker", "question_agent.py", "ask").await;
    let card = ask(&host, &bot).await;
    host.call("stop", json!({"botId": bot})).await;
    assert_eq!(settled(&host, &bot, &card).await["data"]["status"], "cancelled");
}

#[tokio::test]
async fn a_form_the_card_cannot_show_is_declined() {
    if python_missing() {
        return;
    }
    let host = Host::start().await;
    let bot = host.fixture_bot("Counter", "question_agent.py", "number").await;
    host.call("send", json!({"botId": bot, "text": "go", "clientNonce": "n1"})).await;
    assert_eq!(outcome(&host, &bot).await["answer"], json!({"action": "decline"}));
    let history = host.call("history", json!({"botId": bot})).await;
    assert!(history["entries"].as_array().unwrap().iter().all(|e| e["kind"] != "question"));
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cd host && cargo test --test question_e2e`
Expected: all 4 fail. `one_tap…` times out waiting for a `question` entry (the host answers `elicitation/create` with `-32601` today, so the fixture replies `{"error": …}`). `a_form…` fails its `decline` assertion with an `error` answer.

- [ ] **Step 4: Add the entry kind**

In `host/src/store/model.rs`, add `Question` after `Permission` in `enum EntryKind` and in `as_str`:

```rust
    Permission,
    /// An agent's question (`elicitation/create`), answered on its card (see `agent::bot::question`).
    Question,
    Notice,
```

```rust
            Self::Permission => "permission",
            Self::Question => "question",
            Self::Notice => "notice",
```

- [ ] **Step 5: Offer forms at initialize**

In `host/src/agent/bot/launch.rs`, inside `"clientCapabilities"` (around line 120), add the line and extend the comment above it:

```rust
                // `elicitation.form`: the agent may ask the user through a form, shown as a question card
                // (`question`); Claude's adapter only offers AskUserQuestion when it's declared.
                "clientCapabilities": {
                    "fs": {"readTextFile": false, "writeTextFile": false},
                    "terminal": false,
                    "session": {"compaction": {}},
                    "elicitation": {"form": {}},
                    "_meta": {"parameterizedModelPicker": true},
                },
```

- [ ] **Step 6: Teach `updates.rs` about question cards**

In `host/src/agent/bot/updates.rs`:

(a) Add the variant to `enum Answer`:

```rust
    /// A Codync tool (`Cmd::Approve`): true when the user allowed it.
    Codync(oneshot::Sender<bool>),
    /// The agent's `elicitation/create` (a question card), by its JSON-RPC id.
    Elicit(Value),
```

(b) Route the request in `on_incoming` (replace the `if method == "session/request_permission"` block):

```rust
                if method == "session/request_permission" && self.turn.is_some() {
                    self.on_permission(id, params).await;
                } else if method == "elicitation/create" && self.turn.is_some() {
                    self.on_elicit(id, params).await;
                } else {
                    let _ = acp.respond_error(id, -32601, &format!("{method} is not supported by Codync")).await;
                }
```

(c) Make `show_card` shared and pick the entry kind from the answer. Replace its signature line and the lines through `r.activity = …`:

```rust
    /// Posts an approval or question card in the turn's lane and tells the user it needs them.
    pub(super) fn show_card(&mut self, title: &str, data: Value, answer: Answer) {
        let turn = self.turn.unwrap_or(0);
        self.close_seg();
        let (kind, activity) = match answer {
            Answer::Elicit(_) => (EntryKind::Question, "Has a question"),
            Answer::Agent(_) | Answer::Codync(_) => (EntryKind::Permission, "Needs approval"),
        };
        if let Some(e) = self.add(kind, turn, data) {
            self.perms.insert(e.id, answer);
        }
        self.hub.set_runtime(&self.cfg.id, |r| {
            r.status = BotStatus::NeedsInput;
            r.activity = format!("{activity}: {title}");
        });
```

(the push lines after it stay as they are).

(d) In `answer_permission`, add the arm after `Answer::Codync(reply) => { … }`:

```rust
            Answer::Elicit(rpc_id) => {
                // Only a turn that ends gets here (`cancel_turn`): the card's answers go through `answer_question`.
                status = "cancelled";
                if let Some(conn) = &self.conn {
                    let _ = conn.acp.respond(rpc_id, json!({"action": "cancel"})).await;
                }
            }
```

(e) Replace the final `if self.perms.is_empty() && self.turn.is_some() { … }` block of `answer_permission` with `self.after_answer();` and add after `answer_permission`:

```rust
    /// Once the last open card is answered, the bot works on.
    pub(super) fn after_answer(&self) {
        if self.perms.is_empty() && self.turn.is_some() {
            self.hub.set_runtime(&self.cfg.id, |r| {
                r.status = BotStatus::Working;
                r.activity = "Continuing…".into();
            });
        }
    }
```

- [ ] **Step 7: Write the actor side**

Create `host/src/agent/bot/question.rs`:

```rust
//! Agent → user: a form (`elicitation/create`, how Claude Code asks AskUserQuestion) as a question card,
//! answered on the card or cancelled with its turn (`cancel_turn`).

use super::Actor;
use super::question_form;
use super::updates::Answer;
use serde_json::{Value, json};

impl Actor {
    /// Posts the card whatever the bot's approvals: a question has no answer to pick for the user.
    pub(super) async fn on_elicit(&mut self, rpc_id: Value, params: Value) {
        let fields = if params["mode"] == "form" { question_form::fields(&params["requestedSchema"]) } else { None };
        let Some(fields) = fields.filter(|f| !f.is_empty()) else {
            tracing::warn!(bot = %self.cfg.id, mode = %params["mode"], "declined a form a question card can't show");
            if let Some(conn) = &self.conn {
                let _ = conn.acp.respond(rpc_id, json!({"action": "decline"})).await;
            }
            return;
        };
        let title = params["message"].as_str().filter(|m| !m.is_empty()).unwrap_or("A question for you").to_owned();
        let data = json!({"title": title, "fields": fields, "status": "pending", "answer": Value::Null});
        self.show_card(&title, data, Answer::Elicit(rpc_id));
    }

    /// `respondQuestion`: the card's answers (`{field key: [values]}`), or `None` to skip the question.
    pub(super) async fn answer_question(&mut self, entry_id: &str, answers: Option<&Value>) {
        if !matches!(self.perms.get(entry_id), Some(Answer::Elicit(_))) {
            return; // answered already, or not a question card
        }
        let Some(Answer::Elicit(rpc_id)) = self.perms.remove(entry_id) else { return };
        let card = self.hub.store.entry(entry_id);
        let fields = card.as_ref().and_then(|e| e.data["fields"].as_array().cloned()).unwrap_or_default();
        let (content, lines) = answers.map(|a| question_form::content(&fields, a)).unwrap_or_default();
        let (reply, status) = if lines.is_empty() {
            (json!({"action": "decline"}), "declined")
        } else {
            (json!({"action": "accept", "content": content}), "answered")
        };
        if let Some(conn) = &self.conn {
            let _ = conn.acp.respond(rpc_id, reply).await;
        }
        if let Some(mut e) = card {
            e.data["status"] = status.into();
            e.data["answer"] = json!(lines);
            self.hub.set_entry(entry_id, &e.data);
        }
        self.after_answer();
    }
}
```

Register it in `host/src/agent/bot/mod.rs` next to `mod question_form;`:

```rust
mod question;
mod question_form;
```

- [ ] **Step 8: Add the command and the API method**

In `host/src/agent/bot/mod.rs`, add after the `Permission { … }` variant of `enum Cmd`:

```rust
    /// `respondQuestion`: a question card's answers (`{field key: [values]}`), `None` to skip it.
    Question { entry_id: String, answers: Option<Value> },
```

In `host/src/agent/bot/queue.rs`, add after the `Cmd::Permission { … }` arm:

```rust
            Cmd::Question { entry_id, answers } => self.answer_question(&entry_id, answers.as_ref()).await,
```

In `host/src/api/bots.rs`, add after the `"respondPermission"` arm:

```rust
        "respondQuestion" => {
            let entry_id = str_arg(&b, "entryId")?;
            let e = hub.store.entry(entry_id).ok_or_else(|| anyhow!("unknown card"))?;
            // `{field key: [values]}`; none skips the question.
            let answers = b.get("answers").filter(|a| a.is_object()).cloned();
            // A card in a group belongs to the bot that asked.
            let bot = e.data["author"].as_str().unwrap_or(&e.bot_id);
            hub.send_cmd(bot, Cmd::Question { entry_id: entry_id.to_owned(), answers })?;
            json!({})
        }
```

- [ ] **Step 9: Expire question cards a restart left pending**

In `host/src/store/entries.rs` `expire_pending`, change the doc comment's first line to `/// Permission and question cards left `pending` (and sends left `queued`) by a crash or restart` and the SQL's first condition to:

```sql
"SELECT id FROM entries WHERE (kind IN ('permission', 'question') AND json_extract(data, '$.status') = 'pending')
```

In its test `restart_expires_pending_cards_and_queued_sends`, add after the permission insert and bump the count:

```rust
        let card =
            s.insert_entry(&Lane::main("b1"), EntryKind::Question, 1, &serde_json::json!({"status": "pending"})).unwrap();
```

```rust
        assert_eq!(s.expire_pending().unwrap(), 3);
        assert_eq!(s.entry(&card.id).unwrap().data["status"], "expired");
```

- [ ] **Step 10: Run the host checks**

Run: `cd host && cargo test --test question_e2e && cargo test --lib -- question_form expire_pending`
Expected: `4 passed` for `question_e2e`; the `question_form` and `expire_pending` unit tests pass.

Run: `cd host && cargo fmt && cargo clippy --all-targets -- -D warnings`
Expected: no warnings. Fix what clippy reports (pedantic is on) without `#[allow]` unless the existing code does the same.

- [ ] **Step 11: Commit**

```bash
git add host/src host/tests/fixtures/question_agent.py host/tests/question_e2e.rs docs/design/question-cards.md
git commit -m "feat(host): show agents' questions as cards answered with respondQuestion"
```

---

### Task 3: Host — a typed message answers the open question

**Files:**
- Modify: `host/src/agent/bot/question.rs`, `host/src/agent/bot/queue.rs:26-31`
- Test: `host/tests/question_e2e.rs`

- [ ] **Step 1: Write the failing test**

Append to `host/tests/question_e2e.rs`:

```rust
#[tokio::test]
async fn a_typed_message_answers_the_open_question() {
    if python_missing() {
        return;
    }
    let host = Host::start().await;
    let bot = host.fixture_bot("Asker", "question_agent.py", "ask").await;
    let card = ask(&host, &bot).await;
    host.call("send", json!({"botId": bot, "text": "MySQL, please", "clientNonce": "typed"})).await;
    let got = outcome(&host, &bot).await;
    assert_eq!(got["answer"], json!({"action": "accept", "content": {"question_0_custom": "MySQL, please"}}));
    assert_eq!(got["prompts"], 1, "the message answered the card instead of starting a turn");
    assert_eq!(settled(&host, &bot, &card).await["data"]["answer"], json!(["MySQL, please"]));
    let message = wait_for(&host, &bot, |e| e["kind"] == "user" && e["data"]["clientNonce"] == "typed").await;
    assert_eq!(message["data"]["status"], "sent");
    // No second turn followed (it would ask again).
    tokio::time::sleep(Duration::from_millis(500)).await;
    let history = host.call("history", json!({"botId": bot})).await;
    assert_eq!(history["entries"].as_array().unwrap().iter().filter(|e| e["kind"] == "question").count(), 1);
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cd host && cargo test --test question_e2e a_typed_message`
Expected: FAIL, `timed out` waiting for the final reply (the message queues behind the blocked turn).

- [ ] **Step 3: Implement**

Add to the `impl Actor` in `host/src/agent/bot/question.rs`, and extend the module doc's first line to `…answered on its card, by the user's next message, or cancelled with its turn (`cancel_turn`).`:

```rust
    /// A message typed while a question card waits in the turn's lane answers the card's first text field
    /// (AskUserQuestion's "Other") instead of queueing a turn, so clients without question cards (Android,
    /// the TUI, older apps) can answer too. Whether it did; a card it can't answer is skipped, and the
    /// message queues as usual.
    pub(super) async fn answer_with_message(&mut self, entry_id: &str) -> bool {
        let Some(card) = self.perms.iter().find_map(|(id, a)| matches!(a, Answer::Elicit(_)).then(|| id.clone()))
        else {
            return false;
        };
        let Some(mut message) = self.hub.store.entry(entry_id) else { return false };
        let text = message.data["text"].as_str().unwrap_or_default().trim().to_owned();
        let key = self.hub.store.entry(&card).and_then(|e| {
            e.data["fields"].as_array()?.iter().find(|f| f["kind"] == "text")?["key"].as_str().map(str::to_owned)
        });
        let Some(key) = key.filter(|_| !text.is_empty()) else {
            self.answer_question(&card, None).await;
            return false;
        };
        let mut answers = serde_json::Map::new();
        answers.insert(key, json!([text]));
        self.answer_question(&card, Some(&Value::Object(answers))).await;
        message.data["status"] = json!(crate::chat::team::RequestStatus::Sent);
        self.hub.set_entry(entry_id, &message.data);
        true
    }
```

In `host/src/agent/bot/queue.rs`, at the top of the `Cmd::Send { lane, entry_id, text } => {` arm:

```rust
            Cmd::Send { lane, entry_id, text } => {
                // Typed while a question card waits in this lane: it's the card's answer (`question`).
                if self.turn.is_some() && lane == self.lane && self.answer_with_message(&entry_id).await {
                    return true;
                }
                self.queue.push_back(Queued::User { lane, entry_id, text });
```

- [ ] **Step 4: Run all host checks**

Run: `cd host && cargo test --test question_e2e && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: `5 passed` for `question_e2e`; the full suite passes; no lint output.

- [ ] **Step 5: Commit**

```bash
git add host/src/agent/bot/question.rs host/src/agent/bot/queue.rs host/tests/question_e2e.rs
git commit -m "feat(host): a message typed while a question waits answers it"
```

---

### Task 4: Desktop — models and card logic

**Files:**
- Modify: `apps/desktop/src/shared/models.ts` (near `PermissionOption` ~line 165, `EntryData` ~line 280, `isChat` ~line 319)
- Create: `apps/desktop/src/renderer/views/thread/question-form.ts`, `question-form.test.ts`
- Test: `apps/desktop/src/shared/models.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `apps/desktop/src/renderer/views/thread/question-form.test.ts`:

```ts
import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { QuestionField } from '../../../shared/models.ts'
import { hasAnswer, pick, questionOutcome, sendsOnTap } from './question-form.ts'

const db: QuestionField = { key: 'question_0', kind: 'single', title: 'Database', options: [{ value: 'PostgreSQL', label: 'PostgreSQL' }, { value: 'SQLite', label: 'SQLite' }] }
const other: QuestionField = { key: 'question_0_custom', kind: 'text', title: 'Other', options: [] }
const extras: QuestionField = { key: 'question_1', kind: 'multi', title: 'Extras', options: [{ value: 'Cache', label: 'Cache' }, { value: 'Queue', label: 'Queue' }] }

test('one single-choice question sends on tap until something is typed', () => {
  assert.equal(sendsOnTap([db, other], {}), true)
  assert.equal(sendsOnTap([db, other], { question_0_custom: ['  '] }), true)
  assert.equal(sendsOnTap([db, other], { question_0_custom: ['MySQL'] }), false)
})

test('several questions or several picks wait for Submit', () => {
  assert.equal(sendsOnTap([db, other, extras], {}), false)
  assert.equal(sendsOnTap([extras], {}), false)
})

test('a single choice replaces, a multi choice toggles', () => {
  assert.deepEqual(pick({ question_0: ['SQLite'] }, db, 'PostgreSQL'), { question_0: ['PostgreSQL'] })
  const once = pick({}, extras, 'Cache')
  assert.deepEqual(pick(once, extras, 'Queue'), { question_1: ['Cache', 'Queue'] })
  assert.deepEqual(pick(once, extras, 'Cache'), { question_1: [] })
})

test('Submit needs a pick or typed text', () => {
  assert.equal(hasAnswer({}), false)
  assert.equal(hasAnswer({ question_1: [], question_0_custom: [' '] }), false)
  assert.equal(hasAnswer({ question_1: ['Cache'] }), true)
})

test('the answered card shows each answer', () => {
  assert.equal(questionOutcome('answered', ['PostgreSQL', 'Cache, Queue']), '✓ PostgreSQL\n✓ Cache, Queue')
  assert.equal(questionOutcome('declined', null), 'Skipped')
  assert.equal(questionOutcome('cancelled', null), 'Cancelled')
  assert.equal(questionOutcome('expired', null), 'Expired — the agent moved on')
})
```

Add to `apps/desktop/src/shared/models.test.ts`, next to the existing `isReplyable(message('permission', …))` assertion (reuse its `message` helper):

```ts
test('question cards show in the chat and take no replies', () => {
  assert.equal(isChat(message('question', { title: 'Which database?' })), true)
  assert.equal(isReplyable(message('question', { title: 'Which database?' })), false)
})
```

(If `isChat` isn't imported in that file yet, add it to the existing import from `./models.ts`.)

- [ ] **Step 2: Run them to see them fail**

Run: `cd apps/desktop && npm test`
Expected: FAIL — `Cannot find module './question-form.ts'`, and the `isChat(question)` assertion fails.

- [ ] **Step 3: Add the types**

In `apps/desktop/src/shared/models.ts`, after `interface PermissionOption { … }`:

```ts
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
```

In `interface EntryData`, extend the `status` doc to end with `· question: pending | answered | declined | cancelled | expired`, and add after `selected?: string`:

```ts
  /** question: what it asks, in order. */
  fields?: QuestionField[]
  /** question: once answered, one line per answered field. */
  answer?: string[] | null
```

In the `Entry` kind comment, add `question` (`user | agent | thought | tool | plan | permission | question | notice`). In `isChat`, add the case:

```ts
    case 'user':
    case 'permission':
    case 'question':
      return true
```

- [ ] **Step 4: Write the card logic**

Create `apps/desktop/src/renderer/views/thread/question-form.ts`:

```ts
import type { QuestionAnswers, QuestionField } from '../../../shared/models.ts'

const typed = (fields: QuestionField[], answers: QuestionAnswers) =>
  fields.some((f) => f.kind === 'text' && (answers[f.key]?.[0] ?? '').trim() !== '')

/** Whether one tap on an option answers the card: a single one-choice question with nothing typed. */
export function sendsOnTap(fields: QuestionField[], answers: QuestionAnswers): boolean {
  const choices = fields.filter((f) => f.kind !== 'text')
  return choices.length === 1 && choices[0]!.kind === 'single' && !typed(fields, answers)
}

/** `answers` after tapping `value` on `field`: a single choice replaces, a multi choice toggles. */
export function pick(answers: QuestionAnswers, field: QuestionField, value: string): QuestionAnswers {
  const current = answers[field.key] ?? []
  const next = field.kind !== 'multi' ? [value] : current.includes(value) ? current.filter((v) => v !== value) : [...current, value]
  return { ...answers, [field.key]: next }
}

/** Whether Submit has anything to send. */
export const hasAnswer = (answers: QuestionAnswers) => Object.values(answers).some((v) => v.some((s) => s.trim() !== ''))

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
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd apps/desktop && npm test && npm run typecheck`
Expected: all tests pass (the 5 new `question-form` tests included); typecheck clean.

- [ ] **Step 6: Commit**

```bash
git add apps/desktop/src/shared/models.ts apps/desktop/src/shared/models.test.ts apps/desktop/src/renderer/views/thread/question-form.ts apps/desktop/src/renderer/views/thread/question-form.test.ts
git commit -m "feat(desktop): question card models and answer logic"
```

---

### Task 5: Desktop — the question card

**Files:**
- Create: `apps/desktop/src/renderer/views/thread/QuestionCard.tsx`, `apps/desktop/src/renderer/views/thread/question-card.css`
- Modify: `apps/desktop/src/renderer/views/thread/ChatRows.tsx:18,56-62,67-71`, `apps/desktop/src/renderer/views/thread/TraceView.tsx:131`, `apps/desktop/src/renderer/store/bot-store.ts:211-217`

- [ ] **Step 1: Add the store action**

In `apps/desktop/src/renderer/store/bot-store.ts`, add after `respond(…) { … }`, and add `QuestionAnswers` to the file's existing `@shared/models` type import:

```ts
  /** A question card's answers (`{field key: values}`), or `null` to skip it. */
  respondQuestion(entry: Entry, answers: QuestionAnswers | null) {
    this.answer(entry, answers ? 'submit' : 'skip', async (c) => {
      await c.call('respondQuestion', { entryId: entry.id, answers })
      // The card's own update follows on the events stream; hold the spinner until then.
      await sleep(2000)
    })
  }
```

- [ ] **Step 2: Write the card**

Create `apps/desktop/src/renderer/views/thread/question-card.css`:

```css
/* The question card: the approval card's frame (permission-card.css) with choice rows, typed answers and actions. */
.question-option {
  padding: 8px 14px;
  text-align: left;
}
.question-text-row {
  width: 100%;
  padding: 4px 2px;
  text-align: left;
}
.question-text-field {
  animation: question-open 160ms ease-out;
}
@keyframes question-open {
  from { opacity: 0; transform: translateY(-4px); }
}
@media (prefers-reduced-motion: reduce) {
  .question-text-field { animation: none; }
}
.question-card-actions {
  justify-content: flex-end;
}
.question-card-outcome {
  white-space: pre-line;
}
```

Create `apps/desktop/src/renderer/views/thread/QuestionCard.tsx`:

```tsx
import { useState } from 'react'
import type { Entry, QuestionAnswers, QuestionField } from '@shared/models'
import { Button, Spinner } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { Field } from '../marketplace/pieces'
import { hasAnswer, pick, questionOutcome, sendsOnTap } from './question-form'
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
  const tap = (field: QuestionField, value: string) =>
    sendsOnTap(fields, answers) ? respond({ ...answers, [field.key]: [value] }) : setAnswers(pick(answers, field, value))
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
                  open={opened.has(f.key)}
                  value={answers[f.key]?.[0] ?? ''}
                  onOpen={() => setOpened(new Set(opened).add(f.key))}
                  onChange={(v) => setAnswers({ ...answers, [f.key]: [v] })}
                />
              ) : (
                <Choices key={f.key} field={f} chosen={answers[f.key] ?? []} disabled={answering} onTap={(v) => tap(f, v)} />
              ),
            )}
            <HStack gap={8} className="question-card-actions">
              {answering ? <Spinner size={14} /> : <>
                <Button kind="secondary" onClick={() => respond(null)}>Skip</Button>
                {sendsOnTap(fields, answers) ? null : <Button disabled={!hasAnswer(answers)} onClick={() => respond(answers)}>Submit</Button>}
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
      <div className="permission-card-options">
        {field.options.map((o, i) => (
          <div key={o.value}>
            {i > 0 ? <div className="thread-hairline" /> : null}
            <button className="permission-option question-option" disabled={disabled} onClick={() => onTap(o.value)}>
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

function TextAnswer({ field, open, value, onOpen, onChange }: { field: QuestionField; open: boolean; value: string; onOpen: () => void; onChange: (v: string) => void }) {
  const title = field.title ?? 'Answer'
  if (!open) {
    return (
      <button className="question-text-row" onClick={onOpen}>
        <Text size="body" tone="secondary">{`${title}…`}</Text>
      </button>
    )
  }
  return (
    <div className="question-text-field">
      <Field value={value} onChange={onChange} label={title} placeholder={field.description ?? title} lines={[2, 6]} />
    </div>
  )
}
```

- [ ] **Step 3: Render it in the chat and the trace**

In `apps/desktop/src/renderer/views/thread/ChatRows.tsx`:
- add `import { QuestionCard } from './QuestionCard'` after the `PermissionCard` import;
- add this case after `case 'permission': return (…)`:

```tsx
    case 'question':
      return (
        <VStack gap={4} className="chat-row group-start">
          {chat && isGroup(chat) ? <AuthorLabel botId={entry.data.author} /> : null}
          <LiveQuestion entry={entry} />
        </VStack>
      )
```

- add after `function LivePermission(…) { … }`:

```tsx
/** Follows the store: the card shows a spinner while its answers are on their way. */
function LiveQuestion({ entry }: { entry: Entry }) {
  const store = useLiveStore()
  return <QuestionCard entry={entry} answering={store.answering.has(entry.id)} respond={(a) => store.respondQuestion(entry, a)} />
}
```

In `apps/desktop/src/renderer/views/thread/TraceView.tsx`, make the permission row also take questions:

```tsx
    case 'permission':
    case 'question':
```

- [ ] **Step 4: Check it builds**

Run: `cd apps/desktop && npm run typecheck && npm test`
Expected: no type errors; all tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/renderer
git commit -m "feat(desktop): answer agents' questions on a card with one tap"
```

---

### Task 6: iPhone — models and card logic

**Files:**
- Create: `apps/ios/Kit/Sources/CodyncKit/Models/Question.swift`, `apps/ios/Kit/Tests/CodyncKitTests/QuestionFormTests.swift`
- Modify: `apps/ios/Kit/Sources/CodyncKit/Models/EntryData.swift:21-31`, `apps/ios/Kit/Sources/CodyncKit/Models/Models.swift:123`

- [ ] **Step 1: Find a simulator for the tests**

Run: `xcrun simctl list devices available | grep -m1 iPhone`
Expected: a line like `iPhone 17 (UUID) (Shutdown)`. Use its UUID as `<sim-id>` below. If the list is empty, install an iOS simulator runtime in Xcode → Settings → Components first.

- [ ] **Step 2: Write the failing tests**

Create `apps/ios/Kit/Tests/CodyncKitTests/QuestionFormTests.swift`:

```swift
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

@Test func oneSingleChoiceQuestionSendsOnTapUntilSomethingIsTyped() {
    #expect(QuestionForm.sendsOnTap([db, other], [:]))
    #expect(QuestionForm.sendsOnTap([db, other], ["question_0_custom": ["  "]]))
    #expect(!QuestionForm.sendsOnTap([db, other], ["question_0_custom": ["MySQL"]]))
    #expect(!QuestionForm.sendsOnTap([db, other, extras], [:]))
    #expect(!QuestionForm.sendsOnTap([extras], [:]))
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

@Test func theAnsweredCardShowsEachAnswer() {
    #expect(QuestionForm.outcome(status: "answered", answer: ["PostgreSQL", "Cache, Queue"]) == "✓ PostgreSQL\n✓ Cache, Queue")
    #expect(QuestionForm.outcome(status: "declined", answer: nil) == "Skipped")
    #expect(QuestionForm.outcome(status: "cancelled", answer: nil) == "Cancelled")
    #expect(QuestionForm.outcome(status: "expired", answer: nil) == "Expired — the agent moved on")
}
```

- [ ] **Step 3: Run them to see them fail**

Run: `cd apps/ios/Kit && xcodebuild test -scheme CodyncKit-Package -destination 'platform=iOS Simulator,id=<sim-id>' -only-testing:CodyncKitTests/QuestionFormTests 2>&1 | tail -20`
Expected: build fails, `cannot find 'QuestionField' in scope`.

- [ ] **Step 4: Add the types and logic**

Create `apps/ios/Kit/Sources/CodyncKit/Models/Question.swift`:

```swift
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

public struct QuestionOption: Codable, Hashable, Sendable, Identifiable {
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
```

In `apps/ios/Kit/Sources/CodyncKit/Models/EntryData.swift`, extend the `status` doc to end with `· question: pending | answered | declined | cancelled | expired`, and add after `public var selected: String?`:

```swift
    /// question: what it asks, in order.
    public var fields: [QuestionField]?
    /// question: once answered, one line per answered field.
    public var answer: [String]?
```

In `apps/ios/Kit/Sources/CodyncKit/Models/Models.swift:123`, in `isChat`:

```swift
        case "user", "permission", "question": true
```

- [ ] **Step 5: Run the tests to see them pass**

Run: `cd apps/ios/Kit && xcodebuild test -scheme CodyncKit-Package -destination 'platform=iOS Simulator,id=<sim-id>' -only-testing:CodyncKitTests/QuestionFormTests 2>&1 | tail -20`
Expected: `** TEST SUCCEEDED **`, 5 tests passed.

- [ ] **Step 6: Commit**

```bash
git add apps/ios/Kit/Sources/CodyncKit/Models apps/ios/Kit/Tests/CodyncKitTests/QuestionFormTests.swift
git commit -m "feat(ios): question card models and answer logic"
```

---

### Task 7: iPhone — the question card

**Files:**
- Create: `apps/ios/Kit/Sources/CodyncUI/Thread/QuestionCard.swift`
- Modify: `apps/ios/Kit/Sources/CodyncKit/Client/HostClient.swift:247-250`, `apps/ios/Kit/Sources/CodyncUI/Store/BotStore+Actions.swift:112-118`, `apps/ios/Kit/Sources/CodyncUI/Thread/ChatRows.swift:40-47`, `apps/ios/Kit/Sources/CodyncUI/Thread/TraceView.swift:98`

- [ ] **Step 1: Add the client call and the store action**

In `apps/ios/Kit/Sources/CodyncKit/Client/HostClient.swift`, after `func respondPermission(…) { … }` (same `public extension HostClient` block):

```swift
    /// A question card's answers (`[field key: values]`); nil skips the question.
    func respondQuestion(entryId: String, answers: QuestionAnswers?) async throws {
        struct Body: Encodable { var entryId: String; var answers: QuestionAnswers? }
        let _: Empty = try await call("respondQuestion", Body(entryId: entryId, answers: answers))
    }
```

In `apps/ios/Kit/Sources/CodyncUI/Store/BotStore+Actions.swift`, after `public func respond(_ entry: Entry, option: String?) { … }`:

```swift
    /// A question card's answers (`[field key: values]`), or nil to skip it.
    public func respondQuestion(_ entry: Entry, answers: QuestionAnswers?) {
        answer(entry, answers == nil ? "skip" : "submit") {
            try await $0.respondQuestion(entryId: entry.id, answers: answers)
            // The card's own update follows on the events stream; hold the spinner until then.
            try? await Task.sleep(for: .seconds(2))
        }
    }
```

- [ ] **Step 2: Write the card**

Create `apps/ios/Kit/Sources/CodyncUI/Thread/QuestionCard.swift`:

```swift
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

    private var d: EntryData { entry.data }
    private var fields: [QuestionField] { d.fields ?? [] }
    private var pending: Bool { d.status == "pending" }

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
                            if answers[field.key]?.contains(option.value) == true {
                                Image(systemName: "checkmark").font(.footnote.weight(.semibold)).foregroundStyle(Palette.accent)
                            }
                        }
                        .frame(maxWidth: .infinity, minHeight: 46, alignment: .leading)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 6)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(answering)
                }
            }
            .background(Palette.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        }
    }

    @ViewBuilder private func textAnswer(_ field: QuestionField) -> some View {
        let title = field.title ?? "Answer"
        if opened.contains(field.key) {
            TextField(field.description ?? title, text: typed(field), axis: .vertical)
                .lineLimit(2...6)
                .textFieldStyle(.plain)
                .padding(10)
                .background(Palette.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                .transition(.opacity.combined(with: .move(edge: .top)))
        } else {
            Button { Motion.animate { _ = opened.insert(field.key) } } label: {
                Text("\(title)…").font(.body).foregroundStyle(Palette.secondary)
            }
            .buttonStyle(.plain)
        }
    }

    private var actions: some View {
        HStack(spacing: 8) {
            Spacer(minLength: 0)
            if answering {
                Spinner(size: 14)
            } else {
                Button("Skip") { respond(nil) }.buttonStyle(SecondaryButtonStyle())
                if !QuestionForm.sendsOnTap(fields, answers) {
                    Button("Submit") { respond(answers) }
                        .buttonStyle(PrimaryButtonStyle())
                        .disabled(!QuestionForm.hasAnswer(answers))
                }
            }
        }
    }

    private func typed(_ field: QuestionField) -> Binding<String> {
        Binding { answers[field.key]?.first ?? "" } set: { answers[field.key] = [$0] }
    }

    private func tap(_ field: QuestionField, _ value: String) {
        if QuestionForm.sendsOnTap(fields, answers) {
            respond(answers.merging([field.key: [value]]) { $1 })
        } else {
            answers = QuestionForm.pick(answers, field, value)
        }
    }
}
```

- [ ] **Step 3: Render it in the chat and the trace**

In `apps/ios/Kit/Sources/CodyncUI/Thread/ChatRows.swift`, add after the `case "permission":` block (before `default:`):

```swift
        case "question":
            VStack(alignment: .leading, spacing: 4) {
                if chat?.isGroup == true { AuthorLabel(botId: entry.data.author) }
                QuestionCard(entry: entry, answering: model.answering[entry.id] != nil) { answers in
                    model.respondQuestion(entry, answers: answers)
                }
            }
            .padding(.top, 12)
```

In `apps/ios/Kit/Sources/CodyncUI/Thread/TraceView.swift:98`:

```swift
        case "permission", "question":
```

- [ ] **Step 4: Build and test the package**

Run: `cd apps/ios/Kit && xcodebuild test -scheme CodyncKit-Package -destination 'platform=iOS Simulator,id=<sim-id>' 2>&1 | tail -20`
Expected: `** TEST SUCCEEDED **`. Fix any Swift 6 concurrency or SwiftLint-style errors the build reports.

- [ ] **Step 5: Commit**

```bash
git add apps/ios/Kit/Sources
git commit -m "feat(ios): answer agents' questions on a card with one tap"
```

---

### Task 8: Docs, version, real end-to-end check

**Files:**
- Modify: `docs/features/chat-messages.md:43`, `docs/reference/remote-relay.md:318`, `docs/design/question-cards.md:3`
- Modify: `apps/project.yml`, `host/Cargo.toml`, `host/Cargo.lock`, `apps/desktop/package.json`, `apps/desktop/package-lock.json`, `apps/ios/WhatsNew.md`

- [ ] **Step 1: Update the docs**

- `docs/features/chat-messages.md:43`: change `permission cards and notices (`isChat`)` to `permission cards, question cards and notices (`isChat`)`, and add a bullet after that one:

```markdown
- An agent's question (Claude Code's AskUserQuestion, an ACP `elicitation/create` form) is a question card:
  one tap answers a single one-choice question, Submit sends several questions, several picks or typed text,
  Skip declines. A message typed while it waits answers its "Other" box. Desktop and iPhone only; Android and
  the TUI answer by message ([question cards](../design/question-cards.md)).
```

- `docs/reference/remote-relay.md:318`: add `respondQuestion` right after `respondPermission` in the control-scope list.
- `docs/design/question-cards.md:3`: replace `Design, not built yet.` with `Built for the host, desktop app and iPhone app.` and set the `Updated:` date to the day this lands (YYYY-MM-DD).

- [ ] **Step 2: Bump the version to 2.23.0 (protocol addition = minor)**

```bash
sed -i '' 's/MARKETING_VERSION: 2.22.0/MARKETING_VERSION: 2.23.0/' apps/project.yml
sed -i '' 's/^version = "2.22.0"/version = "2.23.0"/' host/Cargo.toml
(cd host && cargo update -p codync-host --offline)
(cd apps/desktop && npm version 2.23.0 --no-git-tag-version)
xcodegen generate --spec apps/project.yml
git diff --stat
```

Expected: the four version files, `Cargo.lock` and `apps/Codync.xcodeproj` change; nothing else. (If `cargo update -p codync-host` names a different package, use the `name` from `host/Cargo.toml`.)

Add a `## 2.23.0` section at the top of `apps/ios/WhatsNew.md` following the file's existing format (`### zh-Hant` and `### en-US`):

```markdown
## 2.23.0

### zh-Hant
- Agent 提問時，直接點選選項即可回答。

### en-US
- When an agent asks you something, answer with one tap on a choice.
```

Commit: `git add -A apps/project.yml apps/Codync.xcodeproj host/Cargo.toml host/Cargo.lock apps/desktop/package.json apps/desktop/package-lock.json apps/ios/WhatsNew.md docs && git commit -m "build: bump version to 2.23.0"`

- [ ] **Step 3: Real end to end on the desktop app (sandbox)**

```bash
cd apps/desktop && npm ci && npm run dev
```

In the `Codync Dev (question-cards)` window (Computer Use):
1. Create a bot with the Claude Code agent, approvals on Ask.
2. Send: `Ask me which database to use with AskUserQuestion: PostgreSQL or SQLite.`
3. Expect a question card. Tap `PostgreSQL`: the card collapses to `✓ PostgreSQL` and Claude's reply uses it.
4. Send: `Ask me two questions at once with AskUserQuestion, the second one multi-select.` Expect Submit; pick, Submit, check the reply.
5. Send another one-question ask; instead of tapping, type `MySQL` in the composer and send: the card shows `✓ MySQL`.

- [ ] **Step 4: Real end to end on the iPhone (simulator)**

Build the `iOS Dev` scheme into `build/dd` per [development: iOS app](../guides/development.md#ios-app) on the simulator from Task 6, pair it with the sandbox host from Step 3, and repeat steps 2–4 on the phone.

- [ ] **Step 5: after-change workflow**

Run the project's `after-change` workflow from this worktree (review → fix → cleanup → e2e), as `CLAUDE.md` requires. Commit its fixes.

- [ ] **Step 6: Report and hand over**

Report what passed, with output, and any check that couldn't run (and why). Do **not** merge or push: merging into `main` triggers a release in this repo's CI; ask the owner where it should land (`origin/main` of the fork, or a PR to `leepokai/Codync`).
