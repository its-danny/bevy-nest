use tokio::net::TcpStream;

use crate::errors::NetworkError;
use crate::server::ClientId;

#[derive(Debug)]
pub(crate) struct IncomingConnection {
    pub(crate) socket: TcpStream,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Connected(ClientId),
    Disconnected(ClientId),
    Error(NetworkError),
}

#[derive(Debug)]
pub enum Message {
    /// Just your regular text message. This is appended with a newline when sent.
    Text(String),
    /// A command is a sequence of bytes used by the telnet protocol.
    /// See: https://users.cs.cf.ac.uk/Dave.Marshall/Internet/node141.html
    Command(Vec<u8>),
}

/// Message sent from a client. These are iterated over each
/// update and sent to Bevy via [`Event<Inbox>`](bevy::ecs::event::Event) to be read over.
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_nest::prelude::*;
///
/// fn read_inbox(mut inbox: EventReader<Inbox>) {
///     for message in inbox.iter() {
///         // ...
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Inbox {
    pub from: ClientId,
    pub content: Message,
}

/// Message sent to a client. These are iterated over each
/// update by the server and sent to the client's socket.
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_nest::prelude::*;
///
/// fn ping_pong(mut inbox: EventReader<Inbox>, mut outbox: EventWriter<Outbox>) {
///     for message in inbox.iter() {
///         if let Message::Text(text) = &message.content {
///             if text == "ping" {
///                 outbox.send(Outbox { to: message.from, content: Message::Text("pong!".into()) })
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Outbox {
    pub to: ClientId,
    pub content: Message,
}
