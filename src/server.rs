use std::sync::Arc;

use bevy::prelude::*;
use dashmap::DashMap;
use libmudtelnet_rs::{
    Parser,
    compatibility::{CompatibilityEntry, CompatibilityTable},
    events::{TelnetEvents, TelnetIAC},
    telnet::{op_command, op_option},
};
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, ToSocketAddrs},
    runtime::{Builder, Runtime},
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    channel::Channel,
    config::NestConfig,
    errors::NetworkError,
    events::{Frame, Inbox, IncomingConnection, NetworkEvent, Outbox, Payload},
};

/// A unique identifier for a client.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub struct ClientId(Uuid);

impl ClientId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ClientId {
    fn default() -> Self {
        Self::new()
    }
}

struct Client {
    outbox: Channel<Outbox>,
    #[allow(dead_code)]
    read_task: JoinHandle<()>,
    #[allow(dead_code)]
    write_task: JoinHandle<()>,
}

#[derive(Resource)]
pub struct Server {
    runtime: Runtime,
    clients: Arc<DashMap<ClientId, Client>>,
    config: NestConfig,
    // Incoming connections.
    pub(crate) incoming: Channel<IncomingConnection>,
    // Recently disconnected clients.
    pub(crate) lost: Channel<ClientId>,
    // Network events.
    pub(crate) events: Channel<NetworkEvent>,
    // Messages received from clients.
    pub(crate) inbox: Channel<Inbox>,
}

impl Server {
    pub(crate) fn new(config: NestConfig) -> Self {
        Self {
            runtime: Builder::new_multi_thread()
                .enable_io()
                .build()
                .expect("Could not build runtime"),
            incoming: Channel::new(),
            clients: Arc::new(DashMap::new()),
            lost: Channel::new(),
            events: Channel::new(),
            inbox: Channel::new(),
            config,
        }
    }

    /// Start listening for incoming connections on the given address.
    /// This should be called from [`add_startup_system`](bevy::app::App.add_startup_system).
    pub fn listen(&self, address: impl ToSocketAddrs + Send + 'static) {
        let events = self.events.sender.clone();
        let incoming = self.incoming.sender.clone();

        // Spawn a new task to listen for incoming connections.
        self.runtime.spawn(async move {
            // Create a TCP listener.
            let listener = match TcpListener::bind(address).await {
                Ok(listener) => listener,
                Err(err) => {
                    if let Err(error) = events.send(NetworkEvent::Error(NetworkError::Listen(err)))
                    {
                        error!("Could not send error: {error}");
                    };

                    return;
                }
            };

            info!("Listening");

            loop {
                // Wait for a new connection.
                match listener.accept().await {
                    // If we get a new connection, send it to the incoming channel
                    // to be proccessed later.
                    Ok((socket, address)) => {
                        info!("Accepted connection from {address}");

                        if let Err(err) = incoming.send(IncomingConnection { socket }) {
                            error!("Failed to send incoming connection: {err}");
                        }
                    }
                    Err(err) => {
                        if let Err(err) =
                            events.send(NetworkEvent::Error(NetworkError::Accept(err)))
                        {
                            error!("Could not send error: {err}");
                        };
                    }
                }
            }
        });
    }

    /// Disconnect a client. This will send a [`NetworkEvent::Disconnected`] event.
    pub fn disconnect(&self, client_id: &ClientId) {
        self.remove_client(client_id);
    }

