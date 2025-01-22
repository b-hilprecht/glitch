use std::{fmt::Debug, time::Instant};

use crate::{node::NodeId, Node};

pub trait ProtocolMessage: Clone + Debug + Eq + PartialEq {
    /// Returns the source of the message.
    fn source(&self) -> NodeId;

    /// Returns the destination of the message.
    fn destination(&self) -> NodeId;
}

pub trait DeterministicNode: Debug {
    type Message: ProtocolMessage;

    /// Returns the ID of the node.
    fn id(&self) -> NodeId;

    /// Performs periodic work and returns messages to be sent. Time is injected
    /// to allow for deterministic behavior.
    fn tick(&mut self, now: Instant) -> Vec<Self::Message>;

    /// Processes a message and returns messages to be sent. Time is injected to
    /// allow for deterministic behavior.
    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message>;

    /// Initiates recovery of the node. Must not be implemented if mean time to
    /// node failure is None in the configuration (i.e., nodes will not fail)
    fn recover(&mut self, now: Instant, nonce: u64, replica_count: usize);

    /// Returns whether the node is currently recovering (the node is not yet
    /// ready to process messages).
    fn is_recovering(&self) -> bool;
}

pub trait DeterministicClient: Debug {
    type Message: ProtocolMessage;

    /// Returns the ID of the client.
    fn id(&self) -> NodeId;

    /// Performs periodic work and returns messages to be sent. Time is injected to
    /// allow for deterministic behavior.
    fn tick(&mut self, now: Instant) -> Vec<Self::Message>;

    /// Processes a message and returns messages to be sent. Time is injected to
    /// allow for deterministic behavior.
    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message>;

    /// Returns whether the client has finished its work, e.g., all requests have
    /// been processed.
    fn finished(&self) -> bool;
}

pub trait InvariantChecker<N: DeterministicNode, C: DeterministicClient<Message = N::Message>> {
    /// Checks invariants of the system given the current state, e.g., an
    /// acknowledged message must be replicated to a majority of nodes in
    /// a consensus protocol. The invariants are checked periodically during
    /// simulation.
    fn check_invariants(&self, seed: u64, nodes: &[Node<N>], clients: &[C]);
}
