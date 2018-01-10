struct RouterClient {
    client: Box<EtherChannel>,
    mac: MacAddr,
    routes: Vec<RouteV4>,
}

pub struct Router {
    clients: Vec<Box<EtherChannel>>,
    frame_buffer: VecDeque<EtherFrame>,

}

impl Router {
    pub fn new() -> Router {
        Router {
            clients: Vec::new(),
        }
    }

    pub fn add(&mut self, client: Box<EtherChannel>) {
        clients.push(client);
    }
}

impl Future for Router {
    type Item = Void;
    type Error = io::Error;

    fn poll(&mut self) -> io::Result<Async<Void>> {
        let mut incoming = Vec::new();
        let mut i = 0;
        while i < self.clients.len() {
            match self.clients[i].poll()? {
                Async::Ready(Some(frame)) => {
                    incoming.push(frame);
                    continue;
                },
                Async::Ready(None) => {
                    self.clients.swap_remove(i);
                    continue;
                },
                Async::NotReady => (),
            }
            i += 1;
        }
    }
}


