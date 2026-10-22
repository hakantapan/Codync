//! An agent's form (`elicitation/create`, how Claude Code asks `AskUserQuestion`) as the fields of a
//! question card, and the card's answers as the form's content.

use crate::agent::acp;
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
    /// Free text, such as `AskUserQuestion`'s "Other" box.
    Text,
}

/// The card's fields for a form's `requestedSchema`, in its order: `{key, kind, title, description, options}`.
/// `None` when a field asks for something the card can't (a number, a yes/no, an object): the form is declined.
pub(super) fn fields(schema: &Value) -> Option<Vec<Value>> {
    schema["properties"].as_object()?.iter().map(|(key, p)| field(key, p)).collect()
}

/// claude-agent-acp's refusal-fallback dialog (`refusalFallbackToCreateRequest`, dialog kind
/// `refusal_fallback_prompt`): its one `choice` field offers `retry_fallback`, the dialog's own result value.
pub(super) fn is_refusal_fallback(schema: &Value) -> bool {
    schema["properties"]["choice"]["oneOf"].as_array().is_some_and(|o| o.iter().any(|o| o["const"] == "retry_fallback"))
}

fn field(key: &str, p: &Value) -> Option<Value> {
    let (kind, options) = match p["type"].as_str()? {
        "string" => options(p).map_or((FieldKind::Text, Vec::new()), |o| (FieldKind::Single, o)),
        "array" => (FieldKind::Multi, options(&p["items"])?),
        _ => return None,
    };
    // Only strings go on: the phone's decode of the whole entry fails on any other type.
    Some(json!({
        "key": key, "kind": kind, "title": p["title"].as_str(), "description": p["description"].as_str(), "options": options
    }))
}

/// A choice field's options: titled `oneOf` / `anyOf` entries, or a bare `enum`. A list with no string options
/// to pick from is no choice at all (`None`).
fn options(p: &Value) -> Option<Vec<Value>> {
    let options: Vec<Value> = if let Some(titled) = p["oneOf"].as_array().or_else(|| p["anyOf"].as_array()) {
        titled
            .iter()
            .filter_map(|o| {
                let value = o["const"].as_str()?;
                let label = o["title"].as_str().unwrap_or(value);
                Some(json!({"value": value, "label": label, "description": o["description"].as_str()}))
            })
            .collect()
    } else {
        let bare = p["enum"].as_array()?;
        bare.iter().filter_map(Value::as_str).map(|v| json!({"value": v, "label": v, "description": null})).collect()
    };
    (!options.is_empty()).then_some(options)
}

/// The card in one line, for clients that can't show it (the bot's activity line, the push body): the question and
/// the first choice field's options, as `Which database? SQLite / PostgreSQL / MySQL (+1 more)`. With several
/// questions `message` is only "Please answer the following questions.", so the first one's own text stands in.
pub(super) fn summary(message: &str, fields: &[Value]) -> String {
    const SHOWN: usize = 3;
    let mut choices = fields.iter().filter(|f| f["kind"] != "text");
    let first = choices.next();
    let question = match (first, choices.next()) {
        (Some(f), Some(_)) => f["description"].as_str().or_else(|| f["title"].as_str()).unwrap_or(message),
        _ => message,
    };
    let labels: Vec<&str> =
        first.and_then(|f| f["options"].as_array()).into_iter().flatten().filter_map(|o| o["label"].as_str()).collect();
    let mut line = question.to_owned();
    if !labels.is_empty() {
        line = format!("{line} {}", labels[..labels.len().min(SHOWN)].join(" / "));
    }
    if labels.len() > SHOWN {
        line = format!("{line} (+{} more)", labels.len() - SHOWN);
    }
    acp::truncate(&line, 200)
}

