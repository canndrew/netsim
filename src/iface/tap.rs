//! Contains utilites for working with virtual (TAP) network interfaces.

use priv_prelude::*;
use sys;
use iface::build::{IfaceBuilder, build};

/// This object can be used to set the configuration options for a `EtherIface` before creating the
/// `EtherIface`
/// using `build`.
#[derive(Debug)]
pub struct EtherIfaceBuilder {
    mac_addr: MacAddr,
    builder: IfaceBuilder,
}

impl Default for EtherIfaceBuilder {
    fn default() -> EtherIfaceBuilder {
        EtherIfaceBuilder {
            mac_addr: MacAddr::random(),
            builder: IfaceBuilder {
                name: String::from("netsim"),
                ipv4_addr: None,
                ipv6_addr: None,
                ipv4_routes: Vec::new(),
                ipv6_routes: Vec::new(),
            },
        }
    }
}

impl EtherIfaceBuilder {
    /// Start building a new `EtherIface` with the default configuration options.
    pub fn new() -> EtherIfaceBuilder {
        Default::default()
    }

    /// Set the interface name.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.builder.name = name.into();
        self
    }

    /// Set the interface MAC address.
    pub fn mac_addr(mut self, addr: MacAddr) -> Self {
        self.mac_addr = addr;
        self
    }

    /// Set the interface's IPv4 address and netmask
    pub fn ipv4_addr(mut self, addr: Ipv4Addr, netmask_bits: u8) -> Self {
        self.builder.ipv4_addr = Some((addr, netmask_bits));
        self
    }

    /// Set the interface's IPv6 address and netmask
    pub fn ipv6_addr(mut self, addr: Ipv6Addr, netmask_bits: u8) -> Self {
        self.builder.ipv6_addr = Some((addr, netmask_bits));
        self
    }

    /// Add an IPv4 route to the interface
    pub fn ipv4_route(mut self, route: RouteV4) -> Self {
        self.builder.ipv4_routes.push(route);
        self
    }

    /// Add an IPv6 route to the interface
    pub fn ipv6_route(mut self, route: RouteV6) -> Self {
        self.builder.ipv6_routes.push(route);
        self
    }

    /// Add a route to the set of routes that will be created and directed through this interface.
    pub fn route(mut self, route: RouteV4) -> Self {
        self.builder.ipv4_routes.push(route);
        self
    }

    /// Consume this `EtherIfaceBuilder` and build a `UnboundEtherIface`. This creates the TAP device but does not
    /// bind it to a tokio event loop. This is useful if the event loop lives in a different thread
    /// to where you need to create the device. You can send a `UnboundEtherIface` to another thread then
    /// `bind` it to create your `EtherIface`.
    pub fn build_unbound(self) -> Result<UnboundEtherIface, IfaceBuildError> {
        let fd = build(self.builder, Some(self.mac_addr))?;

        trace!("creating TAP");

        Ok(UnboundEtherIface { fd })
    }

    /// Consume this `EtherIfaceBuilder` and build the TAP interface. The returned `EtherIface` object can be
    /// used to read/write ethernet frames from this interface. `handle` is a handle to a tokio
    /// event loop which will be used for reading/writing.
    pub fn build(self, handle: &Handle) -> Result<EtherIface, IfaceBuildError> {
        Ok(self.build_unbound()?.bind(handle))
    }
}

/// Represents a TAP device which has been built but not bound to a tokio event loop.
#[derive(Debug)]
pub struct UnboundEtherIface {
    fd: AsyncFd,
}

impl UnboundEtherIface {
    /// Bind the tap device to the event loop, creating a `EtherIface` which you can read/write ethernet
    /// frames with.
    pub fn bind(self, handle: &Handle) -> EtherIface {
        let UnboundEtherIface { fd } = self;
        let fd = unwrap!(PollEvented::new(fd, handle));
        EtherIface { fd }
    }
}

/// A handle to a virtual (TAP) network interface. Can be used to read/write ethernet frames
/// directly to the device.
pub struct EtherIface {
    fd: PollEvented<AsyncFd>,
}

impl Stream for EtherIface {
    type Item = EtherFrame;
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        if let Async::NotReady = self.fd.poll_read() {
            return Ok(Async::NotReady);
        }

        let mut buffer: [u8; sys::ETH_FRAME_LEN as usize] = unsafe {
            mem::uninitialized()
        };
        match self.fd.read(&mut buffer[..]) {
            Ok(0) => Ok(Async::Ready(None)),
            Ok(n) => {

                /*
                'out: for i in 0.. {
                    println!("");
                    for j in 0..4 {
                        let pos = i * 4 + j;
                        if pos < n {
                            print!("{:02x}", buffer[pos]);
                        } else {
                            break 'out;
                        }
                    }
                }
                println!("");
                */

                let bytes = Bytes::from(&buffer[..n]);
                let frame = EtherFrame::from_bytes(bytes);
                info!("TAP sending frame: {:?}", frame);
                Ok(Async::Ready(Some(frame)))
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_read();
                Ok(Async::NotReady)
            },
            Err(e) => Err(e),
        }
    }
}

impl Sink for EtherIface {
    type SinkItem = EtherFrame;
    type SinkError = io::Error;
    
