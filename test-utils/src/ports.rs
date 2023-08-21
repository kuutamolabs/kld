use std::{
    net::{TcpListener, UdpSocket},
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{anyhow, Result};

static NEXT_PORT: AtomicUsize = AtomicUsize::new(10000);

/// get the next free port that has no udp/tcp bound
/// WARNING: If other applications try to acquire these ports independent from
/// our test suite, this method is unreliable and racy.
pub fn get_available_port() -> Result<u16> {
    loop {
        let port = NEXT_PORT.fetch_add(1, Ordering::SeqCst);
        if port > 65535 {
            return Err(anyhow!("no ports left"));
        }
        let port = port as u16;
        if TcpListener::bind(("127.0.0.1", port)).is_ok()
            && UdpSocket::bind(("127.0.0.1", port)).is_ok()
        {
            return Ok(port);
        }
    }
}
