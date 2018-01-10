pub struct Hub {
    taps: Vec<Tap>,
    state: HubState,
}

enum HubState {
    Reading {
        index: usize,
    },
    Writing {
        index: usize,
        frame: EtherFrame,
    },
    Finished,
}

impl Hub {
    pub fn new(handle: &Handle) -> Hub {
    }
}

impl Future for Hub {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Result<Async<()>, Void> {
        /*
        loop {
            match self.rx.poll().void_unwrap() {
                Async::Ready(Some(tap)) => self.taps.push(tap),
                Async::Ready(None) => return Ok(Async::Ready(())),
                Async::NotReady => break,
            }
        }
        */

        let state = mem::swap(&mut self.state, HubState::Finished);
        match state {

        }

        loop {
            if let Some(frame) = self.frame.take() {
                for tap in &mut self.taps {

                }
            }
        }

        let mut i = 0;
        while i < self.taps.len() {
            match self.taps[i].poll()? {
                Async::Ready(Some(frame)) => {
                    for tap in &mut self.taps {
                        tap.send(
                    }
                },
            }
        }
    }
}

