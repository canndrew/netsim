use priv_prelude::*;

pub struct FrameBuffer<C> {
    channel: C,
    buffer: VecDeque<EtherFrame>,
    buffered_now: usize,
    buffered_max: usize,
}

impl<C> Stream for FrameBuffer<C>
where
    C: Stream<Item = EtherFrame, Error = io::Error>
{
    type Item = EtherFrame;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Option<EtherFrame>>> {
        self.channel.poll()
    }
}

impl<C> Sink for FrameBuffer<C>
where
    C: Sink<SinkItem = EtherFrame, SinkError = io::Error>
{
    type SinkItem = EtherFrame;
    type SinkError = io::Error;

    fn start_send(&mut self, item: EtherFrame) -> io::Result<AsyncSink<EtherFrame>> {
        let _ = self.poll_complete()?;
        if self.buffered_max - self.buffered_now < item.len() {
            return Ok(AsyncSink::NotReady(item));
        }
        self.buffered_now += item.len();
        self.buffer.push_back(item);
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> io::Result<Async<()>> {
        loop {
            match self.channel.poll_complete()? {
                Async::Ready(()) => {
                    match self.buffer.pop_front() {
                        Some(frame) => {
                            self.buffered_now -= frame.len();
                            match self.channel.start_send(frame)? {
                                AsyncSink::Ready => continue,
                                AsyncSink::NotReady(frame) => {
                                    // the channel will not accept this frame, even when ready.
                                    drop(frame);
                                    continue;
                                },
                            }
                        },
                        None => return Ok(Async::Ready(())),
                    }
                },
                Async::NotReady => {
                    match self.buffer.pop_front() {
                        Some(frame) => {
                            match self.channel.start_send(frame)? {
                                AsyncSink::Ready => continue,
                                AsyncSink::NotReady(frame) => {
                                    self.buffered_now += frame.len();
                                    self.buffer.push_front(frame);
                                    return Ok(Async::NotReady);
                                },
                            }
                        },
                        None => return Ok(Async::NotReady),
                    }
                },
            }
        }
    }
}

