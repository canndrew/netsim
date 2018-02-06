use priv_prelude::*;
use rand;
use rand::distributions::range::Range;

pub fn expovariant_rand() -> f32 {
    let range = Range::new(0.0, 1.0);
    let mut rng = rand::thread_rng();
    let offset = range.ind_sample(&mut rng);
    -f32::ln(1.0 - offset)
}

#[cfg(test)]
pub fn random_vec(len: usize) -> Vec<u8> {
    let mut ret = Vec::with_capacity(len);
    unsafe {
        ret.set_len(len);
    }
    rand::thread_rng().fill_bytes(&mut ret[..]);
    ret
}

pub fn bytes_mut_zeroed(len: usize) -> BytesMut {
    let mut ret = BytesMut::with_capacity(len);
    unsafe {
        ret.set_len(len);
        for i in 0..len {
            ret[i] = 0;
        }
    }
    ret
}

