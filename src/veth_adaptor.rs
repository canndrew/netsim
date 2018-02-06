use priv_prelude::*;
use rand;

pub struct VethAdaptorV4 {
    channel: EtherBox,
    veth: VethV4,
    sending_frame: Option<EtherFrame>,
}

impl VethAdaptorV4 {
    pub fn new<C: EtherChannel + 'static>(ip_addr: Ipv4Addr, channel: C) -> VethAdaptorV4 {
        VethAdaptorV4 {
            channel: Box::new(channel) as EtherBox,
            veth: VethV4::new(EthernetAddress(rand::random()), ip_addr),
            sending_frame: None,
        }
    }

    pub fn add_route(&mut self, route: RouteV4) {
        self.veth.add_route(route);
    }
}

impl Stream for VethAdaptorV4 {
    type Item = Ipv4Packet<Bytes>;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<Ipv4Packet<Bytes>>>> {
        let disconnected = loop {
            match self.channel.poll()? {
                Async::Ready(Some(frame)) => {
                    trace!("adaptor got frame: {:?}", frame);
                    self.veth.recv_frame(frame);
                },
                Async::Ready(None) => break true,
                Async::NotReady => break false,
            };
        };

        let ret = match self.veth.next_incoming() {
            Async::Ready(packet) => Ok(Async::Ready(Some(packet))),
            Async::NotReady => {
                if disconnected {
                    Ok(Async::Ready(None))
                } else {
                    Ok(Async::NotReady)
                }
            },
        };

        let _ = self.poll_complete()?;
        ret
    }
}

impl Sink for VethAdaptorV4 {
    type SinkItem = Ipv4Packet<Bytes>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Ipv4Packet<Bytes>) -> io::Result<AsyncSink<Ipv4Packet<Bytes>>> {
        trace!("adaptor sending packet: {:?}", item);
        self.veth.send_packet(item);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        trace!("VethAdapatorV4::poll_complete()");
        let complete = loop {
            if let Some(frame) = self.sending_frame.take() {
                trace!("adaptor sending frame: {:?}", frame);
                match self.channel.start_send(frame)? {
                    AsyncSink::Ready => (),
                    AsyncSink::NotReady(frame) => {
                        self.sending_frame = Some(frame);
                        break false;
                    },
                }
            };
            
            let frame = match self.veth.next_outgoing() {
                Async::Ready(frame) => frame,
                Async::NotReady => break true,
            };

            self.sending_frame = Some(frame);
        };

        let complete = match self.channel.poll_complete()? {
            Async::Ready(()) => complete,
            Async::NotReady => false,
        };

        if complete {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

