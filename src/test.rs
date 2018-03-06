use priv_prelude::*;
use std::sync::mpsc;
use void;
use env_logger;

pub fn run_test<F: FnOnce() + Send + 'static>(seconds: u64, func: F) {
    let _ = env_logger::init();

    let (tx, rx) = mpsc::channel();

    let join_handle = thread::spawn(move || {
        trace!("run_test: entering spawned thread");
        func();
        drop(tx);
        trace!("run_test: exiting spawned thread");
    });

    match rx.recv_timeout(Duration::from_secs(seconds)) {
        Ok(v) => void::unreachable(v),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test timed out!"),
        Err(mpsc::RecvTimeoutError::Disconnected) => (),
    };

    unwrap!(join_handle.join());
}

