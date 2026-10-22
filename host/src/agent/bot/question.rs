//! Agent → user: a form (`elicitation/create`, how Claude Code asks `AskUserQuestion`) as a question card,
//! answered on the card, by the user's next message, or cancelled with its turn (`cancel_turn`).

use super::Actor;
use super::cards::Answer;
use super::question_form;
use serde_json::{Value, json};

impl Actor {
    /// Posts the card whatever the bot's approvals: a question has no answer to pick for the user. A routine run or
    /// an ask another bot waits on has nobody to answer, so its question is declined at once and posts no card
    /// (before question cards Claude couldn't ask there at all). Group turns get cards: the user is in the room.
    /// So does a request that reports in the recipient's own chat (the user reads it there, as with approvals).
    pub(super) async fn on_elicit(&mut self, rpc_id: Value, params: Value) {
        // Declaring forms also brings claude-agent-acp's "<model> declined this request. Retry with <fallback>?".
        // Retrying would move the session off the bot's model, past Codync's own fallback chain: declined, the
        // adapter keeps the refusal and the turn ends as it did before question cards.
        if question_form::is_refusal_fallback(&params["requestedSchema"]) {
            tracing::info!(bot = %self.cfg.id, "declined the agent's offer to retry a refusal on another model");
            self.decline(rpc_id).await;
            return;
        }
        if self.active_routine.is_some()
            || self.active_request.as_ref().is_some_and(crate::chat::team::BotRequest::expects_reply)
        {
            tracing::info!(bot = %self.cfg.id, "declined a question: nobody is there to answer a routine or an ask");
            self.decline(rpc_id).await;
            return;
        }
        let fields = if params["mode"] == "form" { question_form::fields(&params["requestedSchema"]) } else { None };
        let Some(fields) = fields.filter(|f| !f.is_empty()) else {
            tracing::warn!(bot = %self.cfg.id, mode = %params["mode"], "declined a form a question card can't show");
            self.decline(rpc_id).await;
            return;
        };
        let title = params["message"].as_str().filter(|m| !m.is_empty()).unwrap_or("A question for you").to_owned();
        let data = json!({"title": title, "fields": fields, "status": "pending", "answer": Value::Null});
        self.show_card(&title, data, Answer::Elicit(rpc_id));
    }

    /// Tells the agent the user skipped its question.
    async fn decline(&self, rpc_id: Value) {
        if let Some(conn) = &self.conn {
            // A failed write means the agent exited; `Incoming::Closed` ends the turn.
            let _ = conn.acp.respond(rpc_id, json!({"action": "decline"})).await;
        }
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
            // A failed write means the agent exited; `Incoming::Closed` ends the turn.
            let _ = conn.acp.respond(rpc_id, reply).await;
        }
        if let Some(mut e) = card {
            e.data["status"] = status.into();
            e.data["answer"] = json!(lines);
            self.hub.set_entry(entry_id, &e.data);
        }
        self.after_answer();
    }

    /// A message typed while a question card waits in the turn's lane answers the card's first text field
    /// (`AskUserQuestion`'s "Other") instead of queueing a turn, so clients without question cards (Android,
    /// the TUI, older apps) can answer too. Whether it did; a card it can't answer is skipped, and the
    /// message queues as usual. The oldest waiting card is the one answered.
    pub(super) async fn answer_with_message(&mut self, entry_id: &str) -> bool {
        let cards = self.perms.iter().filter(|(_, a)| matches!(a, Answer::Elicit(_)));
        let Some(card) = cards.filter_map(|(id, _)| self.hub.store.entry(id)).min_by_key(|e| e.seq) else {
            return false;
        };
        let Some(mut message) = self.hub.store.entry(entry_id) else { return false };
        // Only the user's new words answer. Host notes (`from: host`, as in `turn`) and messages typed before the
        // question (held sign-in messages sent again) travel as `Cmd::Send` too, but weren't written for it.
        // `seq` is insertion order: unlike the wall clock, it can't step backwards.
        if message.data["from"] == "host" || message.seq < card.seq {
            return false;
        }
        let text = message.data["text"].as_str().unwrap_or_default().trim().to_owned();
        // Only the text would reach the agent: the files would be lost, so the message queues with them.
        let has_files = message.data["attachments"].as_array().is_some_and(|a| !a.is_empty());
        let key = card.data["fields"]
            .as_array()
            .and_then(|fields| fields.iter().find(|f| f["kind"] == "text")?["key"].as_str().map(str::to_owned));
        let Some(key) = key.filter(|_| !text.is_empty() && !has_files) else {
            self.answer_question(&card.id, None).await;
            return false;
        };
        let mut answers = serde_json::Map::new();
        answers.insert(key, json!([text]));
        self.answer_question(&card.id, Some(&Value::Object(answers))).await;
        // The message was numbered after the last turn, but it speaks in the running one: clients take the
        // highest sent turn as the one running, and a message past it would break its trace and order.
        if let Some(turn) = self.turn {
            self.hub.set_entry_turn(entry_id, turn);
        }
        message.data["status"] = json!(crate::chat::team::RequestStatus::Sent);
        self.hub.set_entry(entry_id, &message.data);
        true
    }
}
