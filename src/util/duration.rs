use priv_prelude::*;

pub trait DurationExt {
    fn mul_f64(self, m: f64) -> Duration;
    fn div_to_f64(self, other: Duration) -> f64;
}

impl DurationExt for Duration {
    fn mul_f64(self, m: f64) -> Duration {
        let nanos = self.subsec_nanos() as f64 * m;
        let secs = self.as_secs() as f64 * m;

        let subsec_nanos = (nanos % 1e9f64 + secs.fract() * 1e9f64) as u32;
        let secs = (nanos / 1e9f64 + secs.floor()) as u64;

        Duration::new(secs, subsec_nanos)
    }

    fn div_to_f64(self, other: Duration) -> f64 {
        let nanos_a = self.subsec_nanos() as f64 + self.as_secs() as f64 * 1e9f64;
        let nanos_b = other.subsec_nanos() as f64 + other.as_secs() as f64 * 1e9f64;
        nanos_a / nanos_b
    }
}

