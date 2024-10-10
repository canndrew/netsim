use crate::priv_prelude::*;

struct BuildConfig {
    name_opt: Option<String>,
    ipv4_addr_subnet_opt: Option<(Ipv4Addr, u8)>,
    ipv4_routes: Vec<(Ipv4Network, Option<Ipv4Addr>)>,
    ipv6_addr_subnet_opt: Option<(Ipv6Addr, u8)>,
    ipv6_routes: Vec<(Ipv6Network, Ipv6Addr)>,
}

/// Builder for adding an IP interface to a [`Machine`](crate::Machine).
///
/// Once you're done configuring the interface you must `await` the builder to actually add the
/// interface.
pub struct IpIfaceBuilder<'m> {
    machine: &'m Machine,
    build_config: BuildConfig,
}

impl IpIfaceBuilder<'_> {
    pub(crate) fn new(machine: &Machine) -> IpIfaceBuilder<'_> {
        IpIfaceBuilder {
            machine,
            build_config: BuildConfig {
                name_opt: None,
                ipv4_addr_subnet_opt: None,
                ipv4_routes: Vec::new(),
                ipv6_addr_subnet_opt: None,
                ipv6_routes: Vec::new(),
            },
        }
    }

    /// Sets the interface name. Defaults to "netsim" if not set.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.build_config.name_opt = Some(name.into());
        self
    }

    /// Sets the interface's IPv4 address.
    pub fn ipv4_addr(mut self, ipv4_addr: impl Into<Ipv4Addr>) -> Self {
        let ipv4_addr = ipv4_addr.into();
        let network = Ipv4Network::infer_from_addr(ipv4_addr);
        self.build_config.ipv4_addr_subnet_opt = Some((ipv4_addr, network.subnet_mask_bits()));
        self
    }

    /// Sets the interface's IPv6 address.
    pub fn ipv6_addr(mut self, ipv6_addr: impl Into<Ipv6Addr>) -> Self {
        let ipv6_addr = ipv6_addr.into();
        let network = Ipv6Network::infer_from_addr(ipv6_addr);
        self.build_config.ipv6_addr_subnet_opt = Some((ipv6_addr, network.subnet_mask_bits()));
        self
    }

    /// Adds an IPv4 route for this interface to the machine's routing table.
    pub fn ipv4_route(mut self, destination: Ipv4Network) -> Self {
        self.build_config.ipv4_routes.push((destination, None));
        self
    }

    /// Adds an IPv4 route to the machine's routing table which forwards through this interface via
    /// the specified gateway.
    pub fn ipv4_route_with_gateway<A: Into<Ipv4Addr>>(
        mut self,
        destination: Ipv4Network,
        gateway: A,
    ) -> Self {
        let gateway = gateway.into();
        self.build_config.ipv4_routes.push((destination, Some(gateway)));
        self
    }

    /// Adds an IPv4 route for this interface to the machine's routing table and sets it as the
    /// default route.
    pub fn ipv4_default_route(self) -> Self {
        self.ipv4_route(Ipv4Network::new(Ipv4Addr::UNSPECIFIED, 0))
    }

    /// Adds an IPv4 route to the machine's routing table which forwards through this interface via
    /// the specified gateway. Sets it as the default route.
    pub fn ipv4_default_route_with_gateway(self, gateway: Ipv4Addr) -> Self {
        self.ipv4_route_with_gateway(Ipv4Network::new(Ipv4Addr::UNSPECIFIED, 0), gateway)
    }

    /// Adds an IPv6 route for this interface to the machine's routing table.
    pub fn ipv6_route(mut self, destination: Ipv6Network, next_hop: Ipv6Addr) -> Self {
        self.build_config.ipv6_routes.push((destination, next_hop));
        self
    }

    /// Adds an IPv6 route for this interface to the machine's routing table and sets it as the
    /// default route.
    pub fn ipv6_default_route(mut self, next_hop: Ipv6Addr) -> Self {
        self.build_config.ipv6_routes.push((Ipv6Network::new(Ipv6Addr::UNSPECIFIED, 0), next_hop));
        self
    }

    /// Builds the interface. The returned [`IpIface`](crate::IpIface) can be used to send packets
    /// to or receive packets from this interface.
    pub async fn build(self) -> io::Result<IpIface> {
        self.into_future().await
    }
}

