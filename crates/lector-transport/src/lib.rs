pub mod certs;
pub mod sender;
pub mod receiver;
pub mod cascade;

pub use quinn::Endpoint;

pub const CHUNK_SIZE: usize = 64 * 1024;
pub const QUIC_PORT: u16 = 50052;
