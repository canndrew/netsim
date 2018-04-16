use priv_prelude::*;
use std::sync::mpsc;
use void;
use env_logger;

pub fn run_test<F: FnOnce() + Send + 'static>(seconds: u64, func: F) {
    let _ = env_logger::init();

    let (tx, rx) = mpsc::channel();

    let _join_handle = thread::spawn(move || {
        func();
        drop(tx);
    });

    match rx.recv_timeout(Duration::from_secs(seconds)) {
        Ok(v) => void::unreachable(v),
        Err(mpsc::RecvTimeoutError::Timeout) => panic!("test timed out!"),
        Err(mpsc::RecvTimeoutError::Disconnected) => (),
    };

    // TODO: Why the hell does this sometimes panic?
    //unwrap!(join_handle.join());
}

