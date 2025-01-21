use std::time::{Duration, Instant};

use rand::RngCore;
use rand_distr::{Distribution, Exp};

pub(crate) fn sample_failure_time(
    start_time: Instant,
    mtf: Duration,
    rand: &mut dyn RngCore,
) -> Instant {
    let mult = Exp::new(1.0 / mtf.as_secs_f64()).unwrap().sample(rand);

    start_time + Duration::from_secs_f64(mult)
}
