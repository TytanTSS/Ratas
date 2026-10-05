//! Ratas core: content, translations, world generation, the authoritative
//! real-time simulation, the network protocol and the server.

pub mod config;
pub mod content;
pub mod gen;
pub mod game;
pub mod i18n;
pub mod llm;
pub mod proto;
pub mod rng;
pub mod server;
pub mod world;

pub mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}
