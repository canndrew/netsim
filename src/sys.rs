#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(unused)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub fn errno() -> ::std::os::raw::c_int {
    unsafe {
        *__errno_location()
    }
}

