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
    // Answers that aren't `{field: [strings]}` are refused, and the card keeps waiting.
    let error =
        host.refused("respondQuestion", json!({"entryId": card["id"], "answers": {"question_0": "PostgreSQL"}})).await;
    assert!(error.contains("list of strings"), "{error}");
    assert_eq!(wait_for(&host, &bot, |e| e["id"] == card["id"]).await["data"]["status"], "pending");
    assert_eq!(card["data"]["title"], "Which database?");
    let fields = card["data"]["fields"].as_array().unwrap();
    assert_eq!(fields[0]["kind"], "single");
    assert_eq!(
        fields[0]["options"][0],
        json!({"value": "PostgreSQL", "label": "PostgreSQL", "description": "Relational"})
    );
    assert_eq!(fields[1]["kind"], "text");
    // The roster follows the card a moment later.
    let deadline = Instant::now() + Duration::from_secs(5);
    while host.snapshot().await["bots"][0]["status"] != "needsInput" {
        assert!(Instant::now() < deadline, "the bot never showed it needs input");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

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
