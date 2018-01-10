use priv_prelude::*;

pub trait DurationExt {
    fn mul_f32(self, m: f32) -> Duration;
}

impl DurationExt for Duration {
    fn mul_f32(self, m: f32) -> Duration {
        let secs = (self.as_secs() as f64) + (self.subsec_nanos() as f64 * 1e-9);
        let secs = secs * (m as f64);
        let whole_secs = secs.floor() as u64;
        let subsec_nanos = (secs.fract() * 1e9).floor() as u32;
        Duration::new(whole_secs, subsec_nanos)
    }
}

