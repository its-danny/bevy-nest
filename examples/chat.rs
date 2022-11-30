use std::time::Duration;

use bevy::log::LogPlugin;
use bevy::{app::ScheduleRunnerSettings, prelude::*};
use bevy_nest::prelude::*;

#[derive(Component)]
struct Player(ClientId);

fn setup_network(server: Res<Server>) {
    server.listen("127.0.0.1:3000");
}

fn handle_events(
    mut commands: Commands,
    mut events: EventReader<NetworkEvent>,
    mut outbox: EventWriter<Outbox>,
    players: Query<(Entity, &Player)>,
) {
    for event in events.iter() {
        match event {
            NetworkEvent::Connected(id) => {
                commands.spawn(Player(*id));

                for (_, player) in players.iter() {
                    outbox.send(Outbox {
                        to: player.0,
                        content: Message::Text(format!("Player {:?} connected", id)),
                    });
                }
            }
            NetworkEvent::Disconnected(id) => {
                if let Some((entity, _)) = players.iter().find(|(_, c)| c.0 == *id) {
                    commands.entity(entity).despawn();

                    for (_, player) in players.iter() {
                        outbox.send(Outbox {
                            to: player.0,
                            content: Message::Text(format!("Player {:?} disconnected", id)),
                        });
                    }
                }
            }
            NetworkEvent::Error(error) => {
                error!("Network Error: {error:?}");
            }
        }
    }
}

fn handle_messages(
    mut inbox: EventReader<Inbox>,
    mut outbox: EventWriter<Outbox>,
    players: Query<(Entity, &Player)>,
) {
    for message in inbox.iter() {
        match &message.content {
            Message::Text(text) => {
                for (_, player) in players.iter() {
                    outbox.send(Outbox {
                        to: player.0,
                        content: Message::Text(format!("Player {:?}: {:?}", player.0, text)),
                    });
                }
            }
            _ => {}
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ScheduleRunnerSettings::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin {
            ..Default::default()
        })
        .add_plugin(NestPlugin)
        .add_startup_system(setup_network)
        .add_system(handle_events)
        .add_system(handle_messages)
        .run();
}
