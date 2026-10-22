//! The roster as shown: its order, and moving through it.

use super::{App, Bot};

impl App {
    /// Roster order: pinned first, then where the user put them, then most recent (same as the phone).
    pub fn roster(&self) -> Vec<&Bot> {
        let mut v: Vec<&Bot> = self.bots.values().filter(|b| !b.hidden).collect();
        v.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(a.position.cmp(&b.position))
                .then(b.last_at.cmp(&a.last_at))
                .then(a.name.cmp(&b.name))
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
