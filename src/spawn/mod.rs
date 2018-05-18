//! The `spawn` family of functions allows spawning network-isolated threads and virtual networkds
//! which you can read+write raw ethernet or IP data to.
//!
//! `new_namespace` is the most primitive of these functions. It can be used to spawn a thread into
//! a container with no network interfaces. `with_iface` takes network interface parameters as an
//! argument and will automatically set up an ethernet/IP interface in the container for you. Other
//! functions are convenience functions will automatically configure interfaces with common
//! settings.
//!
//! The tree functions can be used to launch a heirarchal network of nodes when used in
//! conjunction with the functions in the `node` module

mod new_namespace;
mod tree;

pub use self::new_namespace::new_namespace;
pub use self::tree::*;

