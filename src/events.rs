use crate::errors::NetworkError;
use crate::server::ClientId;

use bevy::prelude::EventWriter;
use tokio::net::TcpStream;

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

/// Data to be sent to a client over the GMCP protocol.
#[derive(Debug, Clone)]
pub struct Payload {
    pub package: String,
    pub subpackage: Option<String>,
    pub data: Option<String>,
}

/// A message sent from the server to a client or vice versa.
#[derive(Debug)]
pub enum Message {
    /// Just your regular text message. This is appended with a newline when sent
    /// to the client.
    Text(String),
    /// A command is a sequence of bytes used by the telnet protocol. You can use
    /// the constants in the [`telnet`](crate::telnet) module to make things easier.
    ///
    /// See: <https://users.cs.cf.ac.uk/Dave.Marshall/Internet/node141.html>
    Command(Vec<u8>),
    /// A GMCP message is a JSON object serialized into a string. The GMCP
    /// protocol is used to send structured data to the client.
    ///
    /// See: <https://www.gammon.com.au/gmcp>
    GMCP(Payload),
}

impl From<&str> for Message {
    /// Convert a string slice into a [`Message::Text`].
    fn from(s: &str) -> Self {
        Message::Text(s.into())
    }
}

impl From<String> for Message {
    /// Convert a string into a [`Message::Text`].
    fn from(s: String) -> Self {
        Message::Text(s)
    }
}

impl From<Vec<u8>> for Message {
    /// Convert a vector of bytes into a [`Message::Command`].
    fn from(v: Vec<u8>) -> Self {
        Message::Command(v)
    }
}

impl From<Payload> for Message {
    /// Convert a [`Payload`] object into a [`Message::GMCP`].
    fn from(payload: Payload) -> Self {
        Message::GMCP(payload)
    }
}

/// [`Message`] sent from a client. These are iterated over each
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

/// [`Message`] sent to a client. These are iterated over each
/// update by the server and sent to the client's socket.
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_nest::prelude::*;
///
/// fn ping_pong(mut inbox: EventReader<Inbox>, mut outbox: EventWriter<Outbox>) {
///     for message in inbox.iter() {
///         if let Message::Text(content) = &message.content {
///             if content == "ping" {
///                 // There are a few ways to send messages to the outbox:
///                 // 1. Build the message and send it to the outbox.
///                 outbox.send(Outbox { to: message.from, content: Message::Text("pong!".into()) });
///                 // 2. Use the From trait, which is implemented for &str, String, Vec<u8>, and Payload
///                 // for creating text, commands, and GMCP messages respectively.
///                 outbox.send(Outbox { to: message.from, content: "pong!".into() });
///                 // 3. Use the extension trait OutboxWriterExt, which provides convenience methods.
///                 outbox.send_text(message.from, "pong!");
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

/// Extension trait for [`EventWriter<Outbox>`] to make sending messages easier.
pub trait OutboxWriterExt {
    fn send_text(&mut self, to: ClientId, text: impl Into<String>);
    fn send_command(&mut self, to: ClientId, command: impl Into<Vec<u8>>);
    fn send_gmcp(&mut self, to: ClientId, payload: Payload);
}

impl OutboxWriterExt for EventWriter<'_, '_, Outbox> {
    /// Sends a [`Message::Text`] to a client.
    fn send_text(&mut self, to: ClientId, text: impl Into<String>) {
        self.send(Outbox {
            to,
            content: Message::Text(text.into()),
        })
    }

    /// Sends a [`Message::Command`] to a client.
    fn send_command(&mut self, to: ClientId, command: impl Into<Vec<u8>>) {
        self.send(Outbox {
            to,
            content: Message::Command(command.into()),
        })
    }

    /// Sends a [`Message::GMCP`] to a client.
    fn send_gmcp(&mut self, to: ClientId, payload: Payload) {
        self.send(Outbox {
            to,
            content: Message::GMCP(payload),
        })
    }
}
