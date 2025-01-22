use itertools::Itertools;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use tracing::{debug, info};

use std::{
    collections::BTreeMap,
    fmt::Debug,
    time::{Duration, Instant},
};

use crate::{
    node::{Node, NodeId},
    Configuration,
};

use super::{
    model::{DeterministicClient, DeterministicNode, InvariantChecker, ProtocolMessage},
    Network,
};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct EventTime {
    time: Instant,
    // trick: to handle ties in the times, use an offset. We simply
    // use the event count (number of events processed).
    offset: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Event<M: ProtocolMessage> {
    Message(SimulationMessage<M>),
    Tick,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SimulationMessage<M: ProtocolMessage> {
    message: M,
    id: usize,
}

impl<M: ProtocolMessage> SimulationMessage<M> {
    pub fn new(message: M, id: usize) -> Self {
        SimulationMessage { message, id }
    }
}

pub struct Simulator<
    N: DeterministicNode,
    C: DeterministicClient<Message = N::Message>,
    I: InvariantChecker<N, C>,
> {
    start_time: Instant,
    network: Network<N::Message>,
    nodes: Vec<Node<N>>,
    clients: Vec<C>,
    events: BTreeMap<EventTime, Event<N::Message>>,
    config: Configuration,
    rng: ChaCha8Rng,
    elapsed: Duration,
    event_processed_count: usize,
    total_event_count: usize,
    total_message_count: usize,
    invariant_checker: I,
}

impl<
        N: DeterministicNode,
        C: DeterministicClient<Message = N::Message>,
        I: InvariantChecker<N, C>,
    > Simulator<N, C, I>
{
    pub fn new(
        start_time: Instant,
        nodes: Vec<N>,
        clients: Vec<C>,
        config: Configuration,
        invariant_checker: I,
    ) -> Self {
        let mut rng = ChaCha8Rng::seed_from_u64(config.seed);

        validate_node_ids(&nodes, &clients);

        let replica_count = nodes.len();
        let wrapped_nodes: Vec<Node<N>> = nodes
            .into_iter()
            .map(|node| {
                Node::new(
                    node,
                    config.failure_config.clone(),
                    &mut rng,
                    start_time,
                    replica_count,
                )
            })
            .collect();

        let nodes = (0..wrapped_nodes.len())
            .map(NodeId::Node)
            .chain((0..clients.len()).map(NodeId::Client))
            .collect_vec();
        let network = Network::new(start_time, config.network_config.clone(), nodes, &mut rng);

        let events = BTreeMap::from_iter([(
            EventTime {
                time: start_time,
                offset: 0,
            },
            Event::Tick,
        )]);

        Simulator {
            start_time,
            network,
            nodes: wrapped_nodes,
            clients,
            events,
            config,
            rng,
            elapsed: Duration::from_secs(0),
            event_processed_count: 0,
            total_event_count: 0,
            total_message_count: 0,
            invariant_checker,
        }
    }

    pub fn run(&mut self) -> bool {
        while let Some((event_time, event)) = self.events.pop_first() {
            self.event_processed_count += 1;
            let now = event_time.time;
            self.elapsed = now.duration_since(self.start_time);

            if now.duration_since(self.start_time) > self.config.max_sim_time {
                return false;
            }

            if self.clients.iter().all(|client| client.finished()) {
                self.check_invariants();
                return true;
            }

            let messages = self.handle_event(now, event);
            if self.event_processed_count % self.config.check_invariants_frequency == 0 {
                self.check_invariants();
            }

            for msg in messages {
                self.total_message_count += 1;
                let message_id = self.total_message_count;
                debug!(
                    time = ?now.duration_since(self.start_time),
                    from = ?msg.source(),
                    to = ?msg.destination(),
                    msg = ?msg,
                    message_id = message_id,
                    "Sending message"
                );

                let delivered_msgs = self.network.send(msg, now, &mut self.rng);
                for del_msg in delivered_msgs {
                    self.push_event(
                        now + del_msg.delay,
                        Event::Message(SimulationMessage::new(del_msg.message, message_id)),
                    );
                }
            }
        }
        false
    }

    fn push_event(&mut self, time: Instant, event: Event<N::Message>) {
        self.total_event_count += 1;
        self.events.insert(
            EventTime {
                time,
                offset: self.total_event_count,
            },
            event,
        );
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    fn can_additional_node_fail(&self) -> bool {
        let max_failures = self.nodes.len() / 2;
        let currently_failed = self.nodes.iter().filter(|n| !n.is_up()).count();
        currently_failed < max_failures
    }

    fn handle_event(&mut self, now: Instant, event: Event<N::Message>) -> Vec<N::Message> {
        match event {
            Event::Message(msg) => {
                let SimulationMessage {
                    message: msg,
                    id: message_id,
                } = msg;
                debug!(
                    time = ?now.duration_since(self.start_time),
                    from = ?msg.source(),
                    to = ?msg.destination(),
                    msg = ?msg,
                    message_id = message_id,
                    "Received message"
                );

                match msg.destination() {
                    NodeId::Node(node_id) => {
                        let can_fail = self.can_additional_node_fail();
                        self.nodes[node_id].process_message(msg, now, can_fail, &mut self.rng)
                    }
                    NodeId::Client(client_id) => self.clients[client_id].process_message(msg, now),
                }
            }
            Event::Tick => {
                let mut messages = Vec::new();

                info!(
                    time = ?now.duration_since(self.start_time),
                    "Executing tick"
                );

                // Handle node ticks
                for node in &mut self.nodes {
                    messages.extend(node.tick(now, &mut self.rng));
                }

                // Handle client ticks
                for client in &mut self.clients {
                    messages.extend(client.tick(now));
                }

                self.push_event(now + self.config.tick_interval, Event::Tick);

                messages
            }
        }
    }

    fn check_invariants(&self) {
        self.invariant_checker
            .check_invariants(self.config.seed, &self.nodes, &self.clients);
    }
}

fn validate_node_ids<N: DeterministicNode, C: DeterministicClient>(nodes: &[N], clients: &[C]) {
    // Validate node IDs are sequential from 0 to n
    let node_ids: Vec<NodeId> = nodes.iter().map(|n| n.id()).collect();
    assert_eq!(
        node_ids,
        (0..node_ids.len()).map(NodeId::Node).collect_vec(),
        "Node IDs must be sequential starting from 0"
    );

    // Validate client IDs are sequential from 0 to n
    let client_ids: Vec<NodeId> = clients.iter().map(|c| c.id()).collect();
    assert_eq!(
        client_ids,
        (0..client_ids.len()).map(NodeId::Client).collect_vec(),
        "Client IDs must be sequential starting from 0"
    );
}
