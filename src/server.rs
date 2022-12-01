use std::sync::Arc;

use bevy::{prelude::*, utils::Uuid};
use dashmap::DashMap;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, ToSocketAddrs},
    runtime::{Builder, Runtime},
    task::JoinHandle,
};

use crate::{
    channel::Channel,
    errors::NetworkError,
    events::{Data, Inbox, IncomingConnection, Message, NetworkEvent, Outbox},
    telnet::*,
};

/// A unique identifier for a client.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub struct ClientId(Uuid);

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
    pub(crate) fn new() -> Self {
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
        self.clients.remove(client_id);
    }

    pub(crate) fn setup_client(&self, connection: IncomingConnection) {
        let (mut read_socket, mut write_socket) = connection.socket.into_split();

        let id = ClientId(Uuid::new_v4());
        let outbox: Channel<Outbox> = Channel::new();

        let read_events_sender = self.events.sender.clone();
        let write_events_sender = self.events.sender.clone();
        let inbox_sender = self.inbox.sender.clone();
        let outbox_receiver = outbox.receiver.clone();
        let lost_sender = self.lost.sender.clone();

        self.clients.insert(
            id,
            Client {
                outbox,
                // Spawn a new task to read from the socket.
                // Messages received are sent to the server's inbox.
                read_task: self.runtime.spawn(async move {
                    // Create a buffer to read data into.
                    let max_packet_size = 1024;
                    let mut buffer = vec![0; max_packet_size];

                    info!("Starting read task for {id:?}");

                    loop {
                        // Read data from the socket.
                        let length = match read_socket.read(&mut buffer).await {
                            Ok(n) => n,
                            Err(err) => {
                                if let Err(err) = read_events_sender
                                    .send(NetworkEvent::Error(NetworkError::SocketRead(err, id)))
                                {
                                    error!("Could not send error: {err}");
                                };

                                break;
                            }
                        };

                        // If the length is 0, the socket has been closed.
                        if length == 0 {
                            if let Err(err) = lost_sender.send(id) {
                                error!("Could not send lost connection: {err}");
                            }

                            break;
                        }

                        if buffer[0] == 255 {
                            // This is a command because the first byte is 255.
                            // See: https://users.cs.cf.ac.uk/Dave.Marshall/Internet/node141.html
                            if let Err(error) = inbox_sender.send(Inbox {
                                from: id,
                                content: Message::Command(buffer[..length].to_vec()),
                            }) {
                                error!("Could not send to inbox: {error}");
                            }
                        } else {
                            // Convert the buffer into a string.
                            let clean = std::str::from_utf8(&buffer[..length]).unwrap_or("").trim();

                            // Send the message to the inbox.
                            if !clean.is_empty() {
                                if let Err(error) = inbox_sender.send(Inbox {
                                    from: id,
                                    content: Message::Text(clean.into()),
                                }) {
                                    error!("Could not send to inbox: {error}");
                                }
                            }
                        }
                    }
                }),
                write_task: self.runtime.spawn(async move {
                    // Iterate over messages received from the outbox
                    // and write them to the socket.
                    while let Ok(out) = outbox_receiver.recv() {
                        match out.content {
                            Message::Text(text) => {
                                if let Err(err) =
                                    write_socket.write_all((text + "\r\n").as_bytes()).await
                                {
                                    if let Err(err) = write_events_sender.send(NetworkEvent::Error(
                                        NetworkError::SocketWrite(err, out.to),
                                    )) {
                                        error!("Could not send error: {err}");
                                    };

                                    break;
                                }
                            }
                            Message::Command(command) => {
                                if let Err(err) = write_socket.write_all(command.as_slice()).await {
                                    if let Err(err) = write_events_sender.send(NetworkEvent::Error(
                                        NetworkError::SocketWrite(err, out.to),
                                    )) {
                                        error!("Could not send error: {err}");
                                    };

                                    break;
                                }
                            }
                            Message::GMCP(data) => {
                                let mut payload = vec![IAC, SB, GMCP];

                                payload.extend(data.package.as_bytes());

                                if let Some(subpackage) = data.subpackage {
                                    payload.push(b'.');
                                    payload.extend(subpackage.as_bytes());
                                }

                                if let Some(data) = data.data {
                                    payload.push(b' ');
                                    payload.extend(data.as_bytes());
                                }

                                payload.extend(vec![IAC, SE]);

                                if let Err(err) = write_socket.write_all(payload.as_slice()).await {
                                    if let Err(err) = write_events_sender.send(NetworkEvent::Error(
                                        NetworkError::SocketWrite(err, out.to),
                                    )) {
                                        error!("Could not send error: {err}");
                                    };

                                    break;
                                }
                            }
                        }
                    }
                }),
            },
        );

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

    /// Send a message to a client's outbox.
    pub(crate) fn send(&self, out: &Outbox) {
        match &out.content {
            Message::Text(text) => {
                if let Some(client) = self.clients.get(&out.to) {
                    if let Err(err) = client.outbox.sender.send(Outbox {
                        to: out.to,
                        content: Message::Text(text.clone()),
                    }) {
                        error!("Could not send message: {err}");
                    }
                }
            }
            Message::Command(command) => {
                if let Some(client) = self.clients.get(&out.to) {
                    if let Err(err) = client.outbox.sender.send(Outbox {
                        to: out.to,
                        content: Message::Command(command.clone()),
                    }) {
                        error!("Could not send message: {err}");
                    }
                }
            }
            Message::GMCP(data) => {
                if let Some(client) = self.clients.get(&out.to) {
                    if let Err(err) = client.outbox.sender.send(Outbox {
                        to: out.to,
                        content: Message::GMCP(Data {
                            package: data.package.clone(),
                            subpackage: data.subpackage.clone(),
                            data: data.data.clone(),
                        }),
                    }) {
                        error!("Could not send message: {err}");
                    }
                }
            }
        }
    }
}
