use bevy::prelude::*;

use crate::{
    events::{Inbox, NetworkEvent, Outbox},
    server::Server,
    systems::{handle_events, handle_inbox, handle_incoming, handle_lost, handle_outbox},
};

pub struct NestPlugin;

impl Plugin for NestPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Server::new());

        app.add_message::<NetworkEvent>();
        app.add_message::<Inbox>();
        app.add_message::<Outbox>();

        app.add_systems(
            PreUpdate,
            (handle_incoming, handle_lost, handle_events, handle_inbox),
        );

        app.add_systems(Last, handle_outbox);
    }
}
