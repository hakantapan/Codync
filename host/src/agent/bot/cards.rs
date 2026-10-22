//! Approval and question cards: what the agent or a Codync tool asks the user, and the answers going back.

use super::Actor;
use super::updates::merge_tool_content;
use crate::hub::BotStatus;
use crate::remote::push::{self, AlertKind};
use crate::store::{EntryKind, Permission};
use serde_json::{Value, json};
use tokio::sync::oneshot;

/// Who waits for an approval card's answer.
pub(super) enum Answer {
    /// The agent's `session/request_permission`, by its JSON-RPC id.
    Agent(Value),
    /// A Codync tool (`Cmd::Approve`): true when the user allowed it.
    Codync(oneshot::Sender<bool>),
    /// The agent's `elicitation/create` (a question card), by its JSON-RPC id.
    Elicit(Value),
}

impl Actor {
    pub(super) async fn on_permission(&mut self, rpc_id: Value, params: Value) {
        let options = params["options"].as_array().cloned().unwrap_or_default();
        let tool = &params["toolCall"];
        if self.cfg.permission == Permission::Auto {
            let pick = ["allow_once", "allow_always"]
                .iter()
                .find_map(|k| options.iter().find(|o| o["kind"] == *k))
                .or_else(|| options.first());
            if let (Some(o), Some(conn)) = (pick, &self.conn) {
                let _ = conn
                    .acp
                    .respond(rpc_id, json!({"outcome": {"outcome": "selected", "optionId": o["optionId"]}}))
                    .await;
                return;
            }
        }
        let title = tool["title"]
            .as_str()
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                let eid = self.tools.get(tool["toolCallId"].as_str()?)?;
                self.hub.store.entry(eid)?.data["title"].as_str().map(str::to_owned)
            })
            .unwrap_or_else(|| "Use a tool".into());
        let mut detail = json!({"output": "", "diffs": []});
        merge_tool_content(&mut detail, &tool["content"]);
        let raw = &tool["rawInput"];
        let command = raw["command"].as_str().map(str::to_owned).or_else(|| {
            raw["command"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "))
        });
        let data = json!({
            "title": title,
            "toolKind": tool["kind"].as_str().unwrap_or("other"),
            "command": command,
            "detail": detail["output"],
            "diffs": detail["diffs"],
            "cwd": self.cfg.cwd,
            "options": options.iter().map(|o| json!({"optionId": o["optionId"], "name": o["name"], "kind": o["kind"]})).collect::<Vec<_>>(),
            "status": "pending",
            "selected": Value::Null,
        });
        self.show_card(&title, data, Answer::Agent(rpc_id));
    }

    /// `Cmd::Approve`: Allow or Deny on a card, with what the tool will do in its details.
    pub(super) fn ask_approval(&mut self, title: &str, detail: &str, reply: oneshot::Sender<bool>) {
        if self.turn.is_none() {
            return; // dropping `reply` refuses: no turn waits on it
        }
        let data = json!({
            "title": title, "toolKind": "other", "command": Value::Null, "detail": detail, "diffs": [],
            "options": [{"optionId": "allow", "name": "Allow", "kind": "allow_once"},
                        {"optionId": "deny", "name": "Deny", "kind": "reject_once"}],
            "status": "pending", "selected": Value::Null,
        });
        self.show_card(title, data, Answer::Codync(reply));
    }

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
        // Tapping it opens where the card is: the group, for a room turn.
        let target = self.hub.store.bot(&self.lane.chat).ok().flatten().map_or_else(|| self.cfg.clone(), |r| r.config);
        push::notify(
            &self.hub,
            &target,
            Some(&self.cfg.id),
            &format!("{} needs you", self.cfg.name),
            title,
            AlertKind::NeedsInput,
        );
    }

    /// A Codync card whose tool stopped waiting (it timed out, or its call was dropped) expires, buttons and all.
    pub(super) async fn expire_abandoned(&mut self) {
        let gone: Vec<String> = self
            .perms
            .iter()
            .filter(|(_, answer)| matches!(answer, Answer::Codync(reply) if reply.is_closed()))
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            self.answer_permission(&id, None).await;
        }
    }

    pub(super) async fn answer_permission(&mut self, entry_id: &str, option_id: Option<String>) {
        let Some(answer) = self.perms.remove(entry_id) else { return };
        let mut status = if option_id.is_some() { "answered" } else { "cancelled" };
        match answer {
            Answer::Agent(rpc_id) => {
                let outcome = match &option_id {
                    Some(o) => json!({"outcome": {"outcome": "selected", "optionId": o}}),
                    None => json!({"outcome": {"outcome": "cancelled"}}),
                };
                if let Some(conn) = &self.conn {
                    let _ = conn.acp.respond(rpc_id, outcome).await;
                }
            }
            Answer::Codync(reply) => {
                // The tool stopped waiting (see `expire_abandoned`): nothing acts on the answer.
                if reply.send(option_id.as_deref() == Some("allow")).is_err() {
                    status = "expired";
                }
            }
            Answer::Elicit(rpc_id) => {
                // Only a turn that ends gets here (`cancel_turn`): the card's answers go through `answer_question`.
                status = "cancelled";
                if let Some(conn) = &self.conn {
                    let _ = conn.acp.respond(rpc_id, json!({"action": "cancel"})).await;
                }
            }
        }
        if let Some(mut e) = self.hub.store.entry(entry_id) {
            e.data["status"] = status.into();
            e.data["selected"] = option_id.into();
            self.hub.set_entry(entry_id, &e.data);
        }
        self.after_answer();
    }

    /// Once the last open card is answered, the bot works on.
    pub(super) fn after_answer(&self) {
        if self.perms.is_empty() && self.turn.is_some() {
            self.hub.set_runtime(&self.cfg.id, |r| {
                r.status = BotStatus::Working;
                r.activity = "Continuing…".into();
            });
        }
    }
}
