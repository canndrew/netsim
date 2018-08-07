use priv_prelude::*;
use future_utils;

mod latency;
mod packet_loss;

pub use self::latency::*;
pub use self::packet_loss::*;

#[derive(Debug)]
/// Bidirectional network plug that can be used to exchange data between two devices.
/// Anything written to the plub will be readable on the other side.
pub struct Plug<T: fmt::Debug + 'static> {
    /// The sender
    pub tx: UnboundedSender<T>,
    /// The receiver.
    pub rx: UnboundedReceiver<T>,
}

impl<T: fmt::Debug + Send + 'static> Plug<T> {
    /// Create a new connection connecting the two returned plugs.
    pub fn new_pair() -> (Plug<T>, Plug<T>) {
        let (a_tx, b_rx) = future_utils::mpsc::unbounded();
        let (b_tx, a_rx) = future_utils::mpsc::unbounded();
        let a = Plug {
            tx: a_tx,
            rx: a_rx,
        };
        let b = Plug {
            tx: b_tx,
            rx: b_rx,
        };
        (a, b)
    }

    /// Add latency to the end of this connection.
    ///
    /// `min_latency` is the baseline for the amount of delay added to a packet travelling on this
    /// connection. `mean_additional_latency` controls the amount of extra, random latency added to
    /// any given packet on this connection. A non-zero `mean_additional_latency` can cause packets
    /// to be re-ordered.
    pub fn with_latency(
        self,
        handle: &NetworkHandle,
        min_latency: Duration,
        mean_additional_latency: Duration,
    ) -> Plug<T> {
        let (plug_0, plug_1) = Plug::new_pair();
        Latency::spawn(handle, min_latency, mean_additional_latency, self, plug_0);
        plug_1
    }

    /// Add packet loss to the connection. Loss happens in burst, rather than on an individual
    /// packet basis. `mean_loss_duration` controls the burstiness of the loss.
    pub fn with_packet_loss(
        self,
        handle: &NetworkHandle,
        loss_rate: f64,
        mean_loss_duration: Duration,
    ) -> Plug<T> {
        let (plug_0, plug_1) = Plug::new_pair();
        PacketLoss::spawn(handle, loss_rate, mean_loss_duration, self, plug_0);
        plug_1
    }

    /// Returns sender and receiver handles used to interact with the other side of the plug.
    pub fn split(self) -> (UnboundedSender<T>, UnboundedReceiver<T>) {
        (self.tx, self.rx)
    }
}

impl<T: fmt::Debug + 'static> Stream for Plug<T> {
    type Item = T;
    type Error = Void;

    fn poll(&mut self) -> Result<Async<Option<T>>, Void> {
        self.rx.poll()
    }
}

impl<T: fmt::Debug + 'static> Sink for Plug<T> {
    type SinkItem = T;
    type SinkError = Void;

    fn start_send(&mut self, item: T) -> Result<AsyncSink<T>, Void> {
        Ok(self.tx.start_send(item).unwrap_or(AsyncSink::Ready))
    }

    fn poll_complete(&mut self) -> Result<Async<()>, Void> {
        Ok(self.tx.poll_complete().unwrap_or(Async::Ready(())))
    }
}

