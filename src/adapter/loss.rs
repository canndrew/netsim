use crate::priv_prelude::*;

#[pin_project]
pub struct Loss<S> {
    #[pin]
    stream: S,
    jitter: Jitter,
}

struct Jitter {
    loss_rate: f64,
    jitter_period: Duration,
    currently_dropping: bool,
    prev_switch_instant: Instant,
    next_switch_instant: Instant,
}

impl Jitter {
    pub fn new(loss_rate: f64, jitter_period: Duration) -> Jitter {
        let now = Instant::now();
        let mut jitter = Jitter {
            loss_rate,
            jitter_period,
            currently_dropping: false,
            prev_switch_instant: now,
            next_switch_instant: now,
        };
        jitter.reset(now);
        jitter
    }

    pub fn reset(&mut self, switch_instant: Instant) {
        self.prev_switch_instant = switch_instant;
        self.currently_dropping = rand::thread_rng().gen::<f64>() < self.loss_rate;
        self.set_next_switch_instant();
    }

    pub fn advance(&mut self) {
        let now = Instant::now();
        if self.next_switch_instant + (self.jitter_period * 10) < now {
            self.reset(now);
            return;
        }
        while self.next_switch_instant < now {
            self.prev_switch_instant = self.next_switch_instant;
            self.currently_dropping = !self.currently_dropping;
            self.set_next_switch_instant();
        }
    }

    pub fn currently_dropping(&self) -> bool {
        self.currently_dropping
    }

    fn set_next_switch_instant(&mut self) {
        let delay = if self.currently_dropping {
            self.jitter_period.mul_f64(self.loss_rate)
        } else {
            self.jitter_period.mul_f64(1.0 - self.loss_rate)
        };
        self.next_switch_instant = {
            self.prev_switch_instant + adapter::expovariate_duration(delay, &mut rand::thread_rng())
        };
    }
}

impl<S> Loss<S> {
    pub fn new(stream: S, loss_rate: f64, jitter_period: Duration) -> Loss<S> {
        Loss {
            stream,
            jitter: Jitter::new(loss_rate, jitter_period),
        }
    }
}

impl<S> Stream for Loss<S>
where
    S: Stream,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        let mut this = self.project();
        this.jitter.advance();
        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(value)) => {
                    if this.jitter.currently_dropping() {
                        continue;
                    }
                    break Poll::Ready(Some(value));
                },
                Poll::Ready(None) => break Poll::Ready(None),
                Poll::Pending => break Poll::Pending,
            }
        }
    }
}

impl<S, T> Sink<T> for Loss<S>
where
    S: Stream,
    S: Sink<T>,
{
    type Error = <S as Sink<T>>::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.stream.poll_ready(cx)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let this = self.project();
        this.stream.start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.stream.poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.stream.poll_close(cx)
    }
}

