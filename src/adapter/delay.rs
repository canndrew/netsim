use crate::priv_prelude::*;

#[pin_project]
pub struct Delay<S, T, R> {
    min_delay: Duration,
    mean_additional_delay: Duration,
    #[pin]
    stream_opt: Option<S>,
    rng: R,
    pending: FuturesUnordered<Pin<Box<dyn Future<Output = T> + Send + 'static>>>,
}

impl<S, T, R> Delay<S, T, R>
where
    S: Stream<Item = T>,
    R: Rng,
{
    pub fn new(stream: S, rng: R, min_delay: Duration, mean_additional_delay: Duration) -> Delay<S, T, R> {
        Delay {
            min_delay,
            mean_additional_delay,
            stream_opt: Some(stream),
            rng,
            pending: FuturesUnordered::new(),
        }
    }
}

fn calculate_delay<R>(
    min_delay: Duration,
    mean_additional_delay: Duration,
    rng: &mut R,
) -> Duration
where
    R: Rng,
{
    let mean_additional_delay = mean_additional_delay.as_secs_f64();
    let additional_delay = loop {
        let additional_delay = mean_additional_delay * -rng.gen::<f64>().ln();
        match Duration::try_from_secs_f64(additional_delay) {
            Ok(additional_delay) => break additional_delay,
            Err(_) => continue,
        }
    };
    min_delay + additional_delay
}


impl<S, T, R> Stream for Delay<S, T, R>
where
    S: Stream<Item = T>,
    R: Rng,
    T: Send + 'static,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<T>> {
        let mut this = self.project();
        let stream_finished = match this.stream_opt.as_mut().as_pin_mut() {
            None => true,
            Some(mut stream) => {
                loop {
                    match stream.as_mut().poll_next(cx) {
                        Poll::Ready(Some(value)) => {
                            let delay = calculate_delay(
                                *this.min_delay,
                                *this.mean_additional_delay,
                                this.rng,
                            );
                            let instant = tokio::time::Instant::now() + delay;
                            let future = async move {
                                tokio::time::sleep_until(instant).await;
                                value
                            };
                            this.pending.push(Box::pin(future));
                        },
                        Poll::Ready(None) => {
                            this.stream_opt.set(None);
                            break true;
                        },
                        Poll::Pending => {
                            break false;
                        },
                    }
                }
            },
        };
        let pending_finished = match Pin::new(this.pending).poll_next(cx) {
            Poll::Ready(Some(value)) => {
                return Poll::Ready(Some(value));
            },
            Poll::Ready(None) => true,
            Poll::Pending => false,
        };
        if stream_finished && pending_finished {
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

