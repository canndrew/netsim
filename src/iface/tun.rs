//! Contains utilites for working with virtual (TUN) network interfaces.

use priv_prelude::*;
use sys;
use iface::build::{IfaceBuilder, build};

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
                ipv4_addr: ipv4!("0.0.0.0"),
                ipv4_netmask: ipv4!("0.0.0.0"),
                ipv4_routes: Vec::new(),
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

    /// Set the interface IPv4 address.
    pub fn ipv4_address(mut self, address: Ipv4Addr) -> Self {
        self.builder.ipv4_addr = address;
        self
    }

    /// Set the interface IPv4 netmask.
    pub fn ipv4_netmask(mut self, netmask: Ipv4Addr) -> Self {
        self.builder.ipv4_netmask = netmask;
        self
    }

    /// Add an IPv4 route to the set of routes that will be created and directed through this interface.
    pub fn ipv4_route(mut self, route: RouteV4) -> Self {
        self.builder.ipv4_routes.push(route);
        self
    }

    /// Consume this `IpIfaceBuilder` and build a `UnboundIpIface`. This creates the TUN device but does not
    /// bind it to a tokio event loop. This is useful if the event loop lives in a different thread
    /// to where you need to create the device. You can send a `UnboundIpIface` to another thread then
    /// `bind` it to create your `IpIface`.
    pub fn build_unbound(self) -> Result<UnboundIpIface, IfaceBuildError> {
        let fd = build(self.builder, false)?;

        trace!("creating TUN");

        Ok(UnboundIpIface { fd })
    }

    /// Consume this `IpIfaceBuilder` and build the TUN interface. The returned `IpIface` object can be
    /// used to read/write ethernet frames from this interface. `handle` is a handle to a tokio
    /// event loop which will be used for reading/writing.
    pub fn build(self, handle: &Handle) -> Result<IpIface, IfaceBuildError> {
        Ok(self.build_unbound()?.bind(handle))
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
    pub fn bind(self, handle: &Handle) -> IpIface {
        let UnboundIpIface { fd } = self;
        let fd = unwrap!(PollEvented::new(fd, handle));
        IpIface { fd }
    }
}

/// A handle to a virtual (TUN) network interface. Can be used to read/write ethernet frames
/// directly to the device.
pub struct IpIface {
    fd: PollEvented<AsyncFd>,
}

impl Stream for IpIface {
    type Item = IpPacket;
    type Error = io::Error;
    
    fn poll(&mut self) -> io::Result<Async<Option<IpPacket>>> {
        loop {
            if let Async::NotReady = self.fd.poll_read() {
                return Ok(Async::NotReady);
            }

            let mut buffer: [u8; sys::ETH_FRAME_LEN as usize] = unsafe {
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
                    self.fd.need_read();
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
            Ok(n) => {
                trace!("wrote {} bytes of TUN data to interface", n);
                assert_eq!(n, item.as_bytes().len());
            },
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

