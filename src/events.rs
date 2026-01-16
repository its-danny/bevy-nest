use crate::errors::NetworkError;
use crate::server::ClientId;

use bevy::prelude::*;
use tokio::net::TcpStream;

#[derive(Debug)]
pub(crate) struct IncomingConnection {
    pub(crate) socket: TcpStream,
}

#[derive(Debug, Event, Message)]
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

/// A frame representing an application-level telnet message exchanged
/// between the server and a client.
#[derive(Debug)]
pub enum Frame {
    /// Text frame; appended with a newline when sent to the client.
    Text(String),
    /// Telnet command bytes. You can use the constants in the
    /// [`telnet`](crate::telnet) module to make things easier.
    ///
    /// See: <https://users.cs.cf.ac.uk/Dave.Marshall/Internet/node141.html>
    Command(Vec<u8>),
    /// A GMCP message is a JSON object serialized into a string. The GMCP
    /// protocol is used to send structured data to the client.
    ///
    /// See: <https://www.gammon.com.au/gmcp>
    GMCP(Payload),
}

/// Backwards compatibility alias for the old `Message` name.
#[deprecated(note = "bevy renamed events to messages; this crate now calls its payloads Frames.")]
pub type Message = Frame;

impl From<&str> for Frame {
    /// Convert a string slice into a [`Frame::Text`].
    fn from(s: &str) -> Self {
        Frame::Text(s.into())
    }
}

impl From<String> for Frame {
    /// Convert a string into a [`Frame::Text`].
    fn from(s: String) -> Self {
        Frame::Text(s)
    }
}

impl From<Vec<u8>> for Frame {
    /// Convert a vector of bytes into a [`Frame::Command`].
    fn from(v: Vec<u8>) -> Self {
        Frame::Command(v)
    }
}

impl From<Payload> for Frame {
    /// Convert a [`Payload`] object into a [`Frame::GMCP`].
    fn from(payload: Payload) -> Self {
        Frame::GMCP(payload)
    }
}

/// [`Frame`]s received from a client. These are iterated each update and sent
/// to Bevy via [`Event<Inbox>`](bevy::ecs::event::Event) to be read over.
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_nest::prelude::*;
///
/// fn read_inbox(mut inbox: MessageReader<Inbox>) {
///     for frame in inbox.read() {
///         // ...
///     }
/// }
/// ```
#[derive(Debug, Event, Message)]
pub struct Inbox {
    pub from: ClientId,
    pub content: Frame,
}

/// [`Frame`]s destined for a client. These are iterated each update by the
/// server and written to the client's socket.
///
/// ```rust
/// use bevy::prelude::*;
/// use bevy_nest::prelude::*;
///
/// fn ping_pong(mut inbox: MessageReader<Inbox>, mut outbox: MessageWriter<Outbox>) {
///     for frame in inbox.read() {
///         if let Frame::Text(content) = &frame.content {
///             if content == "ping" {
///                 // There are a few ways to send frames to the outbox:
///                 // 1. Build the frame and send it to the outbox.
///                 outbox.write(Outbox { to: frame.from, content: Frame::Text("pong!".into()) });
///                 // 2. Use the From trait, which is implemented for &str, String, Vec<u8>, and Payload
///                 // for creating text, commands, and GMCP payloads respectively.
///                 outbox.write(Outbox { to: frame.from, content: "pong!".into() });
///                 // 3. Use the extension trait OutboxWriterExt, which provides convenience methods.
///                 outbox.write_text(frame.from, "pong!");
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug, Event, Message)]
pub struct Outbox {
    pub to: ClientId,
    pub content: Frame,
}

/// Extension trait for [`MessageWriter<Outbox>`] to make sending frames easier.
pub trait OutboxWriterExt {
    fn write_text(&mut self, to: ClientId, text: impl Into<String>);
    fn write_command(&mut self, to: ClientId, command: impl Into<Vec<u8>>);
    fn write_gmcp(&mut self, to: ClientId, payload: Payload);
}

impl OutboxWriterExt for MessageWriter<'_, Outbox> {
    /// Sends a [`Frame::Text`] to a client.
    fn write_text(&mut self, to: ClientId, text: impl Into<String>) {
        self.write(Outbox {
            to,
            content: Frame::Text(text.into()),
        });
    }

    /// Sends a [`Frame::Command`] to a client.
    fn write_command(&mut self, to: ClientId, command: impl Into<Vec<u8>>) {
        self.write(Outbox {
            to,
            content: Frame::Command(command.into()),
        });
    }

    /// Sends a [`Frame::GMCP`] to a client.
    fn write_gmcp(&mut self, to: ClientId, payload: Payload) {
        self.write(Outbox {
            to,
            content: Frame::GMCP(payload),
        });
    }
}
