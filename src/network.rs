use crate::priv_prelude::*;
use future_utils;
use std;
use tokio;

/// A `Network` manages a set of tasks/devices. Dropping the `Network` will destroy all associated
/// tasks/devices.
pub struct Network {
    drop_tx_tx: std::sync::mpsc::Sender<DropNotify>,
    _drop_tx_rx: std::sync::mpsc::Receiver<DropNotify>,
}

impl Network {
    /// Create a new `Network` running on the given event loop.
    pub fn new() -> Network {
        let (drop_tx_tx, drop_tx_rx) = std::sync::mpsc::channel();
        Network {
            drop_tx_tx,
            _drop_tx_rx: drop_tx_rx,
        }
    }

    /// Get a handle to the network. Can be used to spawn tasks to the network.
    pub fn handle(&self) -> NetworkHandle {
        NetworkHandle {
            drop_tx_tx: self.drop_tx_tx.clone(),
        }
    }

    /// Spawn a hierarchical network of nodes. The returned plug can be used to write packets to the
    /// network and read packets that try to leave the network.
    pub fn spawn_ipv4_tree<N: Ipv4Node>(
        &self,
        ipv4_range: Ipv4Range,
        node: N,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        node.build(&self.handle(), ipv4_range)
    }

    /// Spawn a hierarchical network of nodes. The returned plug can be used to write packets to the
    /// network and read packets that try to leave the network.
    pub fn spawn_ipv6_tree<N: Ipv6Node>(
        &self,
        ipv6_range: Ipv6Range,
        node: N,
    ) -> (SpawnComplete<N::Output>, Ipv6Plug) {
        node.build(&self.handle(), ipv6_range)
    }
}

#[derive(Clone)]
/// A handle to a `Network`
pub struct NetworkHandle {
    drop_tx_tx: std::sync::mpsc::Sender<DropNotify>,
}

impl NetworkHandle {
    /// Spawn a future to the event loop. The future will be cancelled when the `Network` is
    /// destroyed,
    pub fn spawn<F>(&self, f: F)
    where
        F: Future<Item = (), Error = Void> + Send + 'static,
    {
        let (drop_tx, drop_rx) = future_utils::drop_notify();
        if self.drop_tx_tx.send(drop_tx).is_err() {
            panic!("network has been destroyed");
        }

        tokio::spawn({
            f
            .until(drop_rx)
            .map(|_unit_opt| ())
            .infallible()
        });
    }

    /// Spawn a hierarchical network of nodes. The returned plug can be used to write packets to the
    /// network and read packets that try to leave the network.
    pub fn spawn_ipv4_tree<N: Ipv4Node>(
        &self,
        ipv4_range: Ipv4Range,
        node: N,
    ) -> (SpawnComplete<N::Output>, Ipv4Plug) {
        node.build(&self, ipv4_range)
    }
}
