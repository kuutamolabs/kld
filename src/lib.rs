// This lib.rs is just to help with integration testing.
pub mod api;
pub mod controller;
mod event_handler;
mod hex_utils;
pub mod key_generator;
mod net_utils;
mod payment_info;
pub mod prometheus;
pub mod wallet;

pub const VERSION: &str = concat!("LYND v", env!("CARGO_PKG_VERSION"));
