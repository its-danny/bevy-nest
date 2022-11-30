/// Interpret as command
pub const IAC: u8 = 255;
/// Begin option subnegotiation
pub const SB: u8 = 250;
/// End option subnegotiation
pub const SE: u8 = 240;
/// Indicates the desire to begin
pub const WILL: u8 = 251;
/// Indicates the refusal to perform
pub const WONT: u8 = 252;
/// Indicates the request that the other party perform
pub const DO: u8 = 253;
/// Indicates the demand that the other party stop performing
pub const DONT: u8 = 254;
/// Echo
pub const ECHO: u8 = 1;
