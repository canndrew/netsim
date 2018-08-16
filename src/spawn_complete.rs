//! This module implements futures to wait for task completion.

use priv_prelude::*;

/// A handle to the spawned network-isolated thread. Implements `Future` so that you can wait for
/// the thread to complete.
pub struct SpawnComplete<R> {
    ret_rx: oneshot::Receiver<thread::Result<R>>,
    process_handle: Option<ProcessHandle>,
}

impl<R> Future for SpawnComplete<R> {
    type Item = R;
    type Error = Box<Any + Send + 'static>;

    fn poll(&mut self) -> thread::Result<Async<R>> {
        match self.ret_rx.poll() {
            Ok(Async::Ready(res)) => {
                if let Some(mut process_handle) = self.process_handle.take() {
                    process_handle.busy_wait_for_exit();
                }
                res.map(Async::Ready)
            },
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(oneshot::Canceled) => {
                let holds_process = self.process_handle.is_some();
                panic!("thread destroyed without sending response!? {}", holds_process);
            },
        }
    }
}

/// Constructs `SpawnComplete` that waits for a completion signal and until a given process stops.
pub fn from_parts<R>(
    ret_rx: oneshot::Receiver<thread::Result<R>>,
    process_handle: ProcessHandle,
) -> SpawnComplete<R> {
    SpawnComplete {
        ret_rx,
        process_handle: Some(process_handle),
    }
}

/// Constructs `SpawnComplete` that waits for a completion signal.
pub fn from_receiver<R>(
    ret_rx: oneshot::Receiver<thread::Result<R>>,
) -> SpawnComplete<R> {
    SpawnComplete {
        ret_rx,
        process_handle: None,
    }
}

