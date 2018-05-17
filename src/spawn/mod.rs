//! The `spawn` family of functions allows spawning network-isolated threads and virtual networkds
//! which you can read+write raw ethernet or IP data to.
//!
//! `new_namespace` is the most primitive of these functions. It can be used to spawn a thread into
//! a container with no network interfaces. `with_iface` takes network interface parameters as an
//! argument and will automatically set up an ethernet/IP interface in the container for you. Other
//! functions are convenience functions will automatically configure interfaces with common
//! settings.
//!
//! `network_ipv4` can be used to launch a heirarchal network of nodes using the functions in the
//! `node` module.

mod new_namespace;
mod network_ip;
mod network_ipv4;
mod network_ipv6;
mod network_eth;

pub use self::new_namespace::new_namespace;
pub use self::network_ip::network_ip;
pub use self::network_ipv4::network_ipv4;
pub use self::network_ipv6::network_ipv6;
pub use self::network_eth::network_eth;

