// Note: this file was ripped-off from smoltcp

use crate::priv_prelude::*;

fn propagate_carries(word: u32) -> u16 {
    let sum = (word >> 16) + (word & 0xffff);
    ((sum >> 16) as u16) + (sum as u16)
}

/// Compute an RFC 1071 compliant checksum (without the final complement).
pub fn data(mut data: &[u8]) -> u16 {
    let mut accum = 0;

    // For each 32-byte chunk...
    const CHUNK_SIZE: usize = 32;
    while data.len() >= CHUNK_SIZE {
        let mut d = &data[..CHUNK_SIZE];
        // ... take by 2 bytes and sum them.
        while d.len() >= 2 {
            accum += u32::from(NetworkEndian::read_u16(d));
            d = &d[2..];
        }

        data = &data[CHUNK_SIZE..];
    }

    // Sum the rest that does not fit the last 32-byte chunk,
    // taking by 2 bytes.
    while data.len() >= 2 {
        accum += u32::from(NetworkEndian::read_u16(data));
        data = &data[2..];
    }

    // Add the last remaining odd byte, if any.
    if let Some(&value) = data.first() {
        accum += u32::from(value) << 8;
    }

    propagate_carries(accum)
}

/// Combine several RFC 1071 compliant checksums.
pub fn combine(checksums: &[u16]) -> u16 {
    let mut accum: u32 = 0;
    for &word in checksums {
        accum += u32::from(word);
    }
    propagate_carries(accum)
}

/// Compute an IP pseudo header checksum.
pub fn pseudo_header_ipv4(
    source_ip: Ipv4Addr,
    dest_ip: Ipv4Addr,
    protocol: u8,
    length: u32,
) -> u16 {
    let mut proto_len = [0u8; 4];
    proto_len[1] = protocol;
    NetworkEndian::write_u16(&mut proto_len[2..4], length as u16);

    combine(&[
        data(&source_ip.octets()),
        data(&dest_ip.octets()),
        data(&proto_len[..]),
    ])
}

/// Compute an IP pseudo header checksum.
pub fn pseudo_header_ipv6(
    source_ip: Ipv6Addr,
    dest_ip: Ipv6Addr,
    protocol: u8,
    length: u32,
) -> u16 {
    let mut proto_len = [0u8; 8];
    proto_len[7] = protocol;
    NetworkEndian::write_u32(&mut proto_len[0..4], length);
    combine(&[
        data(&source_ip.octets()),
        data(&dest_ip.octets()),
        data(&proto_len[..]),
    ])
}
