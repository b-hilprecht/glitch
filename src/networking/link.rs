use rand::prelude::*;
use rand::{Rng, RngCore};
use tracing::info;

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use crate::{util::sample_failure_time, NodeId};

use super::{DeliverMessage, NetworkConfig};

#[derive(Debug, Clone, PartialEq)]
pub enum LinkState<M> {
    Up {
        expected_failure: Option<Instant>,
    },
    TempFailure {
        expected_recovery: Instant,
    },
    TempHold {
        expected_recovery: Instant,
        queued_messages: Vec<M>,
    },
}

pub struct Link<M> {
    state: LinkState<M>,
    config: Rc<NetworkConfig>,
    simulation_start: Instant,
    from: NodeId,
    to: NodeId,
}

impl<M: Clone + std::fmt::Debug> Link<M> {
    pub fn new(
        config: Rc<NetworkConfig>,
        simulation_start: Instant,
        now: Instant,
        from: NodeId,
        to: NodeId,
        rand: &mut dyn RngCore,
    ) -> Self {
        Link {
            state: Self::gen_up_state(now, rand, &config),
            config,
            simulation_start,
            from,
            to,
        }
    }

    pub fn gen_up_state(
        now: Instant,
        rand: &mut dyn RngCore,
        config: &NetworkConfig,
    ) -> LinkState<M> {
        let Some(mtf) = config.mean_time_between_link_failures else {
            return LinkState::Up {
                expected_failure: None,
            };
        };

        LinkState::Up {
            expected_failure: Some(sample_failure_time(now, mtf, rand)),
        }
    }

    pub fn send(
        &mut self,
        message: M,
        now: Instant,
        rand: &mut dyn RngCore,
    ) -> Vec<DeliverMessage<M>> {
        let mut released_messages = self.check_state_transition(now, rand);

        match &mut self.state {
            LinkState::Up { .. } => {
                if rand.gen_bool(self.config.duplicate_probability) {
                    released_messages.push(message.clone());
                }
                released_messages.push(message);
                released_messages
                    .into_iter()
                    .map(|m| DeliverMessage {
                        message: m,
                        delay: self.calculate_delay(rand),
                    })
                    .collect()
            }
            LinkState::TempHold {
                queued_messages, ..
            } => {
                queued_messages.push(message);
                vec![]
            }
            _ => vec![], // Messages are dropped in other states
        }
    }

    fn check_state_transition(&mut self, now: Instant, rand: &mut dyn RngCore) -> Vec<M> {
        let mut released_messages = vec![];
        let mut new_state = None;
        match &self.state {
            LinkState::Up {
                expected_failure: Some(ef),
                ..
            } => {
                if now >= *ef {
                    if rand.gen_bool(self.config.hold_probability) {
                        new_state = Some(LinkState::TempHold {
                            expected_recovery: sample_failure_time(
                                now,
                                self.config.mean_link_recovery_time,
                                rand,
                            ),
                            queued_messages: Vec::new(),
                        });
                    } else {
                        new_state = Some(LinkState::TempFailure {
                            expected_recovery: sample_failure_time(
                                now,
                                self.config.mean_link_recovery_time,
                                rand,
                            ),
                        });
                    }
                }
            }
            LinkState::TempFailure {
                expected_recovery, ..
            }
            | LinkState::TempHold {
                expected_recovery, ..
            } => {
                if now >= *expected_recovery {
                    if let LinkState::TempHold {
                        queued_messages, ..
                    } = &mut self.state
                    {
                        std::mem::swap(&mut released_messages, queued_messages);
                    }
                    new_state = Some(Self::gen_up_state(now, rand, &self.config));
                }
            }
            _ => {}
        };

        if let Some(new_state) = new_state {
            let description = match &self.state {
                LinkState::Up { .. } => "is up again",
                LinkState::TempFailure { .. } => "failed",
                LinkState::TempHold { .. } => "failed (and messages are held)",
            };
            info!(
                time = ?now.duration_since(self.simulation_start),
                from = ?self.from,
                to = ?self.to,
                "Link {}",
                description,
            );

            self.state = new_state;
        }
        released_messages
    }

    fn calculate_delay(&self, rand: &mut dyn RngCore) -> Duration {
        let mult = self.config.latency_distribution.sample(rand);
        let range =
            (self.config.max_message_latency - self.config.min_message_latency).as_millis() as f64;
        let delay = self.config.min_message_latency + Duration::from_millis((range * mult) as _);
        std::cmp::min(delay, self.config.max_message_latency)
    }
}
