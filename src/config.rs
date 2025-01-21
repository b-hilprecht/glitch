use std::time::Duration;

use crate::NetworkConfig;

#[derive(Debug, Clone)]
pub struct Configuration {
    pub tick_interval: Duration,
    pub max_sim_time: Duration,
    pub seed: u64,
    pub check_invariants_frequency: usize,
    pub network_config: NetworkConfig,
    pub failure_config: FailureConfiguration,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            tick_interval: Duration::from_millis(50),
            max_sim_time: Duration::from_secs(10),
            seed: 1,
            check_invariants_frequency: 1,
            network_config: NetworkConfig::default(),
            failure_config: FailureConfiguration::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FailureConfiguration {
    pub mean_time_between_failures: Option<Duration>,
    pub mean_time_to_recover: Duration,
}

impl Default for FailureConfiguration {
    fn default() -> Self {
        FailureConfiguration {
            mean_time_between_failures: Some(Duration::from_millis(3000)),
            mean_time_to_recover: Duration::from_millis(2000),
        }
    }
}
