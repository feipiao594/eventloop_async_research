use super::Task;
use std::time::Instant;

pub(crate) struct Timer {
    pub(crate) when: Instant,
    pub(crate) seq: u64,
    pub(crate) task: Task,
}

impl PartialEq for Timer {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when && self.seq == other.seq
    }
}

impl Eq for Timer {}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.when
            .cmp(&other.when)
            .then_with(|| self.seq.cmp(&other.seq))
    }
}
