use std::time::Duration;

use rand_distr::Exp;

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    // Latency configuration
    pub min_message_latency: Duration,
    pub max_message_latency: Duration,
    pub latency_distribution: Exp<f64>,

    // Duplicate configuration
    pub duplicate_probability: f64,

    // Failure configuration
    pub hold_probability: f64,
    pub mean_time_between_link_failures: Option<Duration>,
    pub mean_link_recovery_time: Duration,

    // Partition configuration
    pub mean_time_between_partitions: Option<Duration>,
    pub mean_partition_recovery_time: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        NetworkConfig {
            min_message_latency: Duration::from_millis(0),
            max_message_latency: Duration::from_millis(100),
            latency_distribution: Exp::new(5.0).unwrap(),
            duplicate_probability: 0.1,
            mean_time_between_link_failures: Some(Duration::from_millis(1000)),
            mean_link_recovery_time: Duration::from_millis(300),
            hold_probability: 0.3, // 30% chance of temporary failures hold and then recover
            mean_time_between_partitions: Some(Duration::from_millis(4000)),
            mean_partition_recovery_time: Duration::from_millis(1000),
        }
    }
}
