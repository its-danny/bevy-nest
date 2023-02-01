use std::time::Duration;

use bevy::log::LogPlugin;
use bevy::{app::ScheduleRunnerSettings, prelude::*};
use bevy_nest::prelude::*;

#[derive(Component)]
struct Player(ClientId);

#[derive(Resource)]
struct WhoTimer(Timer);

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

                outbox.send_command(*id, vec![IAC, WILL, GMCP]);

                for (_, player) in players.iter() {
                    outbox.send_text(player.0, format!("{id:?} connected"));
                }
            }
            NetworkEvent::Disconnected(id) => {
                if let Some((entity, _)) = players.iter().find(|(_, c)| c.0 == *id) {
                    commands.entity(entity).despawn();

                    for (_, player) in players.iter() {
                        outbox.send_text(player.0, format!("{id:?} disconnected"));
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
        if let Message::Text(text) = &message.content {
            for (_, player) in players.iter() {
                outbox.send_text(player.0, format!("{:?}: {text}", message.from));
            }
        }
    }
}

struct OnlineCount(usize);

impl From<OnlineCount> for Payload {
    fn from(online_count: OnlineCount) -> Self {
        Payload {
            package: "chat".into(),
            subpackage: Some("who".into()),
            data: Some(online_count.0.to_string()),
        }
    }
}

fn who_online(
    time: Res<Time>,
    mut who_timer: ResMut<WhoTimer>,
    mut outbox: EventWriter<Outbox>,
    players: Query<&Player>,
) {
    if who_timer.0.tick(time.delta()).just_finished() {
        for player in players.iter() {
            outbox.send_gmcp(player.0, OnlineCount(players.iter().len()).into());
        }
    }
}

fn main() {
    App::new()
        .insert_resource(ScheduleRunnerSettings::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .insert_resource(WhoTimer(Timer::new(
            Duration::from_secs(3),
            TimerMode::Repeating,
        )))
        .add_plugins(MinimalPlugins)
        .add_plugin(LogPlugin {
            ..Default::default()
        })
        .add_plugin(NestPlugin)
        .add_startup_system(setup_network)
        .add_system(handle_events)
        .add_system(handle_messages)
        .add_system(who_online)
        .run();
}
