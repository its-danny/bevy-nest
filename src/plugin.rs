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

        app.add_event::<NetworkEvent>();
        app.add_event::<Inbox>();
        app.add_event::<Outbox>();

        app.add_systems(
            (handle_incoming, handle_lost, handle_events, handle_inbox)
                .in_base_set(CoreSet::PreUpdate),
        );

        app.add_system(handle_outbox.in_base_set(CoreSet::Last));
    }
}
