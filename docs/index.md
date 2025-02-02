# Glitch: A Deterministic Simulation Testing Library for Distributed Protocols

In distributed systems, the hardest bugs to find aren't the ones that crash your system immediately - they're the ones that lurk silently for months before corrupting your data. These bugs often emerge from the complex interplay of network failures, node crashes, and message delays that are nearly impossible to reproduce in traditional testing environments.

This is the challenge I faced when implementing the [Viewstamped Replication (VSR) protocol](https://pmg.csail.mit.edu/papers/vr-revisited.pdf), a consensus protocol that ensures multiple servers maintain consistent copies of data even when things go wrong. Consensus protocols are the backbone of distributed databases and other systems that need to stay reliable even when individual nodes fail. While protocols like Paxos and Raft are more widely known, VSR offers some unique advantages that caught my attention through [Marc Brooker's blog post](https://brooker.co.za/blog/2014/05/19/vr.html) and an analysis with TLA+ on [Jack Vanlightly's blog](https://jack-vanlightly.com/analyses/2022/12/20/vr-revisited-an-analysis-with-tlaplus).

## The Challenge: Testing Distributed Systems Under Failure

Implementing consensus protocols such as VSR correctly is notoriously difficult. A system might work perfectly in testing and run smoothly for months until that one-in-a-million edge case occurs: a network partition happens exactly when a leader is being elected, or a node crashes precisely between receiving and acknowledging a message. Traditional testing approaches fall short because these scenarios are extremely timing-sensitive and hard to reproduce.

## The Promise of Deterministic Testing

This is where deterministic testing shines. Instead of hoping to catch issues in production, we can simulate our distributed system in an environment where we control every aspect of execution:

- Network delays
- Message delivery order
- Node failures and recoveries
- Network partitions

and see if our system behaves as expected. Given enough executions under failure we can be more confident that our implementation is correct.

[FoundationDB](https://apple.github.io/foundationdb/testing.html) first popularized this approach by systematically testing their distributed database this way. The key insight? By making all "random" events deterministic, we can reproduce any bug by simply replaying the same sequence of events that triggered it.

This inspired me to test my VSR implementation using deterministic testing. I first wanted to use a framework such as [turmoil](https://github.com/tokio-rs/turmoil) but I found that I wanted something else for my use case. Specifically, I wanted to introduce all failure conditions automatically (without having to think about edge cases due to network partitions and failures) and check continuously for invariants (similar to [TLA+](https://learntla.com/index.html) which is a formal verification tool).

## Glitch under the Hood

### The Core Interface

At its heart, Glitch requires distributed protocols to implement a simple interface with two core methods:

```rust
impl DeterministicNode for Node {
    type Message = ProtocolMessage;

    fn process_message(&mut self, msg: Self::Message, now: Instant) -> Vec<Self::Message> {
        // process message and return messages to send
    }

    fn tick(&mut self, now: Instant) -> Vec<Self::Message> {
        // periodic tasks, e.g., sending heartbeats
    }
    // ...
}
```

A `DeterministicNode` represents any participant in your distributed system - whether it's a primary node or backup.

The first method just receives some message from another node or itself, changes some internal state and might require to send messages to other nodes. For instance, if a request in VSR is sent to the primary node, it will send prepare messages to backup nodes. The time is also injected as a dependency to ensure that the execution is deterministic. However, in distributed protocols it’s also important to periodically send heartbeats and have other recurring tasks. This is the purpose of the tick method which just receives the current time and produces messages such as heartbeats etc.

### The Simulation

Behind the scenes, Glitch manages the complexity of distributed execution using a priority queue of events which are either delivered messages or ticks: Every such event has a timestamp for when it should occur and the simulation clock advances discretely from event to event. No real time passes - we control the entire timeline.

We can implement such a simulation using a simple priority queue of events. In a loop, we poll the next event (at time `t`) from the priority queue and either call `tick` on all nodes or process a message on a node. We then add the resulting messages to the priority queue at time `t+d` (some delay) and repeat. Meanwhile, we also check invariants and whether clients have finished to see if we are done.

### Introducing Failures

In this basic model, network failures are easy to introduce: every time we want to send a message on a link, we check if the link has failed (we simply sample a failure time using the mean time to failure for links and partitions) and then if it has not failed sample a delay for the message. Similarly before we let a node process a message or tick event, we can check if that node has failed or recovered in the meantime (also by sampling failure / recovery times using mean time to failure durations).

### Configuring Chaos

In glitch, all of this is configured using a configuration struct where only the mean time to failure is required for links, partitions and nodes. This allows to easily simulate edge cases due to partitions and node crashes without having to think about how to introduce them.

```rust
let config = Configuration {
    tick_interval: Duration::from_millis(50),
    max_sim_time: Duration::from_secs(30),
    network_config: NetworkConfig {
        // Message delivery configuration
        min_message_latency: Duration::from_millis(0),
        max_message_latency: Duration::from_millis(100),
        duplicate_probability: 0.1,  // Chance of duplicate message delivery

        // Network failure parameters
        mean_time_between_link_failures: Some(Duration::from_millis(1000)), // On average, links fail every second
        mean_link_recovery_time: Duration::from_millis(300),
        mean_time_between_partition_failures: Some(Duration::from_millis(1000)),
        mean_partition_recovery_time: Duration::from_millis(300),
    },
    failure_config: FailureConfig {
        mean_time_between_node_failures: Some(Duration::from_millis(1000)),
        mean_node_recovery_time: Duration::from_millis(300),
    }
};
```

### Invariants and Liveness

Eventually, the goal of simulation testing is to catch bugs early. Bugs in distributed protocols can mean two things: the system ends up in an inconsistent state (e.g., data is lost or not on a majority of nodes, log ordering is incorrect etc.) or does not finish (e.g., deadlocks, protocol states we cannot recover from etc.). These properties are called invariants (safety, bad stuff should not happen) and liveness (good stuff eventually happens) in formal verification, e.g., TLA+. We also draw inspiration from TLA+ in glitch. Namely, we let the user define invariants which are checked periodically and a finish condition (e.g., all client requests are processed). In the simulation we can then check that the invariants hold all the time. This is really helpful for debugging since we can see exactly when the invariant is violated way before this results in a wrong behavior of the system. For instance, if we define an invariant that acknowledged requests are always replicated on a majority of nodes, we can see exactly when this invariant is violated even before we have a potential data loss (where the first step in the investigation is to find out when the invariant was violated).

Here is an example invariant for VSR I actually use in my [implementation](https://github.com/b-hilprecht/viewstamped-replication-rs/blob/main/vsr/src/tests/invariants.rs). It checks that every operation in the log up to commit number is the same as in the longest log (i.e., operations are correctly replicated).

```rust
// check that everything up to commit number is same as in longest log
for (i, entry) in max_log.iter().enumerate() {
    if entry.op_number > node.node().commit_number() {
        break;
    }

    assert_eq!(
        entry,
        &log[i],
        "Log mismatch at index {} for node {:?} (seed: {})",
        i,
        node.id(),
        seed
    );
}
```

### Usage

Running a simulation is then as simple as:

```rust
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

For a full example, see the [VSR simulation tests](https://github.com/b-hilprecht/viewstamped-replication-rs/blob/main/vsr/src/tests/simulation_tests.rs).

## Comparison with other Tools

Glitch and turmoil have slightly different goals. Glitch is focused on testing distributed protocols (e.g., VSR) while turmoil is more focused on testing entire distributed systems (e.g., databases). This makes it easier for glitch to automatically introduce node failures and check for invariants which need additional coding in turmoil. Also, the interface is easier to use for a protocol implementation.

TLA+ and Glitch are complementary tools: TLA+ is a formal verification tool and can be used to verify protocol designs. Glitch can also be used to find problems in the protocol design but also in the actual implementation of the protocol.

## Conclusion

Glitch aims to be a simple tool for testing distributed protocols. It is inspired by TLA+ and turmoil but is focused on testing protocols. Using Glitch to test my VSR implementation helped me find several subtle edge cases that would have been nearly impossible to catch with traditional testing approaches.

The source code is available on [GitHub](https://github.com/b-hilprecht/glitch), and I welcome feedback and contributions.
