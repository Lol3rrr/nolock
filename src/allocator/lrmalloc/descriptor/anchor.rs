use std::{fmt::Debug, sync::atomic};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AnchorState {
    Empty,
    Partial,
    Full,
}

impl From<u64> for AnchorState {
    fn from(raw: u64) -> Self {
        match raw {
            0b00 => Self::Empty,
            0b01 => Self::Partial,
            0b10 => Self::Full,
            _ => unreachable!("The Anchor-State has been corrupted"),
        }
    }
}
impl From<AnchorState> for u64 {
    fn from(raw: AnchorState) -> Self {
        match raw {
            AnchorState::Empty => 0b00,
            AnchorState::Partial => 0b01,
            AnchorState::Full => 0b10,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct Anchor {
    pub state: AnchorState,
    /// The First Available Block in the Super-Block
    pub avail: u32,
    /// The Number of Free-Blocks
    pub count: u32,
}

impl Anchor {
    pub fn new(max_count: u32) -> Self {
        Self {
            state: AnchorState::Full,
            avail: max_count,
            count: 0,
        }
    }
}

impl From<u64> for Anchor {
    fn from(raw: u64) -> Self {
        let state_bits = raw >> 62;
        let avail_bits = (raw & (u64::MAX >> 2)) >> 31;
        let count_bits = raw & !(u64::MAX << 31);

        let state = state_bits.into();
        let avail = avail_bits as u32;
        let count = count_bits as u32;

        Self {
            state,
            avail,
            count,
        }
    }
}
impl From<Anchor> for u64 {
    fn from(raw: Anchor) -> Self {
        let state_bits: u64 = u64::from(raw.state) << 62;
        let avail_bits: u64 = (raw.avail as u64) << 31;
        let count_bits: u64 = raw.count as u64;

        state_bits | avail_bits | count_bits
    }
}

pub struct AtomicAnchor(atomic::AtomicU64);

impl AtomicAnchor {
    pub fn new(initial: Anchor) -> Self {
        Self(atomic::AtomicU64::new(initial.into()))
    }

    pub fn load(&self, order: atomic::Ordering) -> Anchor {
        self.0.load(order).into()
    }

    pub fn compare_exchange(
        &self,
        current: Anchor,
        new: Anchor,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<Anchor, Anchor> {
        match self
            .0
            .compare_exchange(current.into(), new.into(), success, failure)
        {
            Ok(raw) => Ok(raw.into()),
            Err(raw) => Err(raw.into()),
        }
    }
}

impl Debug for AtomicAnchor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.load(atomic::Ordering::SeqCst).fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u64_to_anchor_state() {
        assert_eq!(AnchorState::Empty, AnchorState::from(0b00));
        assert_eq!(AnchorState::Partial, AnchorState::from(0b01));
        assert_eq!(AnchorState::Full, AnchorState::from(0b10));
    }
    #[test]
    fn anchor_state_to_u64() {
        assert_eq!(0b00_u64, AnchorState::Empty.into());
        assert_eq!(0b01_u64, AnchorState::Partial.into());
        assert_eq!(0b10_u64, AnchorState::Full.into());
    }

    #[test]
    fn u64_to_anchor() {
        assert_eq!(
            Anchor {
                state: AnchorState::Empty,
                avail: 0x400000ff,
                count: 0x400000ff,
            },
            Anchor::from(0x2000007fc00000ff)
        );
        assert_eq!(
            Anchor {
                state: AnchorState::Partial,
                avail: 0x400000ff,
                count: 0x400000ff,
            },
            Anchor::from(0x6000007fc00000ff)
        );
        assert_eq!(
            Anchor {
                state: AnchorState::Full,
                avail: 0x400000ff,
                count: 0x400000ff,
            },
            Anchor::from(0xa000007fc00000ff)
        );
    }

    #[test]
    fn anchor_to_u64() {
        assert_eq!(
            0x2000007fc00000ff_u64,
            Anchor {
                state: AnchorState::Empty,
                avail: 0x400000ff,
                count: 0x400000ff,
            }
            .into(),
        );
        assert_eq!(
            0x6000007fc00000ff_u64,
            Anchor {
                state: AnchorState::Partial,
                avail: 0x400000ff,
                count: 0x400000ff,
            }
            .into(),
        );
        assert_eq!(
            0xa000007fc00000ff_u64,
            Anchor {
                state: AnchorState::Full,
                avail: 0x400000ff,
                count: 0x400000ff,
            }
            .into(),
        );
    }
}