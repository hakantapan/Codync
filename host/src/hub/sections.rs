//! Roster sections: a name the user files bots and group chats under (`BotConfig::section`), at
//! most one per row. A section exists while a bot is in it.

use anyhow::{Result, bail};

/// The longest section name, in characters.
const MAX_SECTION_CHARS: usize = 40;

/// A section name as bots keep it: trimmed, `None` when nothing is left. Refused when too long or
/// with a control character.
pub(super) fn normalize_section(raw: Option<&str>) -> Result<Option<String>> {
    let Some(name) = raw.map(str::trim).filter(|n| !n.is_empty()) else { return Ok(None) };
    if name.chars().count() > MAX_SECTION_CHARS {
        bail!("A section name can be at most {MAX_SECTION_CHARS} characters.");
    }
    if name.chars().any(char::is_control) {
        bail!("A section name can't contain control characters.");
    }
    Ok(Some(name.to_owned()))
}
