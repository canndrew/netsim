use priv_prelude::*;

use rand;
use rand::distributions::range::Range;

pub mod bytes_mut;
pub mod ipv4_addr;
pub mod ipv6_addr;
pub mod duration;

pub fn expovariate_rand() -> f64 {
    let range = Range::new(0.0, 1.0);
    let mut rng = rand::thread_rng();
    let offset = range.ind_sample(&mut rng);
    -f64::ln(1.0 - offset)
}

