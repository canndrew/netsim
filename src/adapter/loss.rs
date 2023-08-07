use crate::priv_prelude::*;

#[pin_project]
pub struct Loss<S, R> {
    #[pin]
    stream: S,
    rng: R,
    loss: f64,
}

impl<S, R> Loss<S, R> {
    pub fn new(stream: S, rng: R, loss: f64) -> Loss<S, R> {
        Loss { stream, rng, loss }
    }
}

impl<S, R> Stream for Loss<S, R>
where
    S: Stream,
    R: Rng,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut task::Context) -> Poll<Option<S::Item>> {
        let mut this = self.project();
        loop {
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(value)) => {
                    if this.rng.gen::<f64>() < *this.loss {
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

