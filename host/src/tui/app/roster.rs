//! The roster as shown: its order and its lines (pinned bots, then each section under its label,
//! sorted by name, then bots in none), and moving through it.

use std::cmp::Reverse;
use std::collections::HashSet;

use super::{App, Bot};

/// One line of the roster.
pub enum RosterLine<'a> {
    /// `PINNED` above the pinned bots, `BOTS` above the bots in no section.
    Label(&'static str),
    /// A section's label: how many bots it holds and their unread messages (shown while folded).
    Section {
        name: &'a str,
        bots: usize,
        unread: i64,
        folded: bool,
    },
    Bot(&'a Bot),
}

impl Bot {
    /// The section the roster shows it in: none while pinned (pinned bots stay on top, outside sections).
    pub fn shown_section(&self) -> Option<&str> {
        if self.pinned { None } else { self.section.as_deref() }
    }
}

/// `roster` (in `App::roster` order) as lines; a folded section is its label alone. Without pinned
/// bots or sections there is nothing to tell apart, so no labels.
pub fn roster_lines<'a>(roster: &[&'a Bot], folded: &HashSet<String>) -> Vec<RosterLine<'a>> {
    let labeled = roster.iter().any(|b| b.pinned || b.shown_section().is_some());
    let mut lines = vec![];
    let mut group = None;
    for &b in roster {
        let here = (b.pinned, b.shown_section());
        if labeled && group != Some(here) {
            lines.push(match here {
                (true, _) => RosterLine::Label("PINNED"),
                (false, None) => RosterLine::Label("BOTS"),
                (false, Some(name)) => {
                    let members: Vec<&Bot> =
                        roster.iter().copied().filter(|m| m.shown_section() == Some(name)).collect();
                    RosterLine::Section {
                        name,
                        bots: members.len(),
                        unread: members.iter().map(|m| m.unread).sum(),
                        folded: folded.contains(name),
                    }
                }
            });
        }
        group = Some(here);
        if !here.1.is_some_and(|name| folded.contains(name)) {
            lines.push(RosterLine::Bot(b));
        }
    }
    lines
}

/// The line the cursor is on: the selected bot's, or its folded section's label.
pub fn cursor_line(lines: &[RosterLine<'_>], selected: Option<&Bot>) -> Option<usize> {
    let selected = selected?;
    lines.iter().position(|line| match line {
        RosterLine::Bot(b) => b.id == selected.id,
        RosterLine::Section { name, folded: true, .. } => selected.shown_section() == Some(*name),
        _ => false,
    })
}

impl App {
    /// Roster order: pinned first, then sections by name, then bots in none; inside each, where the
    /// user put them, then most recent (same as the phone).
    pub fn roster(&self) -> Vec<&Bot> {
        let mut v: Vec<&Bot> = self.bots.values().filter(|b| !b.hidden).collect();
        // ponytail: `to_lowercase` order, not locale collation ("İş" may land off for Turkish); a
        // collation crate (icu_collator) once someone notices.
        v.sort_by_cached_key(|b| {
            let section = b.shown_section().map(|s| (s.to_lowercase(), s.to_owned()));
            (!b.pinned, section.is_none(), section, b.position, Reverse(b.last_at), b.name.clone())
        });
        v
    }

    pub(super) fn move_bot(&mut self, delta: isize) {
        let ids: Vec<String> = self.roster().iter().map(|b| b.id.clone()).collect();
        if ids.is_empty() {
            return;
        }
        let i = self.selected.as_ref().and_then(|s| ids.iter().position(|x| x == s));
        let n = isize::try_from(ids.len()).unwrap_or(isize::MAX);
        let next = match i {
            Some(i) => (isize::try_from(i).unwrap_or(0) + delta).clamp(0, n - 1),
            None => 0,
        };
        let id = ids[usize::try_from(next).unwrap_or(0)].clone();
        self.select(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Bot, test_app};
    use super::{App, RosterLine, roster_lines};
    use serde_json::json;

    fn add(app: &mut App, id: &str, pinned: bool, section: Option<&str>, last_at: i64, unread: i64) {
        let v = json!({"id": id, "pinned": pinned, "section": section, "lastAt": last_at, "unread": unread});
        app.bots.insert(id.into(), Bot::parse(&v).expect("a bot with an id parses"));
    }

    /// The roster as text: labels, `▾ name (bots)` or `▸ name (bots) unread`, bot ids.
    fn shown(app: &App) -> Vec<String> {
        roster_lines(&app.roster(), &app.folded)
            .iter()
            .map(|line| match line {
                RosterLine::Label(l) => (*l).to_owned(),
                RosterLine::Section { name, bots, folded: false, .. } => format!("▾ {name} ({bots})"),
                RosterLine::Section { name, bots, unread, .. } => format!("▸ {name} ({bots}) {unread}"),
                RosterLine::Bot(b) => b.id.clone(),
            })
            .collect()
    }

    #[tokio::test]
    async fn pinned_bots_then_sections_by_name_then_the_rest() {
        let mut app = test_app();
        for (id, pinned, section, last_at, unread) in [
            ("p", true, Some("Work"), 9, 0),
            ("w1", false, Some("Work"), 3, 2),
            ("w2", false, Some("Work"), 2, 1),
            ("h", false, Some("home"), 1, 0),
            ("x", false, None, 5, 0),
        ] {
            add(&mut app, id, pinned, section, last_at, unread);
        }
        // "home" before "Work": case doesn't count; the pinned bot stays on top and out of Work.
        assert_eq!(shown(&app), ["PINNED", "p", "▾ home (1)", "h", "▾ Work (2)", "w1", "w2", "BOTS", "x"]);
        app.folded.insert("Work".into());
        assert_eq!(shown(&app), ["PINNED", "p", "▾ home (1)", "h", "▸ Work (2) 3", "BOTS", "x"]);
    }
}
