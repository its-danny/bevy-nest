use crossbeam_channel::{Receiver, Sender, unbounded};

pub(crate) struct Channel<T> {
    pub(crate) sender: Sender<T>,
    pub(crate) receiver: Receiver<T>,
}

impl<T> Channel<T> {
    pub(crate) fn new() -> Self {
        let (sender, receiver) = unbounded();

        Self { sender, receiver }
    }
}
