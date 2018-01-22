use priv_prelude::*;

struct Client {
    channel: EtherBox,
    outgoing: VecDeque<EtherFrame>,
}

pub struct Hub {
    clients: Vec<Client>,
}

impl Hub {
    pub fn new() -> Hub {
        Hub {
            clients: Vec::new(),
        }
    }

    pub fn add_client<E: EtherChannel + 'static>(&mut self, client: E) {
        self.clients.push(Client {
            channel: Box::new(client),
            outgoing: VecDeque::new(),
        });
    }
}

impl Future for Hub {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<()>> {
        println!("polling hub");
        let mut i = 0;
        while i < self.clients.len() {
            match self.clients[i].channel.poll()? {
                Async::Ready(Some(frame)) => {
                    println!("hub received frame: {:?}", frame);
                    for client in &mut self.clients {
                        client.outgoing.push_back(frame.clone());
                    }
                },
                Async::Ready(None) => {
                    println!("hub removing device #{}", i);
                    self.clients.swap_remove(i);
                },
                Async::NotReady => {
                    i += 1;
                },
            }
        }

        for client in &mut self.clients {
            loop {
                let frame = match client.outgoing.pop_front() {
                    Some(frame) => frame,
                    None => break,
                };
                match client.channel.start_send(frame)? {
                    AsyncSink::Ready => (),
                    AsyncSink::NotReady(frame) => {
                        client.outgoing.push_front(frame);
                        break;
                    },
                }
            }
            let _ = client.channel.poll_complete()?;
        }

        if self.clients.is_empty() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }
}

