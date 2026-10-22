//! Agent → user: a form (`elicitation/create`, how Claude Code asks `AskUserQuestion`) as a question card,
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
