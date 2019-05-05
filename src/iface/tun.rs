//! Contains utilites for working with virtual (TUN) network interfaces.

use crate::priv_prelude::*;
use libc;
use crate::iface::build::{IfaceBuilder, build};

/// This object can be used to set the configuration options for a `IpIface` before creating the
/// `IpIface`
/// using `build`.
#[derive(Debug)]
pub struct IpIfaceBuilder {
    builder: IfaceBuilder,
}

impl Default for IpIfaceBuilder {
    fn default() -> IpIfaceBuilder {
        IpIfaceBuilder {
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

impl IpIfaceBuilder {
    /// Start building a new `IpIface` with the default configuration options.
    pub fn new() -> IpIfaceBuilder {
        Default::default()
    }

    /// Set the interface name.
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.builder.name = name.into();
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
    pub fn ipv4_route(mut self, route: Ipv4Route) -> Self {
        self.builder.ipv4_routes.push(route);
        self
    }

    /// Add an IPv6 route to the interface
    pub fn ipv6_route(mut self, route: Ipv6Route) -> Self {
        self.builder.ipv6_routes.push(route);
        self
    }

    /// Consume this `IpIfaceBuilder` and build a `UnboundIpIface`. This creates the TUN device but does not
    /// bind it to a tokio event loop. This is useful if the event loop lives in a different thread
    /// to where you need to create the device. You can send a `UnboundIpIface` to another thread then
    /// `bind` it to create your `IpIface`.
    pub fn build_unbound(self) -> Result<UnboundIpIface, IfaceBuildError> {
        let fd = build(self.builder, None)?;

        trace!("creating TUN");

        Ok(UnboundIpIface { fd })
    }

    /// Consume this `IpIfaceBuilder` and build the TUN interface. The returned `IpIface` object can be
    /// used to read/write ethernet frames from this interface. `handle` is a handle to a tokio
    /// event loop which will be used for reading/writing.
    pub fn build(self) -> Result<IpIface, IfaceBuildError> {
        Ok(self.build_unbound()?.bind())
    }
}

/// Represents a TUN device which has been built but not bound to a tokio event loop.
#[derive(Debug)]
pub struct UnboundIpIface {
    fd: AsyncFd,
}

impl UnboundIpIface {
    /// Bind the tap device to the event loop, creating a `IpIface` which you can read/write ethernet
    /// frames with.
    pub fn bind(self) -> IpIface {
        let UnboundIpIface { fd } = self;
        let fd = PollEvented2::new(fd);
        IpIface { fd }
    }
}

/// A handle to a virtual (TUN) network interface. Can be used to read/write ethernet frames
/// directly to the device.
pub struct IpIface {
    fd: PollEvented2<AsyncFd>,
}

impl Stream for IpIface {
    type Item = IpPacket;
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<Option<IpPacket>>> {
        loop {
            if let Async::NotReady = self.fd.poll_read_ready(Ready::readable())? {
                return Ok(Async::NotReady);
            }

            let mut buffer: [u8; libc::ETH_FRAME_LEN as usize] = unsafe {
                mem::uninitialized()
            };
            match self.fd.read(&mut buffer[..]) {
                Ok(0) => return Ok(Async::Ready(None)),
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

                    if buffer[0] >> 4 != 4 {
                        info!("TUN dropping packet with version {}", buffer[0] >> 4);
                        continue;
                    }
                    let bytes = Bytes::from(&buffer[..n]);
                    let frame = IpPacket::from_bytes(bytes);
                    info!("TUN emitting frame: {:?}", frame);
                    return Ok(Async::Ready(Some(frame)));
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.fd.clear_read_ready(Ready::readable())?;
                    return Ok(Async::NotReady);
                },
                Err(e) => return Err(e),
            }
        }
    }
}

impl Sink for IpIface {
    type SinkItem = IpPacket;
    type SinkError = io::Error;
    
    fn start_send(&mut self, item: IpPacket) -> io::Result<AsyncSink<IpPacket>> {
        info!("TUN received frame: {:?}", item);
        if let Async::NotReady = self.fd.poll_write_ready()? {
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
            Ok(n) => {
                trace!("wrote {} bytes of TUN data to interface", n);
                assert_eq!(n, item.as_bytes().len());
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.fd.clear_write_ready()?;
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

