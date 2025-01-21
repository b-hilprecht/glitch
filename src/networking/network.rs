use rand::RngCore;

use std::cmp;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::{NodeId, ProtocolMessage};

use super::link::Link;
use super::partition::NetworkPartition;
use super::NetworkConfig;

#[derive(Debug, Clone)]
pub struct DeliverMessage<M> {
    pub message: M,
    pub delay: Duration,
}

pub struct Network<M> {
    links: HashMap<(NodeId, NodeId), Link<M>>,
    partitioning: NetworkPartition,
    config: Rc<NetworkConfig>,
    simulation_start: Instant,
}

impl<M> Network<M>
where
    M: Clone + std::fmt::Debug + ProtocolMessage,
{
    pub fn new(
        simulation_start: Instant,
        config: NetworkConfig,
        nodes: Vec<NodeId>,
        rand: &mut dyn RngCore,
    ) -> Self {
        let shared_config = Rc::new(config);
        Network {
            links: HashMap::new(),
            config: shared_config.clone(),
            partitioning: NetworkPartition::new(simulation_start, nodes, shared_config, rand),
            simulation_start,
        }
    }

    pub fn send(
        &mut self,
        message: M,
        now: Instant,
        rand: &mut dyn RngCore,
    ) -> Vec<DeliverMessage<M>> {
        let from = message.source();
        let to = message.destination();

        if self.partitioning.is_partitioned(now, &from, &to, rand) {
            return vec![];
        }

        let bidirectional = match from.cmp(&to) {
            cmp::Ordering::Less => (from, to),
            cmp::Ordering::Greater => (to, from),
            cmp::Ordering::Equal => (from, to),
        };

        self.links
            .entry(bidirectional)
            .or_insert_with(|| {
                Link::new(
                    self.config.clone(),
                    self.simulation_start,
                    now,
                    from,
                    to,
                    rand,
                )
            })
            .send(message, now, rand)
    }
}
