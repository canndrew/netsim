use std::net::AddrParseError;
use std::num::ParseIntError;

mod v4;
mod v6;

pub use self::v4::*;
pub use self::v6::*;

quick_error! {
    /// Errors returned by `SubnetV*::from_str`
    #[derive(Debug)]
    pub enum SubnetParseError {
        /// Missing '/' delimiter
        MissingDelimiter {
            description("missing '/' delimiter")
        }
        /// More than one '/' delimiter
        ExtraDelimiter {
            description("more than one '/' delimiter")
        }
        /// error parsing IP address
        ParseAddr(e: AddrParseError) {
            description("error parsing IP address")
            display("error parsing IP address: {}", e)
            cause(e)
        }
        /// error parsing subnet bit count
        ParseBits(e: ParseIntError) {
            description("error parsing subnet bit count")
            display("error parsing subnet bit count: {}", e)
            cause(e)
        }
    }
}