    fn start_send(&mut self, item: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        info!("TAP received frame: {:?}", item);
        if let Async::NotReady = self.fd.poll_write() {
            return Ok(AsyncSink::NotReady(item));
        }

        /*
        trace!("frame as bytes:");
        for chunk in item.as_bytes().chunks(8) {
            let mut s = String::new();
            for b in chunk {
                use std::fmt::Write;
                write!(&mut s, " {:02x}", b).unwrap();
            }
            trace!("   {}", s);
        }
        */

        match self.fd.write(item.as_bytes()) {
            Ok(n) => assert_eq!(n, item.as_bytes().len()),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.need_write();
                return Ok(AsyncSink::NotReady(item));
            }
            Err(e) => return Err(e),
        }
        trace!("sent: {:?}", item);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        Ok(Async::Ready(()))
    }
}

#[cfg(test)]
mod test {
    use priv_prelude::*;
    use spawn;
    use capabilities;
    use rand;
    use iface;
    use get_if_addrs::{self, IfAddr};

    #[test]
    fn build_tap_correct_settings() {
        run_test(1, || {
            let spawn_complete = spawn::new_namespace(|| {
                let mac_addr = MacAddr::random();
                let ipv4_addr = SubnetV4::local_10().random_client_addr();
                let ipv4_netmask_bits = 8 + rand::random::<u8>() % 24;
                let ipv6_addr = Ipv6Addr::random_global();
                let ipv6_netmask_bits = 64 + rand::random::<u8>() % 64;
                let name = format!("foo{:x}", rand::random::<u32>());

                let tap_builder = {
                    EtherIfaceBuilder::new()
                    .mac_addr(mac_addr)
                    .ipv4_addr(ipv4_addr, ipv4_netmask_bits)
                    .ipv6_addr(ipv6_addr, ipv6_netmask_bits)
                    .name(name.clone())
                };

                let tap = unwrap!(tap_builder.build_unbound());

                let found_mac_addr = unwrap!(iface::get_mac_addr(&name));
                assert_eq!(mac_addr, found_mac_addr);

                let mut found_v4 = false;
                let mut found_v6 = false;
                let ifaces = unwrap!(get_if_addrs::get_if_addrs());

                for iface in ifaces {
                    assert_eq!(iface.name, name);
                    match iface.addr {
                        IfAddr::V4(ref ifv4_addr) => {
                            assert!(!found_v4);
                            assert_eq!(ifv4_addr.ip, ipv4_addr);
                            assert_eq!(
                                ifv4_addr.netmask,
                                Ipv4Addr::from_netmask_bits(ipv4_netmask_bits),
                            );

                            found_v4 = true;
                        },
                        IfAddr::V6(ref ifv6_addr) => {
                            assert!(!found_v6);
                            assert_eq!(ifv6_addr.ip, ipv6_addr);
                            assert_eq!(
                                ifv6_addr.netmask,
                                Ipv6Addr::from_netmask_bits(ipv6_netmask_bits),
                            );

                            found_v6 = true;
                        },
                    }

                    assert!(found_v4 && found_v6);
                }

                drop(tap);
            });
            let mut core = unwrap!(Core::new());
            unwrap!(core.run(spawn_complete))
        })
    }

    #[test]
    fn build_tap_name_contains_nul() {
        run_test(1, || {
            let tap_builder = {
                EtherIfaceBuilder::new()
                .ipv4_addr(Ipv4Addr::random_global(), 0)
                .name("hello\0")
            };
            let res = tap_builder.build_unbound();
            match res {
                Err(IfaceBuildError::NameContainsNul) => (),
                x => panic!("unexpected result: {:?}", x),
            }
        })
    }

    #[test]
    fn build_tap_duplicate_name() {
        run_test(3, || {
            let spawn_complete = spawn::new_namespace(|| {
                let tap_builder = {
                    EtherIfaceBuilder::new()
                    .ipv4_addr(Ipv4Addr::random_global(), 0)
                    .name("hello")
                };
                trace!("build_tap_duplicate_name: building first interface");
                let _tap = unwrap!(tap_builder.build_unbound());
                
                let tap_builder = {
                    EtherIfaceBuilder::new()
                    .ipv4_addr(Ipv4Addr::random_global(), 0)
                    .name("hello")
                };
                trace!("build_tap_duplicate_name: building second interface");
                match tap_builder.build_unbound() {
                    Err(IfaceBuildError::InterfaceAlreadyExists) => (),
                    res => panic!("unexpected result: {:?}", res),
                }
                trace!("build_tap_duplicate_name: done");
            });
            let mut core = unwrap!(Core::new());
            unwrap!(core.run(spawn_complete))
        });
    }

    #[test]
    fn build_tap_permission_denied() {
        run_test(3, || {
            let spawn_complete = spawn::new_namespace(|| {
                unwrap!(unwrap!(capabilities::Capabilities::new()).apply());

                let tap_builder = EtherIfaceBuilder::new();
                match tap_builder.build_unbound() {
                    Err(IfaceBuildError::CreateIfacePermissionDenied) => (),
                    res => panic!("unexpected result: {:?}", res),
                }
            });
            let mut core = unwrap!(Core::new());
            unwrap!(core.run(spawn_complete))
        })
    }
}

