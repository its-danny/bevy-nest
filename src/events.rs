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
#[derive(Debug, Clone)]
pub enum Frame {
    /// Text frame; CRLF is handled by the telnet parser.
    Text(String),
    /// A GMCP message is a JSON object serialized into a string.
    GMCP(Payload),
    /// Negotiation command/option pair (WILL/WONT/DO/DONT).
    Negotiation { command: u8, option: u8 },
    /// Subnegotiation payload for a given option.
    Subnegotiation { option: u8, data: Vec<u8> },
    /// Raw bytes to send untouched (already telnet-framed).
    Raw(Vec<u8>),
    /// Low-level IAC command emitted by the parser.
    IAC(u8),
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
    /// Convert a vector of bytes into a [`Frame::Raw`].
    fn from(v: Vec<u8>) -> Self {
        Frame::Raw(v)
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
#[derive(Debug, Event, Message, Clone)]
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
///                 outbox.write_text(frame.from, "pong!");
///             }
///         }
///     }
/// }
/// ```
#[derive(Debug, Event, Message, Clone)]
pub struct Outbox {
    pub to: ClientId,
    pub content: Frame,
}

/// Extension trait for [`MessageWriter<Outbox>`] to make sending frames easier.
pub trait OutboxWriterExt {
    fn write_text(&mut self, to: ClientId, text: impl Into<String>);
    fn write_gmcp(&mut self, to: ClientId, payload: Payload);
    fn write_negotiation(&mut self, to: ClientId, command: u8, option: u8);
    fn write_subnegotiation(&mut self, to: ClientId, option: u8, data: impl Into<Vec<u8>>);
    fn write_raw(&mut self, to: ClientId, data: impl Into<Vec<u8>>);
}

impl OutboxWriterExt for MessageWriter<'_, Outbox> {
    /// Sends a [`Frame::Text`] to a client.
    fn write_text(&mut self, to: ClientId, text: impl Into<String>) {
        self.write(Outbox {
            to,
            content: Frame::Text(text.into()),
        });
    }

    /// Sends a [`Frame::GMCP`] to a client.
    fn write_gmcp(&mut self, to: ClientId, payload: Payload) {
        self.write(Outbox {
            to,
            content: Frame::GMCP(payload),
        });
    }

    /// Sends a negotiation frame (WILL/WONT/DO/DONT).
    fn write_negotiation(&mut self, to: ClientId, command: u8, option: u8) {
        self.write(Outbox {
            to,
            content: Frame::Negotiation { command, option },
        });
    }

    /// Sends a subnegotiation payload for an option.
    fn write_subnegotiation(&mut self, to: ClientId, option: u8, data: impl Into<Vec<u8>>) {
        self.write(Outbox {
            to,
            content: Frame::Subnegotiation {
                option,
                data: data.into(),
            },
        });
    }

    /// Sends raw bytes that are assumed already telnet-framed/escaped.
    fn write_raw(&mut self, to: ClientId, data: impl Into<Vec<u8>>) {
        self.write(Outbox {
            to,
            content: Frame::Raw(data.into()),
        });
    }
}
