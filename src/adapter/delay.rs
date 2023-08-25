use crate::priv_prelude::*;

#[pin_project]
pub struct Delay<S>
where
    S: Stream,
{
    min_delay: Duration,
    mean_additional_delay: Duration,
    stream_finished: bool,
    #[pin]
    stream: S,
    #[pin]
    sleep_opt: Option<tokio::time::Sleep>,
    pending: BTreeMap<Instant, Vec<<S as Stream>::Item>>,
}

impl<S> Delay<S>
where
    S: Stream,
{
    pub fn new(stream: S, min_delay: Duration, mean_additional_delay: Duration) -> Delay<S> {
        Delay {
            min_delay,
            mean_additional_delay,
            stream,
            stream_finished: false,
            sleep_opt: None,
            pending: BTreeMap::new(),
        }
    }
}

impl<S> Stream for Delay<S>
where
    S: Stream,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        let mut this = self.project();
        if !*this.stream_finished {
            loop {
                match this.stream.as_mut().poll_next(cx) {
                    Poll::Ready(Some(value)) => {
                        let delay = *this.min_delay + adapter::expovariate_duration(
                            *this.mean_additional_delay,
                            &mut rand::thread_rng(),
                        );
                        let instant = Instant::now() + delay;
                        match this.sleep_opt.as_mut().as_pin_mut() {
                            Some(_sleep) => (),
                            None => {
                                let sleep = tokio::time::sleep_until(instant.into());
                                this.sleep_opt.set(Some(sleep));
                            },
                        }
                        this.pending.entry(instant).or_default().push(value);
                    },
                    Poll::Ready(None) => {
                        *this.stream_finished = true;
                        break;
                    },
                    Poll::Pending => break,
                }
            }
        }
        let pending_finished = match this.sleep_opt.as_mut().as_pin_mut() {
            None => true,
            Some(mut sleep) => {
                match sleep.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let mut entry = this.pending.first_entry().unwrap();
                        let value = entry.get_mut().pop().unwrap();
                        let next_instant_opt = if entry.get().is_empty() {
                            let _ = entry.remove();
                            match this.pending.first_key_value() {
                                None => None,
                                Some((&instant, _value)) => Some(instant),
                            }
                        } else {
                            Some(*entry.key())
                        };
                        match next_instant_opt {
                            None => this.sleep_opt.set(None),
                            Some(instant) => sleep.reset(instant.into()),
                        }
                        return Poll::Ready(Some(value));
                    },
                    Poll::Pending => false,
                }
            },
        };
        if *this.stream_finished && pending_finished {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

impl<S, T> Sink<T> for Delay<S>
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

