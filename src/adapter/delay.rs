use crate::priv_prelude::*;

/// `Sink`/`Stream` adapter which adds a time delay to items sent/received through the
/// `Sink`/`Stream`. Can be created via
/// [`SinkStreamExt::with_delay`](crate::SinkStreamExt::with_delay).
#[pin_project]
pub struct Delay<S, T>
where
    S: Stream + Sink<T>,
{
    min_delay: Duration,
    mean_additional_delay: Duration,
    stream_finished: bool,
    #[pin]
    stream: S,
    #[pin]
    stream_queue: DelayQueue<<S as Stream>::Item>,
    #[pin]
    sink_queue: DelayQueue<T>,
}

#[pin_project]
struct DelayQueue<T> {
    #[pin]
    sleep_opt: Option<tokio::time::Sleep>,
    pending: BTreeMap<Instant, Vec<T>>,
}

impl<T> DelayQueue<T> {
    pub fn new() -> DelayQueue<T> {
        DelayQueue {
            sleep_opt: None,
            pending: BTreeMap::new(),
        }
    }

    pub fn push(self: Pin<&mut Self>, delay: Duration, value: T) {
        let mut this = self.project();
        let instant = Instant::now() + delay;
        match this.sleep_opt.as_mut().as_pin_mut() {
            Some(_sleep) => (),
            None => {
                this.sleep_opt.set(Some(tokio::time::sleep_until(instant.into())));
            },
        }
        this.pending.entry(instant).or_default().push(value);
    }

    pub fn pop(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<T>> {
        let mut this = self.project();
        match this.sleep_opt.as_mut().as_pin_mut() {
            None => Poll::Ready(None),
            Some(mut sleep) => {
                match sleep.as_mut().poll(cx) {
                    Poll::Ready(()) => {
                        let mut entry = this.pending.first_entry().unwrap();
                        let value = entry.get_mut().pop().unwrap();
                        let next_instant_opt = if entry.get().is_empty() {
                            let _ = entry.remove();
                            this.pending.first_key_value().map(|(&instant, _values)| instant)
                        } else {
                            Some(*entry.key())
                        };
                        match next_instant_opt {
                            None => this.sleep_opt.set(None),
                            Some(instant) => sleep.reset(instant.into()),
                        }
                        Poll::Ready(Some(value))
                    },
                    Poll::Pending => Poll::Pending,
                }
            },
        }
    }
}

impl<S, T> Delay<S, T>
where
    S: Stream + Sink<T>,
{
    /// Creates a new [`Delay`]. See the documentation for
    /// [`SinkStreamExt::with_delay`](crate::SinkStreamExt::with_delay).
    pub fn new(stream: S, min_delay: Duration, mean_additional_delay: Duration) -> Delay<S, T> {
        Delay {
            min_delay,
            mean_additional_delay,
            stream,
            stream_finished: false,
            stream_queue: DelayQueue::new(),
            sink_queue: DelayQueue::new(),
        }
    }
}

impl<S, T> Stream for Delay<S, T>
where
    S: Stream + Sink<T>,
{
    type Item = <S as Stream>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        if !*this.stream_finished {
            loop {
                match this.stream.as_mut().poll_next(cx) {
                    Poll::Ready(Some(value)) => {
                        let delay = *this.min_delay + adapter::expovariate_duration(
                            *this.mean_additional_delay,
                            &mut rand::thread_rng(),
                        );
                        this.stream_queue.as_mut().push(delay, value);
                    },
                    Poll::Ready(None) => {
                        *this.stream_finished = true;
                        break;
                    },
                    Poll::Pending => break,
                }
            }
        }
        let pending_finished = match this.stream_queue.pop(cx) {
            Poll::Pending => false,
            Poll::Ready(None) => true,
            Poll::Ready(Some(value)) => return Poll::Ready(Some(value)),
        };
        if *this.stream_finished && pending_finished {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

impl<S, T> Sink<T> for Delay<S, T>
where
    S: Stream,
    S: Sink<T>,
{
    type Error = <S as Sink<T>>::Error;

    fn poll_ready(self: Pin<&mut Self>, _cx: &mut task::Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        let this = self.project();
        let delay = *this.min_delay + adapter::expovariate_duration(
            *this.mean_additional_delay,
            &mut rand::thread_rng(),
        );
        this.sink_queue.push(delay, item);
        Ok(())
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let mut this = self.project();
        loop {
            match this.stream.as_mut().poll_ready(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Ready(Ok(())) => (),
            }
            match this.sink_queue.as_mut().pop(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => {
                    return this.stream.poll_flush(cx);
                },
                Poll::Ready(Some(item)) => {
                    match this.stream.as_mut().start_send(item) {
                        Ok(()) => (),
                        Err(err) => return Poll::Ready(Err(err)),
                    }
                    match this.stream.as_mut().poll_flush(cx) {
                        Poll::Pending => (),
                        Poll::Ready(Ok(())) => (),
                        Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                    }
                },
            }
        };
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.project();
        this.stream.poll_close(cx)
    }
}

