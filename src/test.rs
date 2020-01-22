use crate::priv_prelude::*;
use env_logger;
use std::sync::mpsc;
use void;

/// Runs callback in a new thread with the given timeout. Panics, if `func()` panics.
pub fn run_test<F: FnOnce() + Send + 'static>(seconds: u64, func: F) {
    let _ = env_logger::init();

    let (tx, rx) = mpsc::channel();

    let join_handle = thread::spawn(move || {
        func();
        drop(tx);
    });

    match rx.recv_timeout(Duration::from_secs(seconds)) {
        Ok(v) => void::unreachable(v),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test timed out!"),
        Err(mpsc::RecvTimeoutError::Disconnected) => (),
    };

    // FIXME: this sometimes panics, even if the thread of join_handle didn't.
    unwrap!(join_handle.join());
}
