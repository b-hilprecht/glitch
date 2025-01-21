use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use std::time::Instant;
use tracing::info;

use derive_more::derive::IsVariant;

use crate::{util::sample_failure_time, DeterministicNode, FailureConfiguration, NodeId};

#[derive(Debug)]
pub struct Node<N: DeterministicNode> {
    node: N,
    state: NodeState,
    failure_config: FailureConfiguration,
    replica_count: usize,
    start_time: Instant,
}

#[derive(Debug, IsVariant)]
enum NodeState {
    Normal { failure_time: Option<Instant> },
    Failed { recovery_time: Instant },
}

impl<N: DeterministicNode> Node<N> {
    pub fn new(
        node: N,
        failure_config: FailureConfiguration,
        rng: &mut ChaCha8Rng,
        start_time: Instant,
        replica_count: usize,
    ) -> Self {
        let failure_time =
            failure_config
                .mean_time_between_failures
                .map(|mean_time_between_failures| {
                    sample_failure_time(start_time, mean_time_between_failures, rng)
                });

        Node {
            node,
            state: NodeState::Normal { failure_time },
            failure_config,
            replica_count,
            start_time,
        }
    }

    pub fn node(&self) -> &N {
        &self.node
    }

    pub fn id(&self) -> NodeId {
        self.node.id()
    }

    pub fn is_up(&self) -> bool {
        !(self.state.is_failed() || self.node.is_recovering())
    }

    fn has_failed(&mut self, now: Instant, can_fail: bool, rand: &mut dyn RngCore) -> bool {
        let mut new_state = None;
        match &self.state {
            NodeState::Normal { failure_time } => {
                if let Some(failure_time) = failure_time {
                    if now >= *failure_time && can_fail {
                        new_state = Some(NodeState::Failed {
                            recovery_time: sample_failure_time(
                                now,
                                self.failure_config.mean_time_between_failures.unwrap(),
                                rand,
                            ),
                        });
                    }
                }
            }
            NodeState::Failed { recovery_time } => {
                if now >= *recovery_time {
                    new_state = Some(NodeState::Normal {
                        failure_time: self.failure_config.mean_time_between_failures.map(
                            |mean_time_between_failures| {
                                sample_failure_time(now, mean_time_between_failures, rand)
                            },
                        ),
                    });
                }
            }
        };

        if let Some(new_state) = new_state {
            if new_state.is_normal() {
                info!(
                    time = ?now.duration_since(self.start_time),
                    node = ?self.id(),
                    "Node restarted"
                );
                let nonce = rand.next_u64();
                self.node.recover(now, nonce, self.replica_count);
            } else {
                info!(
                    time = ?now.duration_since(self.start_time),
                    node = ?self.id(),
                    "Node crashed"
                );
            }

            self.state = new_state;
        }

        self.state.is_failed()
    }

    pub fn tick(&mut self, now: Instant, rand: &mut dyn RngCore) -> Vec<N::Message> {
        if self.has_failed(now, false, rand) {
            return vec![];
        }
        self.node.tick(now)
    }

    pub fn process_message(
        &mut self,
        msg: N::Message,
        now: Instant,
        can_fail: bool,
        rand: &mut dyn RngCore,
    ) -> Vec<N::Message> {
        if self.has_failed(now, can_fail, rand) {
            return vec![];
        }
        self.node.process_message(msg, now)
    }
}
