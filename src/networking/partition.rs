use rand::seq::SliceRandom;
use rand::{Rng, RngCore};
use tracing::info;

use std::collections::HashSet;
use std::rc::Rc;
use std::time::Instant;

use crate::node::NodeId;
use crate::util::sample_failure_time;

use super::NetworkConfig;

#[derive(Debug)]
enum PartitionState {
    Normal {
        expected_partition: Option<Instant>,
    },
    Partition {
        partioned_nodes: HashSet<NodeId>,
        expected_recovery: Instant,
    },
}

impl PartitionState {
    fn is_partitioned(&self, from: &NodeId, to: &NodeId) -> bool {
        match self {
            PartitionState::Partition {
                partioned_nodes, ..
            } => partioned_nodes.contains(from) != partioned_nodes.contains(to),
            _ => false,
        }
    }
}

pub struct NetworkPartition {
    partition_state: PartitionState,
    nodes: Vec<NodeId>,
    config: Rc<NetworkConfig>,
    simulation_start: Instant,
}

impl NetworkPartition {
    pub fn new(
        now: Instant,
        nodes: Vec<NodeId>,
        config: Rc<NetworkConfig>,
        rand: &mut dyn RngCore,
    ) -> Self {
        let expected_partition = config
            .mean_time_between_partitions
            .map(|mtbp| sample_failure_time(now, mtbp, rand));

        NetworkPartition {
            partition_state: PartitionState::Normal { expected_partition },
            nodes,
            config,
            simulation_start: now,
        }
    }

    pub fn check_partition_state_transition(&mut self, now: Instant, rand: &mut dyn RngCore) {
        let mut new_state = None;
        match &self.partition_state {
            PartitionState::Normal {
                expected_partition: Some(ep),
            } => {
                if now >= *ep {
                    new_state = Some(PartitionState::Partition {
                        partioned_nodes: sample_random_subset(&self.nodes, 1, rand),
                        expected_recovery: sample_failure_time(
                            now,
                            self.config.mean_partition_recovery_time,
                            rand,
                        ),
                    });
                }
            }
            PartitionState::Partition {
                expected_recovery, ..
            } => {
                if now >= *expected_recovery {
                    let expected_partition = self
                        .config
                        .mean_time_between_partitions
                        .map(|mtbp| sample_failure_time(now, mtbp, rand));
                    new_state = Some(PartitionState::Normal { expected_partition });
                }
            }
            _ => {}
        };

        if let Some(new_state) = new_state {
            match &new_state {
                PartitionState::Normal { .. } => {
                    info!(
                        time = ?now.duration_since(self.simulation_start),
                        "Network partition ended"
                    );
                }
                PartitionState::Partition {
                    partioned_nodes,
                    expected_recovery: _,
                } => {
                    info!(
                        time = ?now.duration_since(self.simulation_start),
                        partitioned_nodes = ?partioned_nodes,
                        "Network partition started"
                    );
                }
            }

            self.partition_state = new_state;
        }
    }

    pub fn is_partitioned(
        &mut self,
        now: Instant,
        from: &NodeId,
        to: &NodeId,
        rand: &mut dyn RngCore,
    ) -> bool {
        self.check_partition_state_transition(now, rand);
        self.partition_state.is_partitioned(from, to)
    }
}

fn sample_random_subset(
    nodes: &[NodeId],
    min_nodes: usize,
    rng: &mut dyn RngCore,
) -> HashSet<NodeId> {
    let mut partitioned = HashSet::new();
    let node_count = rng.gen_range(min_nodes..=nodes.len());
    let mut node_indices: Vec<usize> = (min_nodes..nodes.len()).collect();
    node_indices.shuffle(rng);

    for &idx in node_indices.iter().take(node_count) {
        partitioned.insert(nodes[idx]);
    }
    partitioned
}
