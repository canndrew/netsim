//! The `spawn` family of functions allows spawning network-isolated threads and virtual networkds
//! which you can read+write raw ethernet or IP data to.
//!
//! `new_namespace` is the most primitive of these functions. It can be used to spawn a thread into
//! a container with no network interfaces. `with_iface` takes network interface parameters as an
//! argument and will automatically set up an ethernet/IP interface in the container for you. Other
//! functions are convenience functions will automatically configure interfaces with common
//! settings.
//!
//! `network_v4` can be used to launch a heirarchal network of nodes using the functions in the
//! `node` module.

mod new_namespace;
mod with_ether_iface;
mod with_ipv4_iface;
mod on_subnet_v4;
mod on_internet_v4;
mod behind_nat_v4;
mod network_v4;

pub use self::new_namespace::new_namespace;
pub use self::with_ether_iface::with_ether_iface;
pub use self::with_ipv4_iface::with_ipv4_iface;
pub use self::on_subnet_v4::on_subnet_v4;
pub use self::on_internet_v4::on_internet_v4;
pub use self::behind_nat_v4::behind_nat_v4;
pub use self::network_v4::network_v4;