    pub(crate) fn setup_client(&self, connection: IncomingConnection) {
        let (read_socket, mut write_socket) = connection.socket.into_split();

        let id = ClientId::new();
        let outbox: Channel<Outbox> = Channel::new();

        let parser: Arc<Mutex<Parser>> = if self.config.enable_gmcp {
            let mut table = CompatibilityTable::new();

            table.set_option(
                op_option::GMCP,
                CompatibilityEntry::new(true, true, false, false),
            );

            Arc::new(Mutex::new(Parser::with_support(table)))
        } else {
            Arc::new(Mutex::new(Parser::new()))
        };

        let read_parser: Arc<Mutex<Parser>> = Arc::clone(&parser);
        let write_parser: Arc<Mutex<Parser>> = Arc::clone(&parser);
        let config = self.config.clone();

        let read_events_sender = self.events.sender.clone();
        let write_events_sender = self.events.sender.clone();
        let inbox_sender = self.inbox.sender.clone();
        let outbox_receiver = outbox.receiver.clone();
        let outbox_sender = outbox.sender.clone();
        let lost_sender = self.lost.sender.clone();

        self.clients.insert(
            id,
            Client {
                outbox,
                // Spawn a new task to read from the socket.
                // Messages received are sent to the server's inbox.
                read_task: self.runtime.spawn({
                    let parser = read_parser;
                    let config = config.clone();

                    async move {
                        let mut read_socket = read_socket;
                        let max_packet_size = 2048;
                        let mut buffer = vec![0; max_packet_size];

                        info!("Starting read task for {id:?}");

                        loop {
                            let length = match read_socket.read(&mut buffer).await {
                                Ok(n) => n,
                                Err(err) => {
                                    if let Err(err) = read_events_sender.send(NetworkEvent::Error(
                                        NetworkError::SocketRead(err, id),
                                    )) {
                                        error!("Could not send error: {err}");
                                    };

                                    break;
                                }
                            };

                            if length == 0 {
                                if let Err(err) = lost_sender.send(id) {
                                    error!("Could not send lost connection: {err}");
                                }

                                break;
                            }

                            let mut parser = parser.lock().await;
                            let events = parser.receive(&buffer[..length]);
                            drop(parser);

                            for event in events {
                                match event {
                                    TelnetEvents::DataReceive(buf) => {
                                        let text = String::from_utf8_lossy(&buf).into_owned();

                                        if let Err(error) = inbox_sender.send(Inbox {
                                            from: id,
                                            content: Frame::Text(text),
                                        }) {
                                            error!("Could not send to inbox: {error}");
                                        }
                                    }
                                    TelnetEvents::Negotiation(negotiation) => {
                                        if !(config.enable_gmcp
                                            && negotiation.option == op_option::GMCP)
                                            && let Err(error) = inbox_sender.send(Inbox {
                                                from: id,
                                                content: Frame::Negotiation {
                                                    command: negotiation.command,
                                                    option: negotiation.option,
                                                },
                                            })
                                        {
                                            error!("Could not send to inbox: {error}");
                                        }
                                    }
                                    TelnetEvents::Subnegotiation(sub) => {
                                        if sub.option == op_option::GMCP
                                            && let Some(frame) =
                                                parse_gmcp(sub.buffer.to_vec()).map(Frame::GMCP)
                                            && let Err(error) = inbox_sender.send(Inbox {
                                                from: id,
                                                content: frame,
                                            })
                                        {
                                            error!("Could not send GMCP to inbox: {error}");
                                        } else if let Err(error) = inbox_sender.send(Inbox {
                                            from: id,
                                            content: Frame::Subnegotiation {
                                                option: sub.option,
                                                data: sub.buffer.to_vec(),
                                            },
                                        }) {
                                            error!(
                                                "Could not send subnegotiation to inbox: {error}"
                                            );
                                        }
                                    }
                                    TelnetEvents::IAC(TelnetIAC { command }) => {
                                        let _ = inbox_sender.send(Inbox {
                                            from: id,
                                            content: Frame::IAC(command),
                                        });
                                    }
                                    TelnetEvents::DataSend(bytes) => {
                                        if let Err(err) = outbox_sender.send(Outbox {
                                            to: id,
                                            content: Frame::Raw(bytes.to_vec()),
                                        }) {
                                            error!("Could not forward parser DataSend: {err}");
                                        }
                                    }
                                    TelnetEvents::DecompressImmediate(_bytes) => {
                                        // MCCP not supported; drop.
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }),
                write_task: self.runtime.spawn({
                    let parser = write_parser;

                    async move {
                        while let Ok(out) = outbox_receiver.recv() {
                            let result = match out.content {
                                Frame::Text(text) => {
                                    let mut parser = parser.lock().await;
                                    let event = parser.send_text(&text);
                                    drop(parser);
                                    write_event(event, &mut write_socket).await
                                }
                                Frame::GMCP(payload) => {
                                    let gmcp_string = format_gmcp(&payload);
                                    let mut parser = parser.lock().await;
                                    let event =
                                        parser.subnegotiation_text(op_option::GMCP, &gmcp_string);
                                    drop(parser);
                                    if let Some(event) = event {
                                        write_event(event, &mut write_socket).await
                                    } else {
                                        Ok(())
                                    }
                                }
                                Frame::Negotiation { command, option } => {
                                    let mut parser = parser.lock().await;
                                    let event = parser.negotiate(command, option);
                                    drop(parser);
                                    write_event(event, &mut write_socket).await
                                }
                                Frame::Subnegotiation { option, data } => {
                                    let mut parser = parser.lock().await;
                                    let event = parser.subnegotiation(option, data);
                                    drop(parser);
                                    if let Some(event) = event {
                                        write_event(event, &mut write_socket).await
                                    } else {
                                        Ok(())
                                    }
                                }
                                Frame::Raw(command) => {
                                    write_socket.write_all(command.as_slice()).await
                                }
                                Frame::IAC(command) => write_socket.write_all(&[command]).await,
                            };

                            if let Err(err) = result {
                                if let Err(err) = write_events_sender.send(NetworkEvent::Error(
                                    NetworkError::SocketWrite(err, out.to),
                                )) {
                                    error!("Could not send error: {err}");
                                };

                                break;
                            }
                        }
                    }
                }),
            },
        );

        if self.config.enable_gmcp {
            let _ = self.clients.get(&id).and_then(|client| {
                client
                    .outbox
                    .sender
                    .send(Outbox {
                        to: id,
                        content: Frame::Negotiation {
                            command: op_command::WILL,
                            option: op_option::GMCP,
                        },
                    })
                    .err()
            });
        }

        if let Err(err) = self.events.sender.send(NetworkEvent::Connected(id)) {
            error!("Could not send connected event: {err}");
        }
    }

    // Remove a client from the server.
    pub(crate) fn remove_client(&self, id: &ClientId) {
        self.clients.remove(id);

        info!("Client disconnected: {id:?}");

        if let Err(err) = self.events.sender.send(NetworkEvent::Disconnected(*id)) {
            error!("Could not send event: {err}");
        }
    }

    /// Send a frame to a client's outbox.
    pub(crate) fn send(&self, out: &Outbox) {
        if let Some(client) = self.clients.get(&out.to)
            && let Err(err) = client.outbox.sender.send(out.clone())
        {
            error!("Could not send frame: {err}");
        }
    }
}

async fn write_event(
    event: TelnetEvents,
    write_socket: &mut tokio::net::tcp::OwnedWriteHalf,
) -> Result<(), std::io::Error> {
    match event {
        TelnetEvents::DataSend(buf) => write_socket.write_all(&buf).await,
        other => {
            warn!("Unexpected telnet write event: {other:?}");
            Ok(())
        }
    }
}

fn format_gmcp(payload: &Payload) -> String {
    let mut gmcp = payload.package.clone();

    if let Some(sub) = &payload.subpackage {
        gmcp.push('.');
        gmcp.push_str(sub);
    }

    if let Some(data) = &payload.data {
        gmcp.push(' ');
        gmcp.push_str(data);
    }

    gmcp
}

fn parse_gmcp(buffer: Vec<u8>) -> Option<Payload> {
    let text = String::from_utf8(buffer).ok()?;
    let (package_part, data_part) = text.split_once(' ').unwrap_or((text.as_str(), ""));

    let mut parts = package_part.split('.');
    let package = parts.next()?.to_string();
    let subpackage = parts.next().map(|s| s.to_string());

    let data = if data_part.is_empty() {
        None
    } else {
        Some(data_part.to_string())
    };

    Some(Payload {
        package,
        subpackage,
        data,
    })
}
