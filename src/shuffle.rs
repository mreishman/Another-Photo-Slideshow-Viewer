use std::{
    collections::{hash_map::RandomState, HashSet},
    hash::BuildHasher,
};

pub struct Shuffle {
    history: Vec<usize>,
    cursor: usize,
    remaining: Vec<usize>,
    pending_history: Option<usize>,
    random: RandomState,
    draws: u64,
}

impl Shuffle {
    pub fn new(length: usize, current: Option<usize>, failed: &HashSet<usize>) -> Self {
        let mut shuffle = Self {
            history: current.into_iter().collect(),
            cursor: 0,
            remaining: Vec::new(),
            pending_history: None,
            random: RandomState::new(),
            draws: 0,
        };
        shuffle.refill(length, failed);
        shuffle.remaining.retain(|&index| Some(index) != current);
        shuffle
    }

    fn refill(&mut self, length: usize, failed: &HashSet<usize>) {
        self.remaining = (0..length)
            .filter(|index| !failed.contains(index))
            .collect();
        for end in (1..self.remaining.len()).rev() {
            let position = (self.random.hash_one(self.draws) % (end as u64 + 1)) as usize;
            self.draws = self.draws.wrapping_add(1);
            self.remaining.swap(end, position);
        }
    }

    fn history_position(&self, direction: isize, failed: &HashSet<usize>) -> Option<usize> {
        let valid = |position: &usize| !failed.contains(&self.history[*position]);
        if direction < 0 {
            (0..self.cursor).rev().find(valid)
        } else {
            (self.cursor + 1..self.history.len()).find(valid)
        }
    }

    pub fn peek(&self, direction: isize, failed: &HashSet<usize>) -> Option<usize> {
        if let Some(position) = self.history_position(direction, failed) {
            return Some(self.history[position]);
        }
        if direction > 0 {
            return self
                .remaining
                .iter()
                .rev()
                .copied()
                .find(|index| !failed.contains(index));
        }
        None
    }

    pub fn can_loop(length: usize, current: usize, failed: &HashSet<usize>) -> bool {
        length.saturating_sub(failed.len()) > usize::from(!failed.contains(&current))
    }

    pub fn step(
        &mut self,
        direction: isize,
        length: usize,
        repeat: bool,
        failed: &HashSet<usize>,
    ) -> Option<usize> {
        if let Some(position) = self.history_position(direction, failed) {
            self.pending_history = Some(position);
            return Some(self.history[position]);
        }
        if direction < 0 {
            return None;
        }
        while let Some(&index) = self.remaining.last() {
            if !failed.contains(&index) {
                return Some(index);
            }
            self.remaining.pop();
        }

        let current = *self.history.get(self.cursor)?;
        if !repeat || !Self::can_loop(length, current, failed) {
            return None;
        }
        self.refill(length, failed);
        let last = self.remaining.len() - 1;
        if self.remaining[last] == current {
            self.remaining.swap(0, last);
        }
        self.remaining.last().copied()
    }

    pub fn reject(&mut self) {
        self.pending_history = None;
    }

    pub fn accept(&mut self, index: usize) {
        if let Some(position) = self.pending_history.take() {
            self.cursor = position;
            return;
        }
        if self.history.get(self.cursor) == Some(&index) {
            return;
        }
        if self.remaining.last() == Some(&index) {
            self.remaining.pop();
        } else {
            self.remaining.retain(|&candidate| candidate != index);
        }
        if self.history.is_empty() {
            self.record(index);
        } else {
            self.cursor += 1;
            self.history.insert(self.cursor, index);
        }
    }

    fn record(&mut self, index: usize) {
        self.cursor = self.history.len();
        self.history.push(index);
    }
}

#[cfg(test)]
mod tests {
    use super::Shuffle;
    use std::collections::HashSet;

    #[test]
    fn visits_each_readable_photo_once_and_stops_without_looping() {
        let mut failed = HashSet::new();
        let mut shuffle = Shuffle::new(6, Some(0), &failed);
        let corrupt = shuffle.step(1, 6, false, &failed).unwrap();
        failed.insert(corrupt);
        shuffle.reject();

        let mut seen = HashSet::from([0]);
        while let Some(index) = shuffle.step(1, 6, false, &failed) {
            assert!(seen.insert(index));
            shuffle.accept(index);
        }
        assert_eq!(seen.len(), 5);
        assert!(!seen.contains(&corrupt));
    }

    #[test]
    fn previous_retraces_history_across_a_loop_boundary() {
        let failed = HashSet::new();
        let mut shuffle = Shuffle::new(4, Some(0), &failed);
        let mut previous = 0;
        for _ in 0..3 {
            previous = shuffle.step(1, 4, true, &failed).unwrap();
            shuffle.accept(previous);
        }
        let first = shuffle.step(1, 4, true, &failed).unwrap();
        assert_ne!(first, previous);
        shuffle.accept(first);
        assert_eq!(shuffle.step(-1, 4, true, &failed), Some(previous));
        shuffle.accept(previous);
        assert_eq!(shuffle.step(1, 4, true, &failed), Some(first));
        shuffle.accept(first);

        let mut seen = HashSet::from([first]);
        for _ in 0..3 {
            let index = shuffle.step(1, 4, true, &failed).unwrap();
            assert!(seen.insert(index));
            shuffle.accept(index);
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn thumbnail_jump_counts_toward_the_current_pass() {
        let failed = HashSet::new();
        let mut shuffle = Shuffle::new(3, Some(0), &failed);
        shuffle.accept(2);
        assert_eq!(shuffle.step(-1, 3, false, &failed), Some(0));
        shuffle.accept(0);
        assert_eq!(shuffle.step(1, 3, false, &failed), Some(2));
        shuffle.accept(2);
        assert_eq!(shuffle.step(1, 3, false, &failed), Some(1));
        shuffle.accept(1);
        assert_eq!(shuffle.step(1, 3, false, &failed), None);
    }
}
