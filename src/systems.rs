use crate::{
    events::{Inbox, NetworkEvent, Outbox},
    server::Server,
};
use bevy::prelude::*;

// Retrieve incoming connections from the server and spawn tasks to handle them.
pub(crate) fn handle_incoming(server: Res<Server>) {
    for connection in server.incoming.receiver.try_iter() {
        info!("Handling incoming connection: {connection:?}");

        server.setup_client(connection);
    }
}

// Retrieve lost clients from the server and remove them from the client list.
pub(crate) fn handle_lost(server: Res<Server>) {
    for id in server.lost.receiver.try_iter() {
        info!("Handling lost connection: {id:?}");

        server.remove_client(&id);
    }
}

// Retrieve events from the server and send them to Bevy.
pub(crate) fn handle_events(server: Res<Server>, mut events: MessageWriter<NetworkEvent>) {
    for event in server.events.receiver.try_iter() {
        info!("Handling event: {event:?}");

        events.write(event);
    }
}

// Retrieve frames from the server and send them to Bevy.
pub(crate) fn handle_inbox(server: Res<Server>, mut inbox: MessageWriter<Inbox>) {
    for frame in server.inbox.receiver.try_iter() {
        info!("Handling inbox frame: {frame:?}");

        inbox.write(frame);
    }
}

// Retrieve frames from Bevy and send them to the server.
pub(crate) fn handle_outbox(server: Res<Server>, mut outbox: MessageReader<Outbox>) {
    for out in outbox.read() {
        server.send(out);
    }
}
