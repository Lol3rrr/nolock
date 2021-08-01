use std::fmt::Display;

// Internal Storage
//
// Bits:
// 1. IsSafe
// 2-32. Cycle
// 33-64. Index
pub struct QueueEntryData(u64);

impl QueueEntryData {
    /// Creates a new EntryData-Instance based on the given Data
    pub fn new(is_safe: bool, cycle: u32, index: u32) -> Self {
        let base_val = if is_safe { 0x8000000000000000 } else { 0 };
        Self(base_val | (((cycle & 0x7fffffff) as u64) << 32) | (index as u64))
    }

    /// Checks if the decoded EntryData is marked as Safe
    pub fn is_safe(&self) -> bool {
        (self.0 >> 63) == 1
    }

    /// Gets the Cycle that is decoded in this EntryData
    pub fn cycle(&self) -> u32 {
        ((self.0 >> 32) & 0x7fffffff) as u32
    }

    /// Gets the Index that is decoded in this EntryData
    pub fn index(&self) -> u32 {
        (self.0 & 0xffffffff) as u32
    }
}
impl From<u64> for QueueEntryData {
    fn from(data: u64) -> Self {
        Self(data)
    }
}
impl Into<u64> for QueueEntryData {
    fn into(self) -> u64 {
        self.0
    }
}
impl Display for QueueEntryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(IsSafe: {}, Cycle: {}, Index: {})",
            self.is_safe(),
            self.cycle(),
            self.index()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_entry_is_safe() {
        assert_eq!(true, QueueEntryData::from(0x8000000000000000).is_safe());
        assert_eq!(true, QueueEntryData::from(0xd000000000000000).is_safe());
        assert_eq!(false, QueueEntryData::from(0x4000000000000000).is_safe());
    }
    #[test]
    fn queue_entry_cycle() {
        assert_eq!(0, QueueEntryData::from(0x8000000000000000).cycle());
        assert_eq!(1, QueueEntryData::from(0x8000000100000000).cycle());
        assert_eq!(0x70000000, QueueEntryData::from(0xf000000000000000).cycle());
    }
    #[test]
    fn queue_entry_index() {
        assert_eq!(0x80000000, QueueEntryData::from(0x8123456780000000).index());
    }
    #[test]
    fn queue_entry_to_value() {
        assert_eq!(
            0x8000000000000000u64,
            QueueEntryData::new(true, 0, 0).into()
        );
        assert_eq!(
            0x8000001500000000u64,
            QueueEntryData::new(true, 0x15, 0).into()
        );
        assert_eq!(
            0x8000001500000015u64,
            QueueEntryData::new(true, 0x15, 0x15).into()
        );
    }
}
