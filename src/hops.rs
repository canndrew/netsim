use priv_prelude::*;

pub struct Hops {
    channel: EtherBox,
    sending_frame: Option<EthernetFrame<Bytes>>,
    hops: u8,
}

impl Hops {
    pub fn new<C: EtherChannel + 'static>(
        channel: C,
        hops: u8,
    ) -> Hops {
        Hops {
            channel: Box::new(channel) as EtherBox,
            sending_frame: None,
            hops: hops,
        }
    }

    pub fn process_frame(&mut self, frame: EthernetFrame<Bytes>) -> Option<EthernetFrame<Bytes>> {
        let mut frame = EthernetFrame::new(BytesMut::from(frame.into_inner()));
        match frame.ethertype() {
            EthernetProtocol::Ipv4 => {
                let mut ipv4 = Ipv4Packet::new(frame.payload_mut());
                let hop_limit = ipv4.hop_limit();
                match hop_limit.checked_sub(self.hops) {
                    Some(hop_limit) => ipv4.set_hop_limit(hop_limit),
                    None => return None,
                }
            },
            EthernetProtocol::Ipv6 => {
                let mut ipv6 = Ipv6Packet::new(frame.payload_mut());
                let hop_limit = ipv6.hop_limit();
                match hop_limit.checked_sub(self.hops) {
                    Some(hop_limit) => ipv6.set_hop_limit(hop_limit),
                    None => return None,
                }
            },
            _ => return None,
        };
        let frame = EthernetFrame::new(frame.into_inner().freeze());
        Some(frame)
    }
}

impl Stream for Hops {
    type Item = EthernetFrame<Bytes>;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EthernetFrame<Bytes>>>> {
        loop {
            match self.channel.poll()? {
                Async::Ready(Some(frame)) => {
                    match self.process_frame(frame) {
                        Some(frame) => return Ok(Async::Ready(Some(frame))),
                        None => continue,
                    }
                },
                Async::Ready(None) => return Ok(Async::Ready(None)),
                Async::NotReady => return Ok(Async::NotReady),
            }
        }
    }
}

impl Sink for Hops {
    type SinkItem = EthernetFrame<Bytes>;
    type SinkError = io::Error;

    fn start_send(
        &mut self,
        frame: EthernetFrame<Bytes>,
    ) -> io::Result<AsyncSink<EthernetFrame<Bytes>>> {
        if self.sending_frame.is_some() {
            return Ok(AsyncSink::NotReady(frame));
        }

        self.sending_frame = self.process_frame(frame);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        loop {
            if let Async::NotReady = self.channel.poll_complete()? {
                return Ok(Async::NotReady);
            }

            if let Some(frame) = self.sending_frame.take() {
                match self.channel.start_send(frame)? {
                    AsyncSink::Ready => continue,
                    AsyncSink::NotReady(frame) => {
                        self.sending_frame = Some(frame);
                        return Ok(Async::NotReady);
                    },
                }
            }

            return Ok(Async::Ready(()));
        }
    }
}

