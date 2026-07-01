//! Paste strategies for FiloStack queue management.

use std::collections::VecDeque;

/// Trait for paste order strategies.
pub trait PasteStrategy {
    /// Pop the next item from the queue.
    fn pop(&self, queue: &mut VecDeque<String>) -> Option<String>;

    /// Name of this strategy for UI display.
    fn name(&self) -> &'static str;

    /// Index (1-based) of the next item that will be popped, for UI.
    fn next_index(&self, total: usize) -> usize;
}

/// FIFO (First In, First Out) — queue mode.
pub struct QueueStrategy;

impl PasteStrategy for QueueStrategy {
    fn pop(&self, queue: &mut VecDeque<String>) -> Option<String> {
        queue.pop_front()
    }

    fn name(&self) -> &'static str {
        "queue"
    }

    fn next_index(&self, _total: usize) -> usize {
        1 // always the first item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_strategy_fifo() {
        let strategy = QueueStrategy;
        let mut queue: VecDeque<String> = ["a", "b", "c"].into_iter().map(String::from).collect();

        assert_eq!(strategy.pop(&mut queue), Some("a".into()));
        assert_eq!(strategy.pop(&mut queue), Some("b".into()));
        assert_eq!(strategy.pop(&mut queue), Some("c".into()));
        assert_eq!(strategy.pop(&mut queue), None);
    }

    #[test]
    fn test_queue_empty() {
        let strategy = QueueStrategy;
        let mut queue = VecDeque::new();
        assert_eq!(strategy.pop(&mut queue), None);
    }
}
