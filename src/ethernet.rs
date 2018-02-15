use priv_prelude::*;
use util;
#[cfg(test)]
use rand;

/// Convenience type alias for a boxed stream/sink of ethernet frames.
pub type EtherBox = Box<EtherChannel<
    Item = EthernetFrame<Bytes>,
    Error = io::Error,
    SinkItem = EthernetFrame<Bytes>,
    SinkError = io::Error,
> + 'static>;

/// Trait alias (or at least will be when trait aliases are stable) representing a `Stream`/`Sink`
/// of ethernet frames.
pub trait EtherChannel: Stream<Item=EthernetFrame<Bytes>, Error=io::Error>
                      + Sink<SinkItem=EthernetFrame<Bytes>, SinkError=io::Error>
{
}

impl<T> EtherChannel for T
where
    T: Stream<Item=EthernetFrame<Bytes>, Error=io::Error>
       + Sink<SinkItem=EthernetFrame<Bytes>, SinkError=io::Error>
       + Sized,
{
}

pub trait EthernetFrameExt {
    fn new_ipv4<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        ipv4: &Ipv4Packet<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>;

    fn new_ipv6<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        ipv6: &Ipv6Packet<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>;

    fn new_arp<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        arp: &ArpPacket<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>;
}

impl EthernetFrameExt for EthernetFrame<Bytes> {
    fn new_ipv4<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        ipv4: &Ipv4Packet<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>
    {
        let bytes = ipv4.as_ref();

        let frame_repr = EthernetRepr {
            src_addr: src_addr,
            dst_addr: dst_addr,
            ethertype: EthernetProtocol::Ipv4,
        };

        let len = bytes.len() + frame_repr.buffer_len();
        let mut frame = EthernetFrame::new(util::bytes_mut_zeroed(len));
        frame_repr.emit(&mut frame);
        frame.payload_mut().clone_from_slice(bytes);
        EthernetFrame::new(frame.into_inner().freeze())
    }

    fn new_arp<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        arp: &ArpPacket<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>
    {
        let bytes = arp.as_ref();

        let frame_repr = EthernetRepr {
            src_addr: src_addr,
            dst_addr: dst_addr,
            ethertype: EthernetProtocol::Arp,
        };

        let len = bytes.len() + frame_repr.buffer_len();
        let mut frame = EthernetFrame::new(util::bytes_mut_zeroed(len));
        frame_repr.emit(&mut frame);
        frame.payload_mut().clone_from_slice(bytes);
        EthernetFrame::new(frame.into_inner().freeze())
    }

    fn new_ipv6<T>(
        src_addr: EthernetAddress,
        dst_addr: EthernetAddress,
        ipv6: &Ipv6Packet<T>,
    ) -> EthernetFrame<Bytes>
    where
        T: AsRef<[u8]>
    {
        let bytes = ipv6.as_ref();

        let frame_repr = EthernetRepr {
            src_addr: src_addr,
            dst_addr: dst_addr,
            ethertype: EthernetProtocol::Ipv6,
        };

        let len = bytes.len() + frame_repr.buffer_len();
        let mut frame = EthernetFrame::new(util::bytes_mut_zeroed(len));
        frame_repr.emit(&mut frame);
        frame.payload_mut().clone_from_slice(bytes);
        EthernetFrame::new(frame.into_inner().freeze())
    }
}

#[cfg(test)]
pub fn respond_to_arp<C: EtherChannel + 'static>(
    channel: C,
    ip: Ipv4Addr,
    mac: EthernetAddress,
) -> IoFuture<C> {
    channel
    .into_future()
    .map_err(|(e, _channel)| e)
    .and_then(move |(frame_opt, channel)| {
        let frame = unwrap!(frame_opt);
        assert_eq!(frame.ethertype(), EthernetProtocol::Arp);
        let arp = {
            let src_mac = frame.src_addr();
            let frame_ref = EthernetFrame::new(frame.as_ref());
            let arp = ArpPacket::new(frame_ref.payload());
            assert_eq!(arp.operation(), ArpOperation::Request);
            assert_eq!(arp.source_hardware_addr(), src_mac.as_bytes());
            let src_ip = Ipv4Addr::from(assert_len!(4, arp.source_protocol_addr()));
            assert_eq!(arp.target_hardware_addr(), &[0, 0, 0, 0, 0, 0]);
            assert_eq!(arp.target_protocol_addr(), &ip.octets());
            ArpPacket::new_reply(
                mac,
                ip,
                src_mac,
                src_ip,
            )
        };
        let frame = EthernetFrame::new_arp(
            mac,
            frame.src_addr(),
            &arp,
        );

        channel
        .send(frame)
    })
    .into_boxed()
}

#[cfg(test)]
pub fn random_mac() -> EthernetAddress {
    let mut bytes: [u8; 6] = rand::random();
    bytes[0] &= 0xfc;
    EthernetAddress::from_bytes(&bytes)
}

