use crate::priv_prelude::*;

pub trait BytesMutExt {
    unsafe fn uninit(len: usize) -> BytesMut;
}

impl BytesMutExt for BytesMut {
    unsafe fn uninit(len: usize) -> BytesMut {
        let mut ret = BytesMut::with_capacity(len);
        ret.set_len(len);
        ret
    }
}

