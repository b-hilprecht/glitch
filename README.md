# Glitch

Glitch is a framework for testing distributed system protocols under various network and node failure conditions. It provides a deterministic simulation environment where you can verify protocol correctness under adverse conditions.

## Features

- Deterministic simulation with reproducible results making it easy to debug and reproduce issues
- Automatically simulates the following hardships (with configurable parameters):
  - Message delays and duplicates
  - Network partitions
  - Link failures
  - Message duplication
  - Node failures and recovery (never more than a quorum of nodes failing at a time)
- Allows to define custom invariants (similar to TLA+) to verify protocol correctness during simulation
- Simply implement a tracing subscriber to get detailed logs of the simulation

## Usage

To test your protocol, implement these key traits:

```rust
// Define your protocol messages
#[derive(Debug, Clone, Eq, PartialEq)]
enum EchoMessage {
    Request { id: u64, data: String },
    Response { id: u64, data: String },
}

impl ProtocolMessage for EchoMessage {
    fn source(&self) -> NodeId { ... }
    fn destination(&self) -> NodeId { ... }
}

// Implement your node behavior
impl DeterministicNode for EchoServer {
    type Message = EchoMessage;

    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message> {
        match msg {
            EchoMessage::Request { id, data } => {
                vec![EchoMessage::Response { id, data }]
            }
            _ => vec![],
        }
    }
    // ... other required methods
}

// Define invariants to check
impl InvariantChecker<EchoServer, EchoClient> for EchoInvariantChecker {
    fn check_invariants(&self, seed: u64, nodes: &[Node<EchoServer>], clients: &[EchoClient]) {
        // Verify protocol correctness properties
    }
}
```

## Running Tests

```rust
let config = Configuration {
    tick_interval: Duration::from_millis(50),
    max_sim_time: Duration::from_secs(30),
    network_config: NetworkConfig {
        min_message_latency: Duration::from_millis(0),
        max_message_latency: Duration::from_millis(100),
        duplicate_probability: 0.1,
        mean_time_between_link_failures: Some(Duration::from_millis(1000)),
        mean_link_recovery_time: Duration::from_millis(300),
        ..Default::default()
    },
    ..Default::default()
};

let mut simulator = Simulator::new(
    start_time,
    vec![server],
    vec![client],
    config,
    checker
);

// tracing for debugging
FmtSubscriber::builder()
    .with_max_level(Level::DEBUG)
    .pretty()
    .init();

assert!(simulator.run());
```

## Failure Modes

- **Network Failures**:

  - Message loss: Messages can be dropped during link failures
  - Delays: Configurable message latency based on provided distribution
  - Duplicates: Messages may be duplicated with configurable probability
  - Partitions: Network can split into disconnected components

- **Node Failures**:
  - Crash-recovery: Nodes can crash and recover with configurable frequency
  - Recovery state: Nodes must handle recovery and maintain consistency
