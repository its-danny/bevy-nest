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

        app.add_system_set_to_stage(
            CoreStage::PreUpdate,
            SystemSet::new()
                .label("nest")
                .with_system(handle_incoming)
                .with_system(handle_lost)
                .with_system(handle_events)
                .with_system(handle_inbox),
        );

        app.add_system_set_to_stage(CoreStage::Last, SystemSet::new().with_system(handle_outbox));
    }
}
