use priv_prelude::*;

enum Disconnect<D> {
    Connected(D),
    Waiting(Timeout),
    Finished,
}

pub struct WithDisconnect<C, D> {
    channel: C,
    disconnect: Disconnect<D>,
    handle: Handle,
}

impl<C, D> WithDisconnect<C, D> {
    pub fn new(channel: C, disconnect: D, handle: &Handle) -> WithDisconnect<C, D> {
        let disconnect = Disconnect::Connected(disconnect);
        let handle = handle.clone();
        WithDisconnect {
            channel,
            disconnect,
            handle,
        }
    }
}

impl<C, D> Stream for WithDisconnect<C, D>
where
    C: Stream<Item = EthernetFrame<Bytes>, Error = io::Error>,
    D: Future<Item = (), Error = Void>,
{
    type Item = EthernetFrame<Bytes>;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EthernetFrame<Bytes>>>> {
        let disconnect = mem::replace(&mut self.disconnect, Disconnect::Finished);

        match disconnect {
            Disconnect::Connected(mut d) => {
                match self.channel.poll()? {
                    Async::Ready(x) => {
                        self.disconnect = Disconnect::Connected(d);
                        Ok(Async::Ready(x))
                    },
                    Async::NotReady => {
                        match d.poll().void_unwrap() {
                            Async::Ready(()) => {
                                let mut timeout = Timeout::new(Duration::from_millis(500), &self.handle);
                                let _ = timeout.poll().void_unwrap();
                                self.disconnect = Disconnect::Waiting(timeout);
                            },
                            Async::NotReady => {
                                self.disconnect = Disconnect::Connected(d);
                            },
                        }
                        Ok(Async::NotReady)
                    },
                }
            },
            Disconnect::Waiting(mut timeout) => {
                match self.channel.poll()? {
                    Async::Ready(x) => {
                        timeout.reset(Instant::now() + Duration::from_millis(500));
                        self.disconnect = Disconnect::Waiting(timeout);
                        Ok(Async::Ready(x))
                    },
                    Async::NotReady => {
                        match timeout.poll().void_unwrap() {
                            Async::Ready(()) => {
                                Ok(Async::Ready(None))
                            },
                            Async::NotReady => {
                                self.disconnect = Disconnect::Waiting(timeout);
                                Ok(Async::NotReady)
                            },
                        }
                    },
                }
            }
            Disconnect::Finished => Ok(Async::Ready(None)),
        }
    }
}

impl<C, D> Sink for WithDisconnect<C, D>
where
    C: Sink<SinkItem = EthernetFrame<Bytes>, SinkError = io::Error>,
    D: Future<Item = (), Error = Void>,
{
    type SinkItem = EthernetFrame<Bytes>;
    type SinkError = io::Error;

    fn start_send(&mut self, item: EthernetFrame<Bytes>) -> io::Result<AsyncSink<EthernetFrame<Bytes>>> {
        match self.disconnect {
            Disconnect::Connected(..) => {
                self.channel.start_send(item)
            }
            _ => Ok(AsyncSink::Ready)
        }
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        match self.disconnect {
            Disconnect::Connected(..) => {
                self.channel.poll_complete()
            }
            _ => Ok(Async::Ready(()))
        }
    }
}

