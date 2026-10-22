//! The roster as shown: its order and its lines (pinned bots, then each section under its label,
//! sorted by name, then bots in none), and moving through it.

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashSet};

use super::super::manage::{Submit, section_fields};
use super::{App, Bot, Overlay};

/// The first host with roster sections (`section`, `renameSection`).
const SECTIONS_SINCE: &str = "2.23.0";

/// One line of the roster.
pub enum RosterLine<'a> {
    /// `PINNED` above the pinned bots, `BOTS` above the bots in no section.
    Label(&'static str),
    /// A section's label: how many bots it holds, their unread messages (shown while folded) and the
    /// first of them, which the cursor lands on while it is folded.
    Section {
        name: &'a str,
        bots: usize,
        unread: i64,
        folded: bool,
        first: &'a Bot,
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
                        first: b,
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

impl RosterLine<'_> {
    /// The bot the cursor selects on this line: a bot's own, a folded section's first.
    fn stop(&self) -> Option<&str> {
        match self {
            Self::Bot(b) | Self::Section { first: b, folded: true, .. } => Some(b.id.as_str()),
            _ => None,
        }
    }
}

/// What the cursor can stop on, in order, as (line, bot id): the bots shown and each folded section
/// (at its first bot). The number keys, j/k and the compact strip all count these.
pub fn stops<'l>(lines: &'l [RosterLine<'_>]) -> Vec<(usize, &'l str)> {
    lines.iter().enumerate().filter_map(|(i, l)| Some((i, l.stop()?))).collect()
}

/// The bot `delta` stops from the cursor (no cursor: the first). A folded section is one stop.
fn step(lines: &[RosterLine<'_>], selected: Option<&Bot>, delta: isize) -> Option<String> {
    let stops = stops(lines);
    let last = stops.len().checked_sub(1)?;
    let at = cursor_line(lines, selected).and_then(|c| stops.iter().position(|&(i, _)| i == c));
    let next = at.map_or(0, |i| i.saturating_add_signed(delta).min(last));
    Some(stops[next].1.to_owned())
}

const NOT_IN_A_SECTION: &str = "This bot isn't in a section";

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

    /// j/k in the roster: the next shown bot, a folded section counting as one.
    pub(super) fn move_bot(&mut self, delta: isize) {
        let Some(id) = step(&roster_lines(&self.roster(), &self.folded), self.bot(), delta) else { return };
        self.select(&id);
    }

    /// The bot key `n` (from 0 for `1`) selects: the n-th stop, a folded section being one (its first bot).
    pub(super) fn stop_at(&self, n: usize) -> Option<String> {
        let lines = roster_lines(&self.roster(), &self.folded);
        stops(&lines).get(n).map(|&(_, id)| id.to_owned())
    }

    /// `z`: folds the section under the cursor, or unfolds it.
    pub(super) fn toggle_section(&mut self) {
        let Some(bot) = self.bot() else { return };
        let Some(name) = bot.shown_section().map(str::to_owned) else {
            self.flash(NOT_IN_A_SECTION);
            return;
        };
        if !self.folded.remove(&name) {
            self.folded.insert(name);
        }
    }

    /// `w`: "Move to section" for the selected bot: one of the sections, a new name, or empty for none.
    pub(super) fn ask_section(&mut self) {
        if self.bot().is_none() || !self.sections_supported() {
            return;
        }
        let Some(bot) = self.bot() else { return };
        let names: BTreeSet<&str> = self.bots.values().filter_map(|o| o.section.as_deref()).collect();
        let mut note = "Empty takes it out of its section; a new name makes one.".to_owned();
        if !names.is_empty() {
            let list: Vec<&str> = names.into_iter().collect();
            note = format!("{note} Sections: {}", list.join(", "));
        }
        let fields = section_fields("Move to section", note, bot.section.as_deref(), Submit::Section(bot.id.clone()));
        self.overlays.push(Overlay::Fields(Box::new(fields)));
    }

    /// `W`: "Rename section" for the section under the cursor: an existing name merges the two, empty
    /// removes the section (its bots stay).
    pub(super) fn ask_rename_section(&mut self) {
        if self.bot().is_none() || !self.sections_supported() {
            return;
        }
        let Some(name) = self.bot().and_then(Bot::shown_section).map(str::to_owned) else {
            self.flash(NOT_IN_A_SECTION);
            return;
        };
        let note = "Empty removes the section; its bots stay. A name in use merges them.".to_owned();
        let fields = section_fields("Rename section", note, Some(name.as_str()), Submit::RenameSection(name.clone()));
        self.overlays.push(Overlay::Fields(Box::new(fields)));
    }

    /// Whether the host has sections; says what to update when it doesn't.
    fn sections_supported(&mut self) -> bool {
        if self.host_version.is_empty() {
            self.flash("Still connecting: try again in a moment.");
            return false;
        }
        let supported = crate::compat::at_least(&self.host_version, SECTIONS_SINCE);
        if !supported {
            let why = format!("Update Codync on {}: sections need {SECTIONS_SINCE} or newer.", self.host);
            self.flash(&why);
        }
        supported
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Bot, Overlay, test_app};
    use super::{App, NOT_IN_A_SECTION, RosterLine, cursor_line, roster_lines};
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

    #[tokio::test]
    async fn j_and_k_pass_a_folded_section_as_one_row() {
        let mut app = test_app();
        for (id, section, last_at) in [("a", Some("Work"), 3), ("b", Some("Work"), 2), ("c", None, 1)] {
            add(&mut app, id, false, section, last_at, 0);
        }
        app.selected = Some("b".into());
        app.toggle_section();
        assert!(app.folded.contains("Work"));
        let lines = roster_lines(&app.roster(), &app.folded);
        assert_eq!(cursor_line(&lines, app.bot()), Some(0), "the cursor is on the folded label");
        app.move_bot(1);
        assert_eq!(app.selected.as_deref(), Some("c"));
        app.move_bot(-1);
        assert_eq!(app.selected.as_deref(), Some("a"), "back onto the label: its first bot");
        app.toggle_section();
        assert!(app.folded.is_empty());
        app.move_bot(1);
        assert_eq!(app.selected.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn number_keys_count_a_folded_section_as_one_row() {
        let mut app = test_app();
        for (id, section, last_at) in [("a", Some("Work"), 3), ("b", Some("Work"), 2), ("c", None, 1)] {
            add(&mut app, id, false, section, last_at, 0);
        }
        assert_eq!([0, 1, 2, 3].map(|n| app.stop_at(n)), [Some("a".into()), Some("b".into()), Some("c".into()), None]);
        app.folded.insert("Work".into());
        let at = [0, 1, 2].map(|n| app.stop_at(n));
        assert_eq!(at, [Some("a".into()), Some("c".into()), None], "1 is the section's first bot, 2 the next row");
    }

    #[tokio::test]
    async fn z_outside_a_section_only_says_so() {
        let mut app = test_app();
        add(&mut app, "p", true, Some("Work"), 1, 0);
        app.selected = Some("p".into());
        app.toggle_section();
        assert!(app.folded.is_empty(), "a pinned bot shows in no section");
        assert_eq!(app.hint.as_ref().map(|(s, _)| s.as_str()), Some(NOT_IN_A_SECTION));
        app.hint = None;
        app.selected = None;
        app.toggle_section();
        assert!(app.hint.is_none(), "nothing selected: nothing to say");
    }

    #[tokio::test]
    async fn sections_wait_for_a_host_that_has_them() {
        let mut app = test_app();
        add(&mut app, "a", false, Some("Work"), 0, 0);
        app.ask_section();
        app.ask_rename_section();
        assert!(app.overlays.is_empty() && app.hint.is_none(), "nothing selected: silent, like z");
        app.selected = Some("a".into());
        app.ask_section();
        assert!(app.overlays.is_empty(), "no hello yet: the host's version is unknown");
        assert_eq!(app.hint.as_ref().map(|(s, _)| s.as_str()), Some("Still connecting: try again in a moment."));
        app.host_version = "2.22.2".into();
        app.ask_section();
        assert!(app.overlays.is_empty(), "an older host would drop the name");
        app.host_version = "2.23.0".into();
        app.ask_section();
        let Some(Overlay::Fields(f)) = app.overlays.last() else { panic!("the section prompt") };
        assert_eq!((f.title.as_str(), f.inputs[0].ed.text.as_str()), ("Move to section", "Work"));
        app.overlays.clear();
        app.ask_rename_section();
        let Some(Overlay::Fields(f)) = app.overlays.last() else { panic!("the rename prompt") };
        assert_eq!((f.title.as_str(), f.inputs[0].placeholder.as_str()), ("Rename section", "Section name"));
    }
}