/// The form's content for the card's answers (`{field key: [chosen values or typed text]}`), and what the
/// answered card shows: one line per answered field. Blank answers, and values that aren't one of a choice
/// field's options, are left out (the answers come from a client). Only a `multi` field keeps more than one value.
pub(super) fn content(fields: &[Value], answers: &Value) -> (Value, Vec<String>) {
    let mut content = Map::new();
    let mut lines = Vec::new();
    for f in fields {
        let Some(key) = f["key"].as_str() else { continue };
        let options = f["options"].as_array().map_or(&[][..], Vec::as_slice);
        let label = |v: &str| options.iter().find(|o| o["value"] == v).and_then(|o| o["label"].as_str());
        let multi = f["kind"] == "multi";
        let mut picked: Vec<&str> = answers[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty() && (f["kind"] == "text" || label(v).is_some()))
            .collect();
        if !multi {
            picked.truncate(1);
        }
        let Some(first) = picked.first() else { continue };
        content.insert(key.to_owned(), if multi { json!(picked) } else { json!(first) });
        lines.push(picked.iter().map(|v| label(v).unwrap_or(v)).collect::<Vec<_>>().join(", "));
    }
    (Value::Object(content), lines)
}

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
    fn only_string_titles_and_descriptions_go_to_the_card() {
        let schema = json!({"properties": {"q": {"type": "string", "title": 5, "description": ["x"],
            "oneOf": [{"const": "a", "description": {"x": 1}}]}}});
        let f = fields(&schema).unwrap();
        assert_eq!((&f[0]["title"], &f[0]["description"]), (&Value::Null, &Value::Null));
        assert_eq!(f[0]["options"][0]["description"], Value::Null);
    }

    #[test]
    fn a_choice_list_without_options_is_no_choice() {
        let text = fields(&json!({"properties": {"s": {"type": "string", "oneOf": [{"title": "no const"}]}}})).unwrap();
        assert_eq!((&text[0]["kind"], &text[0]["options"]), (&json!("text"), &json!([])));
        assert!(fields(&json!({"properties": {"a": {"type": "array", "items": {"anyOf": []}}}})).is_none());
        assert!(fields(&json!({"properties": {"a": {"type": "array", "items": {"enum": [1, 2]}}}})).is_none());
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
    fn a_single_field_keeps_only_its_first_value() {
        let fields = fields(&ask_user_question()).unwrap();
        let (content, lines) = content(&fields, &json!({"question_0": ["SQLite", "PostgreSQL"]}));
        assert_eq!(content, json!({"question_0": "SQLite"}));
        assert_eq!(lines, ["SQLite"]);
    }

    #[test]
    fn typed_text_is_kept_as_typed() {
        let fields = fields(&ask_user_question()).unwrap();
        let (content, lines) = content(&fields, &json!({"question_0_custom": ["MySQL, please"]}));
        assert_eq!(content, json!({"question_0_custom": "MySQL, please"}));
        assert_eq!(lines, ["MySQL, please"]);
    }

    #[test]
    fn the_summary_names_the_first_question_and_its_choices() {
        let options = |labels: &[&str]| labels.iter().map(|l| json!({"const": l, "title": l})).collect::<Vec<_>>();
        let several = fields(&json!({"properties": {
            "question_0": {"type": "string", "title": "Database", "description": "Which database should we use?",
                           "oneOf": options(&["SQLite", "PostgreSQL", "MySQL", "Redis"])},
            "question_0_custom": {"type": "string", "title": "Other"},
            "question_1": {"type": "array", "description": "Which extras?", "items": {"anyOf": options(&["Cache"])}}}}))
        .unwrap();
        assert_eq!(
            summary("Please answer the following questions.", &several),
            "Which database should we use? SQLite / PostgreSQL / MySQL (+1 more)"
        );
        // One question carries its text in `message`.
        let one = fields(&json!({"properties": {"question_0": {"type": "string", "oneOf": options(&["Yes", "No"])},
            "question_0_custom": {"type": "string"}}}))
        .unwrap();
        assert_eq!(summary("Ship it?", &one), "Ship it? Yes / No");
        assert_eq!(summary("Your name?", &one[1..]), "Your name?");
        let long = summary(&"x".repeat(300), &one);
        assert!(long.ends_with('…') && long.chars().count() == 201, "{long}");
    }

    #[test]
    fn no_answers_is_empty() {
        let fields = fields(&ask_user_question()).unwrap();
        let (content, lines) = content(&fields, &json!({}));
        assert_eq!(content, json!({}));
        assert_eq!(lines, Vec::<String>::new());
    }
}
