use priv_prelude::*;

pub struct WithDisconnect<C, D> {
    channel: C,
    disconnect: Option<D>,
}

impl<C, D> WithDisconnect<C, D> {
    pub fn new(channel: C, disconnect: D) -> WithDisconnect<C, D> {
        let disconnect = Some(disconnect);
        WithDisconnect {
            channel,
            disconnect,
        }
    }
}

impl<C, D> Stream for WithDisconnect<C, D>
where
    C: Stream<Item = EtherFrame, Error = io::Error>,
    D: Future<Item = (), Error = Void>,
{
    type Item = EtherFrame;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        if let Some(mut disconnect) = self.disconnect.take() {
            match self.channel.poll()? {
                Async::Ready(x) => {
                    self.disconnect = Some(disconnect);
                    Ok(Async::Ready(x))
                },
                Async::NotReady => {
                    match disconnect.poll().void_unwrap() {
                        Async::Ready(()) => {
                            Ok(Async::Ready(None))
                        },
                        Async::NotReady => {
                            self.disconnect = Some(disconnect);
                            Ok(Async::NotReady)
                        },
                    }
                },
            }
        } else {
            Ok(Async::Ready(None))
        }
    }
}

impl<C, D> Sink for WithDisconnect<C, D>
where
    C: Sink<SinkItem = EtherFrame, SinkError = io::Error>,
    D: Future<Item = (), Error = Void>,
{
    type SinkItem = EtherFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        if let Some(mut disconnect) = self.disconnect.take() {
            match disconnect.poll().void_unwrap() {
                Async::Ready(()) => {
                    Ok(AsyncSink::Ready)
                },
                Async::NotReady => {
                    self.disconnect = Some(disconnect);
                    self.channel.start_send(item)
                },
            }
        } else {
            Ok(AsyncSink::Ready)
        }
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        if let Some(mut disconnect) = self.disconnect.take() {
            match disconnect.poll().void_unwrap() {
                Async::Ready(()) => {
                    Ok(Async::Ready(()))
                },
                Async::NotReady => {
                    self.disconnect = Some(disconnect);
                    self.channel.poll_complete()
                },
            }
        } else {
            Ok(Async::Ready(()))
        }
    }
}

