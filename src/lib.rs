extern crate rand;
extern crate byteorder;
extern crate bytes;

/// Convert a variable-length slice to a fixed-length array
macro_rules! slice_assert_len {
    ($len:tt, $slice:expr) => {{
        use std::ptr;

        union MaybeUninit<T: Copy> {
            init: T,
            uninit: (),
        }
        
        let mut array: MaybeUninit<[_; $len]> = MaybeUninit { uninit: () };
        let slice: &[_] = $slice;
        for i in 0..$len {
            let x = slice[i];
            unsafe {
                ptr::write(&mut array.init[i], x)
            }
        }

        unsafe {
            array.init
        }
    }}
}

mod prelude;
mod priv_prelude;
mod util;
mod checksum;
pub mod mac;
pub mod arp;
pub mod ipv4;
pub mod udp;
pub mod ether;

pub use prelude::*;

