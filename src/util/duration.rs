use priv_prelude::*;

pub trait DurationExt {
    fn mul_f64(self, m: f64) -> Duration;
    fn div_to_f64(self, other: Duration) -> f64;
}

impl DurationExt for Duration {
    fn mul_f64(self, m: f64) -> Duration {
        let nanos = f64::from(self.subsec_nanos()) * m;
        let secs = f64::from(self.as_secs() as u32) * m;

        let subsec_nanos = (nanos % 1e9f64 + secs.fract() * 1e9f64) as u32;
        let secs = (nanos / 1e9f64 + secs.floor()) as u64;

        Duration::new(secs, subsec_nanos)
    }

    fn div_to_f64(self, other: Duration) -> f64 {
        let nanos_a = f64::from(self.subsec_nanos()) + self.as_secs() as f64 * 1e9f64;
        let nanos_b = f64::from(other.subsec_nanos() as u32) + other.as_secs() as f64 * 1e9f64;
        nanos_a / nanos_b
    }
}

