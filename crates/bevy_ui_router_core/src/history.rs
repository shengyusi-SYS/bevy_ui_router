use crate::location::RouteLocation;

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HistoryIntent {
    Push(RouteLocation),
}

#[allow(dead_code)]
impl HistoryIntent {
    pub(crate) fn publish(self, history: &mut MemoryHistory) {
        match self {
            Self::Push(location) => history.push(location),
        }
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
