use priv_prelude::*;
use std;
use future_utils;
use spawn;
use self::tap::TapTask;
use self::tun::TunTask;

mod tap;
mod tun;

#[derive(Default)]
/// A builder for creating a virtual network machine.
/// Machines are simulated using Linux TUN/TAP devices and network namespaces.
pub struct MachineBuilder {
    ether_ifaces: Vec<(EtherIfaceBuilder, EtherPlug)>,
    ip_ifaces: Vec<(IpIfaceBuilder, IpPlug)>,
}

impl MachineBuilder {
    /// Create a new `MachineBuilder` with no interfaces.
    pub fn new() -> MachineBuilder {
        MachineBuilder::default()
    }

    /// Add an ethernet (TAP) interface to the machine
    pub fn add_ether_iface(
        mut self,
        iface: EtherIfaceBuilder,
        plug: EtherPlug,
    ) -> MachineBuilder {
        self.ether_ifaces.push((iface, plug));
        self
    }

    /// Add an IP (TUN) interface to the machine
    pub fn add_ip_iface(
        mut self,
        iface: IpIfaceBuilder,
        plug: IpPlug,
    ) -> MachineBuilder {
        self.ip_ifaces.push((iface, plug));
        self
    }

    /// Spawn the machine onto the event loop. The returned `SpawnComplete` will resolve with the
    /// value returned by the given function.
    pub fn spawn<F, R>(
        self,
        handle: &NetworkHandle,
        func: F,
    ) -> SpawnComplete<R>
    where
        F: FnOnce() -> R,
        F: Send + 'static,
        R: Send + 'static,
    {
        let (ether_tx, ether_rx) = std::sync::mpsc::channel();
        let (ip_tx, ip_rx) = std::sync::mpsc::channel();
        let spawn_complete = spawn::new_namespace(move || {
            let mut drop_txs = Vec::new();

            for (iface, plug) in self.ether_ifaces {
                let (drop_tx, drop_rx) = future_utils::drop_notify();
                let tap_unbound = unwrap!(iface.build_unbound());
                unwrap!(ether_tx.send((tap_unbound, plug, drop_rx)));
                drop_txs.push(drop_tx);
            }
            drop(ether_tx);

            for (iface, plug) in self.ip_ifaces {
                let (drop_tx, drop_rx) = future_utils::drop_notify();
                let tun_unbound = unwrap!(iface.build_unbound());
                unwrap!(ip_tx.send((tun_unbound, plug, drop_rx)));
                drop_txs.push(drop_tx);
            }
            drop(ip_tx);

            let ret = func();
            drop(drop_txs);
            ret
        });

        for (tap_unbound, plug, drop_rx) in ether_rx {
            let tap = tap_unbound.bind();
            let task = TapTask::new(tap, plug, drop_rx);
            handle.spawn(task.infallible());
        }

        for (tun_unbound, plug, drop_rx) in ip_rx {
            let tun = tun_unbound.bind();
            let task = TunTask::new(tun, plug, drop_rx);
            handle.spawn(task.infallible());
        }

        spawn_complete
    }
}

