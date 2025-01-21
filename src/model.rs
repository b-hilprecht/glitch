use std::{fmt, fmt::Debug, time::Instant};

use crate::Node;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum NodeId {
    Node(usize),
    Client(usize),
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeId::Node(id) => write!(f, "Node({})", id),
            NodeId::Client(id) => write!(f, "Client({})", id),
        }
    }
}

pub trait ProtocolMessage: Clone + Debug + Eq + PartialEq {
    fn source(&self) -> NodeId;
    fn destination(&self) -> NodeId;
}

pub trait DeterministicNode: Debug {
    type Message: ProtocolMessage;

    fn id(&self) -> NodeId;
    fn tick(&mut self, now: Instant) -> Vec<Self::Message>;
    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message>;
    fn recover(&mut self, now: Instant, nonce: u64, replica_count: usize);
    fn is_recovering(&self) -> bool;
}

pub trait DeterministicClient: Debug {
    type Message: ProtocolMessage;

    fn id(&self) -> NodeId;
    fn tick(&mut self, now: Instant) -> Vec<Self::Message>;
    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message>;
    fn finished(&self) -> bool;
}

pub trait InvariantChecker<N: DeterministicNode, C: DeterministicClient<Message = N::Message>> {
    fn check_invariants(&self, seed: u64, nodes: &[Node<N>], clients: &[C]);
}
