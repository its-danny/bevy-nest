/// Global configuration for the Nest telnet server.
use bevy::prelude::Resource;

#[derive(Debug, Clone, Resource)]
pub struct NestConfig {
    /// Enable GMCP negotiation and allow sending/receiving GMCP subnegotiations.
    /// When true, the server will automatically:
    /// - Offer `WILL GMCP` on connect.
    /// - Reply `WILL GMCP` to an incoming `DO GMCP`.
    /// - Reply `DO GMCP` to an incoming `WILL GMCP`.
    pub enable_gmcp: bool,
}

impl Default for NestConfig {
    fn default() -> Self {
        Self { enable_gmcp: true }
    }
}
