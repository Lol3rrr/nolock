use std::{fmt::Debug, sync::atomic};

/// The possible States of a Node
#[derive(Debug, PartialEq)]
pub enum NodeState {
    /// The Node is either empty or currently being written to
    Empty,
    /// The Node contains a value
    Set,
    /// The Node's value has already been handled by the consumer
    Handled,
}

impl NodeState {
    /// Converts the State to its u8 representation for Storage
    pub const fn to_u8(&self) -> u8 {
        match self {
            Self::Empty => 0,
            Self::Set => 1,
            Self::Handled => 2,
        }
    }

    /// Decodes the stored Value into an actual State
    pub const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Empty),
            1 => Some(Self::Set),
            2 => Some(Self::Handled),
            _ => None,
        }
    }
}

/// A single Entry in the Queue
pub struct Node<T> {
    /// The actual Datat itself that is stored in the Node
    data: Option<T>,
    /// This holds one of three Values indicating the "State" of
    /// the Value
    is_set: atomic::AtomicU8,
}

impl<T> Node<T> {
    /// Atomically loads the current `is_set` State and decodes it
    /// as a NodeState enum
    pub fn get_state(&self) -> NodeState {
        let raw = self.is_set.load(atomic::Ordering::Acquire);
        NodeState::from_u8(raw).unwrap()
    }

    /// Stores the given Data into the Node updating its Data-Field
    /// as well as its `is_set` State to `NodeState::Set`
    pub fn store(&self, data: T) {
        unsafe {
            let raw_data: &Option<T> = &self.data;
            #[allow(mutable_transmutes)]
            let mut_data: &mut Option<T> = std::mem::transmute(raw_data);
            mut_data.replace(data);
        }
        self.is_set
            .store(NodeState::Set.to_u8(), atomic::Ordering::Release);
    }

    /// Attempts to load the Data from the Node itself, this can only be
    /// done once
    pub fn load(&self) -> T {
        let data: &Option<T> = &self.data;
        #[allow(mutable_transmutes)]
        let mut_data: &mut Option<T> = unsafe { std::mem::transmute(data) };
        mut_data.take().unwrap()
    }

    /// Sets the State of the Node to `Handled`
    pub fn handled(&self) {
        self.is_set
            .store(NodeState::Handled.to_u8(), atomic::Ordering::Release);
    }
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Self {
            data: None,
            is_set: atomic::AtomicU8::new(NodeState::Empty.to_u8()),
        }
    }
}

impl<T> Debug for Node<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Node ( is_set = {} )",
            self.is_set.load(atomic::Ordering::SeqCst)
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_store_load() {
        let node: Node<u64> = Default::default();

        node.store(15);
        assert_eq!(15, node.load());
    }

    #[test]
    fn node_state_store_state() {
        let node: Node<u64> = Default::default();

        assert_eq!(NodeState::Empty, node.get_state());
        node.store(13);
        assert_eq!(NodeState::Set, node.get_state());
    }
}
