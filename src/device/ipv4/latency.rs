use priv_prelude::*;

/// Adds latency to an IPv4 connection
pub struct LatencyV4 {
    //inner: Latency<Ipv4Packet>,
}

impl LatencyV4 {
    /// Connect the two given plugs with latency added to the connection.
    ///
    /// `min_latency` is the baseline for the amount of delay added to packets travelling along
    /// this connection. `mean_additional_latency` controls the amount of random, additional
    /// latency added to any given packet. A non-zero `mean_additional_latency` can cause packets
    /// to be re-ordered.
    pub fn spawn(
        handle: &Handle,
        min_latency: Duration,
        mean_additional_latency: Duration,
        plug_a: Ipv4Plug,
        plug_b: Ipv4Plug,
    ) {

        Latency::spawn(
            handle,
            min_latency,
            mean_additional_latency,
            plug_a.into(),
            plug_b.into(),
        )
    }
}

