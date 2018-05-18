use priv_prelude::*;
use future_utils;
use std;

/// A `Network` manages a set of tasks/devices. Dropping the `Network` will destroy all associated
/// tasks/devices.
pub struct Network {
    handle: Handle,
    drop_tx_tx: std::sync::mpsc::Sender<DropNotify>,
    _drop_tx_rx: std::sync::mpsc::Receiver<DropNotify>,
}

impl Network {
    /// Create a new `Network` running on the given event loop.
    pub fn new(handle: &Handle) -> Network {
        let (drop_tx_tx, drop_tx_rx) = std::sync::mpsc::channel();
        Network {
            handle: handle.clone(),
            drop_tx_tx: drop_tx_tx,
            _drop_tx_rx: drop_tx_rx,
        }
    }

    /// Get a handle to the network. Can be used to spawn tasks to the network.
    pub fn handle(&self) -> NetworkHandle {
        NetworkHandle {
            handle: self.handle.clone(),
            drop_tx_tx: self.drop_tx_tx.clone(),
        }
    }
}

#[derive(Clone)]
/// A handle to a `Network`
pub struct NetworkHandle {
    handle: Handle,
    drop_tx_tx: std::sync::mpsc::Sender<DropNotify>,
}

impl NetworkHandle {
    /// Get a copy of the event loop handle
    pub fn event_loop(&self) -> &Handle {
        &self.handle
    }

    /// Spawn a future to the event loop. The future will be cancelled when the `Network` is
    /// destroyed,
    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Item = (), Error = Void> + 'static,
    {
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        if self.drop_tx_tx.send(drop_tx).is_err() {
            panic!("network has been destroyed");
        }

        self.handle.spawn({
            f
            .until(drop_rx)
            .map(|_unit_opt| ())
            .infallible()
        });
    }
}

