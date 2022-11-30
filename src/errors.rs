use thiserror::Error;

use crate::server::ClientId;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("An error occured when accepting a new connnection: {0}")]
    Accept(std::io::Error),
    #[error("An error occured when trying to start listening for new connections: {0}")]
    Listen(std::io::Error),
    #[error("An error occured when reading from socket: {0} {1:?}")]
    SocketRead(std::io::Error, ClientId),
    #[error("An error occured when writing to socket: {0} {1:?}")]
    SocketWrite(std::io::Error, ClientId),
}
