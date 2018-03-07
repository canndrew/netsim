use priv_prelude::*;

/// A handle to the spawned network-isolated thread. Implements `Future` so that you can wait for
/// the thread to complete.
pub struct SpawnComplete<R> {
    ret_rx: oneshot::Receiver<thread::Result<R>>,
}

impl<R> Future for SpawnComplete<R> {
    type Item = R;
    type Error = Box<Any + Send + 'static>;

    fn poll(&mut self) -> thread::Result<Async<R>> {
        match self.ret_rx.poll() {
            Ok(Async::Ready(Ok(r))) => Ok(Async::Ready(r)),
            Ok(Async::Ready(Err(e))) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(oneshot::Canceled) => panic!("thread destroyed without sending response!?"),
        }
    }
}

pub fn from_receiver<R>(
    ret_rx: oneshot::Receiver<thread::Result<R>>,
) -> SpawnComplete<R> {
    SpawnComplete {
        ret_rx,
    }
}

