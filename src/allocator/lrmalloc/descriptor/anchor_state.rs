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
}