impl<'m> IntoFuture for IpIfaceBuilder<'m> {
    type Output = io::Result<IpIface>;
    type IntoFuture = Pin<Box<dyn Future<Output = io::Result<IpIface>> + Send + 'm>>;

    fn into_future(self) -> Pin<Box<dyn Future<Output = io::Result<IpIface>> + Send + 'm>> {
        let IpIfaceBuilder { machine, build_config } = self;
        Box::pin(async move {
            let task = async move {
                create_tun_interface(build_config)
            };
            let res = machine.spawn(task).await;
            let fd = match res {
                Ok(res_opt) => res_opt.unwrap()?,
                Err(err) => panic::resume_unwind(err),
            };
            IpIface::new(fd)
        })
    }
}

fn create_tun_interface(build_config: BuildConfig) -> io::Result<OwnedFd> {
    let BuildConfig {
        name_opt,
        ipv4_addr_subnet_opt,
        ipv4_routes,
        ipv6_addr_subnet_opt,
        ipv6_routes,
    } = build_config;
    let name = name_opt.as_deref().unwrap_or("netsim");
    let name_cstr = match CString::new(name) {
        Ok(name_cstr) => name_cstr,
        Err(err) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, err));
        },
    };

    #[allow(clippy::unnecessary_cast)]
    if name_cstr.as_bytes_with_nul().len() > libc::IF_NAMESIZE as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "name too long"));
    }

    let fd = {
        let raw_fd = unsafe {
            libc::open(c"/dev/net/tun".as_ptr() as *const _, libc::O_RDWR)
        };
        if raw_fd < 0 {
            let err = io::Error::last_os_error();
            return Err(io::Error::new(err.kind(), "opening /dev/net/tun"));
        }
        unsafe {
            OwnedFd::from_raw_fd(raw_fd)
        }
    };
    let flags = unsafe {
        libc::fcntl(fd.as_raw_fd(), libc::F_GETFL, 0)
    };
    if flags < 0 {
        let err = io::Error::last_os_error();
        return Err(io::Error::new(err.kind(), "calling fcntl(F_GETFL) on /dev/net/tun"));
    }
    let res = unsafe {
        libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK)
    };
    if res < 0 {
        let err = io::Error::last_os_error();
        return Err(io::Error::new(err.kind(), "calling fcntl(F_SETFL) on /dev/net/tun"));
    }
    let mut req = unsafe {
        let mut req: libc::ifreq = mem::zeroed();
        ptr::copy_nonoverlapping(
            name_cstr.as_ptr(),
            req.ifr_name.as_mut_ptr(),
            name_cstr.as_bytes().len(),
        );
        req.ifr_ifru.ifru_flags = libc::IFF_NO_PI as i16;
        req.ifr_ifru.ifru_flags |= libc::IFF_TUN as i16;
        req
    };
    let res = unsafe {
        ioctl::tunsetiff(fd.as_raw_fd(), &mut req as *mut _ as *mut _)
    };
    if res < 0 {
        let err = io::Error::last_os_error();
        return Err(io::Error::new(err.kind(), "calling ioctl(TUNSETIFF) failed"));
    };
    let real_name = {
        let name = &req.ifr_name[..];
        let name = match name.iter().position(|b| *b == 0) {
            Some(p) => &name[..p],
            None => name,
        };
        let name = unsafe {
            slice::from_raw_parts(name.as_ptr() as *const _, name.len())
        };
        let name = match std::str::from_utf8(name) {
            Ok(name) => name,
            Err(err) => {
                return Err(io::Error::new(io::ErrorKind::InvalidData, err));
            },
        };
        name.to_owned()
    };


    if let Some((ipv4_addr, subnet_mask_bits)) = ipv4_addr_subnet_opt {
        iface::configure::set_ipv4_addr(&real_name, ipv4_addr, subnet_mask_bits)?;
    }
    if let Some((ipv6_addr, subnet_mask_bits)) = ipv6_addr_subnet_opt {
        iface::configure::set_ipv6_addr(&real_name, ipv6_addr, subnet_mask_bits)?;
    }

    iface::configure::put_up(&real_name)?;

    for (destination, gateway_opt) in ipv4_routes {
        iface::configure::add_ipv4_route(&real_name, destination, gateway_opt)?;
    }
    for (destination, next_hop) in ipv6_routes {
        iface::configure::add_ipv6_route(&real_name, destination, next_hop)?;
    }

    Ok(fd)
}

