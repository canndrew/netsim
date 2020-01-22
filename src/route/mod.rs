use crate::priv_prelude::*;

mod v4;
mod v6;

pub use self::v4::*;
pub use self::v6::*;

quick_error! {
    /// Errors returned by `add_route` and `Route::add`
    #[allow(missing_docs)]
    #[derive(Debug)]
    pub enum AddRouteError {
        /// Process file descriptor limit hit
        ProcessFileDescriptorLimit(e: io::Error) {
            description("process file descriptor limit hit")
            display("process file descriptor limit hit ({})", e)
            cause(e)
        }
        /// System file descriptor limit hit
        SystemFileDescriptorLimit(e: io::Error) {
            description("system file descriptor limit hit")
            display("system file descriptor limit hit ({})", e)
            cause(e)
        }
        /// Interface name contains an interior NUL byte
        NameContainsNul {
            description("interface name contains interior NUL byte")
        }
    }
}
