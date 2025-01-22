#[cfg(test)]
mod tests {
    use tracing::Level;
    use tracing_subscriber::FmtSubscriber;

    use crate::{
        node::NodeId, Configuration, DeterministicClient, DeterministicNode, InvariantChecker,
        NetworkConfig, Node, ProtocolMessage, Simulator,
    };
    use std::{
        collections::HashSet,
        time::{Duration, Instant},
    };

    #[derive(Debug, Clone, Eq, PartialEq)]
    enum EchoMessage {
        Request { id: u64, data: String },
        Response { id: u64, data: String },
    }

    impl ProtocolMessage for EchoMessage {
        fn source(&self) -> NodeId {
            match self {
                EchoMessage::Request { .. } => NodeId::Client(0),
                EchoMessage::Response { .. } => NodeId::Node(0),
            }
        }

        fn destination(&self) -> NodeId {
            match self {
                EchoMessage::Request { .. } => NodeId::Node(0),
                EchoMessage::Response { .. } => NodeId::Client(0),
            }
        }
    }

    #[derive(Debug)]
    struct EchoServer {
        id: NodeId,
        replied_requests: HashSet<u64>,
    }

    impl DeterministicNode for EchoServer {
        type Message = EchoMessage;

        fn id(&self) -> NodeId {
            self.id
        }

        fn tick(&mut self, _now: Instant) -> Vec<Self::Message> {
            vec![]
        }

        fn process_message(&mut self, msg: Self::Message, _now: Instant) -> Vec<Self::Message> {
            match msg {
                EchoMessage::Request { id, data } => {
                    self.replied_requests.insert(id);
                    vec![EchoMessage::Response { id, data }]
                }
                _ => vec![],
            }
        }

        fn recover(&mut self, _now: Instant, _nonce: u64, _replica_count: usize) {}

        fn is_recovering(&self) -> bool {
            false
        }
    }

    #[derive(Debug)]
    struct EchoClient {
        id: NodeId,
        current_request: u64,
        total_requests: u64,
        completed_requests: HashSet<u64>,
        last_request_time: Option<Instant>,
        retry_interval: Duration,
        with_retries: bool,
    }

    impl EchoClient {
        fn new(total_requests: u64, retry_interval: Duration, with_retries: bool) -> Self {
            EchoClient {
                id: NodeId::Client(0),
                current_request: 0,
                total_requests,
                completed_requests: HashSet::new(),
                last_request_time: None,
                retry_interval,
                with_retries,
            }
        }
    }

    impl DeterministicClient for EchoClient {
        type Message = EchoMessage;

        fn id(&self) -> NodeId {
            self.id
        }

        fn tick(&mut self, now: Instant) -> Vec<Self::Message> {
            let mut messages = Vec::new();

            // Send next request
            if (self.completed_requests.contains(&self.current_request)
                || (self.current_request == 0 && self.last_request_time.is_none()))
                && self.current_request <= self.total_requests
            {
                self.current_request += 1;
                self.last_request_time = Some(now);
                messages.push(EchoMessage::Request {
                    id: self.current_request,
                    data: format!("echo_{}", self.current_request),
                });
            }

            // Handle retries if enabled
            if self.with_retries {
                if let Some(last_time) = self.last_request_time {
                    if now.duration_since(last_time) >= self.retry_interval {
                        messages.push(EchoMessage::Request {
                            id: self.current_request,
                            data: format!("echo_{}", self.current_request),
                        });

                        self.last_request_time = Some(now);
                    }
                }
            }

            messages
        }

        fn process_message(&mut self, msg: Self::Message, _now: Instant) -> Vec<Self::Message> {
            if let EchoMessage::Response { id, .. } = msg {
                self.completed_requests.insert(id);
            }
            vec![]
        }

        fn finished(&self) -> bool {
            self.completed_requests.len() as u64 == self.total_requests
        }
    }

    #[derive(Debug)]
    struct EchoInvariantChecker;

    impl InvariantChecker<EchoServer, EchoClient> for EchoInvariantChecker {
        fn check_invariants(&self, seed: u64, nodes: &[Node<EchoServer>], clients: &[EchoClient]) {
            let client = &clients[0];
            let server = &nodes[0];

            for request_id in &client.completed_requests {
                // every request that the client sees as completed should have been replied by the server
                assert!(
                    server.node().replied_requests.contains(request_id),
                    "Request {} was not replied by the server (seed: {})",
                    request_id,
                    seed
                );

                // and current request should be greater or equal to the completed request
                assert!(
                    client.current_request >= *request_id,
                    "Current request {} is less than the completed request {} (seed: {})",
                    client.current_request,
                    request_id,
                    seed
                );
            }
        }
    }

    fn test_echo_protocol(network_config: NetworkConfig, total_requests: u64, with_retries: bool) {
        let start_time = Instant::now();

        let server = EchoServer {
            id: NodeId::Node(0),
            replied_requests: HashSet::new(),
        };

        let client = EchoClient::new(total_requests, Duration::from_millis(200), with_retries);

        let config = Configuration {
            tick_interval: Duration::from_millis(50),
            max_sim_time: Duration::from_secs(30),
            seed: 1,
            check_invariants_frequency: 1,
            network_config,
            ..Configuration::default()
        };

        let checker = EchoInvariantChecker {};

        let mut simulator = Simulator::new(start_time, vec![server], vec![client], config, checker);

        assert!(simulator.run());
    }

    #[test]
    fn test_reliable_network() {
        FmtSubscriber::builder()
            .with_max_level(Level::DEBUG)
            .pretty()
            .init();

        let config = NetworkConfig {
            mean_time_between_link_failures: None,
            mean_time_between_partitions: None,
            ..NetworkConfig::default()
        };
        test_echo_protocol(config, 10, false);
    }

    #[test]
    #[should_panic]
    fn test_unreliable_network_without_retries() {
        let config = NetworkConfig::default();
        test_echo_protocol(config, 10, false);
    }

    #[test]
    fn test_unreliable_network_with_retries() {
        let config = NetworkConfig::default();
        test_echo_protocol(config, 10, true);
    }
}
