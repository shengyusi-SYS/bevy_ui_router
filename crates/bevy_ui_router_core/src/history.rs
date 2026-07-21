//! ```compile_fail
//! use bevy_ui_router_core::prelude::{HistorySnapshot, MemoryHistory};
//!
//! let _ = HistorySnapshot::project(&MemoryHistory::new());
//! ```

use crate::location::RouteLocation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistorySnapshot {
    entries: Vec<RouteLocation>,
    cursor: Option<usize>,
}

impl HistorySnapshot {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn project(history: &MemoryHistory) -> Self {
        Self {
            entries: history.entries.clone(),
            cursor: history.cursor,
        }
    }

    pub fn current(&self) -> Option<&RouteLocation> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    pub fn entries(&self) -> &[RouteLocation] {
        &self.entries
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn can_back(&self) -> bool {
        self.cursor.is_some_and(|cursor| cursor > 0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HistoryError {
    Empty,
    AtStart,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MemoryHistory {
    entries: Vec<RouteLocation>,
    cursor: Option<usize>,
}

impl MemoryHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, location: RouteLocation) {
        match self.cursor {
            Some(cursor) => {
                self.entries.truncate(cursor + 1);
                self.entries.push(location);
                self.cursor = Some(self.entries.len() - 1);
            }
            None => {
                self.entries.push(location);
                self.cursor = Some(0);
            }
        }
    }

    pub fn replace(&mut self, location: RouteLocation) {
        match self.cursor {
            Some(cursor) => self.entries[cursor] = location,
            None => self.push(location),
        }
    }

    pub fn back(&mut self) -> Result<RouteLocation, HistoryError> {
        let cursor = self.cursor.ok_or(HistoryError::Empty)?;
        if cursor == 0 {
            return Err(HistoryError::AtStart);
        }

        let next_cursor = cursor - 1;
        self.cursor = Some(next_cursor);
        Ok(self.entries[next_cursor].clone())
    }

    pub fn current(&self) -> Option<&RouteLocation> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_a_frozen_projection_of_history_state() {
        fn assert_traits<T: Clone + std::fmt::Debug + Eq + PartialEq>() {}
        assert_traits::<HistorySnapshot>();

        let mut history = MemoryHistory::new();
        history.push(RouteLocation::new("/"));
        history.push(RouteLocation::new("/settings"));
        let snapshot = HistorySnapshot::project(&history);

        history.back().unwrap();

        assert_eq!(
            snapshot.current().map(RouteLocation::path),
            Some("/settings")
        );
        assert_eq!(
            snapshot
                .entries()
                .iter()
                .map(RouteLocation::path)
                .collect::<Vec<_>>(),
            vec!["/", "/settings"]
        );
        assert_eq!(snapshot.cursor(), Some(1));
        assert_eq!(snapshot.len(), 2);
        assert!(snapshot.can_back());
    }

    #[test]
    fn push_replace_and_back_track_memory_locations() {
        let mut history = MemoryHistory::new();

        history.push(RouteLocation::new("/"));
        history.push(RouteLocation::new("/settings"));
        history.replace(RouteLocation::new("/settings/profile"));

        assert_eq!(
            history.current().map(RouteLocation::path),
            Some("/settings/profile")
        );
        assert_eq!(
            history.back().map(|location| location.path().to_owned()),
            Ok("/".to_owned())
        );
        assert_eq!(history.current().map(RouteLocation::path), Some("/"));
    }

    #[test]
    fn back_reports_empty_and_start_boundaries() {
        let mut history = MemoryHistory::new();

        assert_eq!(history.back(), Err(HistoryError::Empty));

        history.push(RouteLocation::new("/"));

        assert_eq!(history.back(), Err(HistoryError::AtStart));
    }

    #[test]
    fn push_after_back_truncates_forward_entries() {
        let mut history = MemoryHistory::new();

        history.push(RouteLocation::new("/"));
        history.push(RouteLocation::new("/settings"));
        history.push(RouteLocation::new("/settings/profile"));

        assert_eq!(
            history.back().map(|location| location.path().to_owned()),
            Ok("/settings".to_owned())
        );

        history.push(RouteLocation::new("/help"));

        assert_eq!(history.len(), 3);
        assert_eq!(history.current().map(RouteLocation::path), Some("/help"));
        assert_eq!(
            history.back().map(|location| location.path().to_owned()),
            Ok("/settings".to_owned())
        );
    }
}
