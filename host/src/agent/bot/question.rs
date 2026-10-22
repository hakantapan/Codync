//! Agent → user: a form (`elicitation/create`, how Claude Code asks `AskUserQuestion`) as a question card,
//! answered on the card or cancelled with its turn (`cancel_turn`).

use super::Actor;
use super::cards::Answer;
use super::question_form;
use serde_json::{Value, json};

impl Actor {
    /// Posts the card whatever the bot's approvals: a question has no answer to pick for the user. A routine run or
    /// a bot-to-bot request has nobody to answer, so its question is declined at once and posts no card (before
    /// question cards Claude couldn't ask there at all). Group turns get cards: the user is in the room.
    pub(super) async fn on_elicit(&mut self, rpc_id: Value, params: Value) {
        if self.active_routine.is_some() || self.active_request.is_some() {
            tracing::info!(bot = %self.cfg.id, "declined a question: nobody is there to answer a routine or a request");
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
}
