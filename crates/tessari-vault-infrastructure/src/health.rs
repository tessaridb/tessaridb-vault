//! Whether the server answers: what `tessaridb-vault health` asks of a running server, for an image with no shell or
//! HTTP client of its own to ask it with.
//!
//! The probe reads `GET /api/sign-in/methods`, a route that names no account, opens no session and counts no
//! attempt, so asking every few seconds changes nothing the server keeps.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Why the server did not answer as a healthy one does.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum HealthError {
    /// Nothing accepted a connection at the address.
    #[error("nothing answers at {0}")]
    Unreachable(SocketAddr),
    /// The server answered with a status other than 200.
    #[error("the server answered {0}")]
    Status(u16),
    /// What came back was not an HTTP status line.
    #[error("the answer is not HTTP")]
    NotHttp,
    /// No answer within the budget.
    #[error("no answer within {0:?}")]
    TimedOut(Duration),
}

/// The address to probe for a server listening on `listen`: a server bound to every address is asked on the loopback
/// of the same family, at the same port.
#[must_use]
pub fn probe_address(listen: SocketAddr) -> SocketAddr {
    let ip = match listen.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, listen.port())
}

/// How long `health` waits for the whole exchange: inside the image's five-second health check timeout.
pub const PROBE_BUDGET: Duration = Duration::from_secs(3);

/// The most of an answer read: enough for any status line.
const STATUS_LINE_MAX: usize = 256;

/// Ask the server at `address` whether it answers, within `budget` for the whole exchange.
///
/// # Errors
///
/// [`HealthError`] naming what was wrong.
pub async fn probe(address: SocketAddr, budget: Duration) -> Result<(), HealthError> {
    match tokio::time::timeout(budget, exchange(address)).await {
        Ok(answer) => answer,
        Err(_) => Err(HealthError::TimedOut(budget)),
    }
}

async fn exchange(address: SocketAddr) -> Result<(), HealthError> {
    let mut socket = TcpStream::connect(address)
        .await
        .map_err(|_| HealthError::Unreachable(address))?;
    let request = format!(
        "GET /api/sign-in/methods HTTP/1.1\r\nhost: {address}\r\nconnection: close\r\n\r\n"
    );
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|_| HealthError::Unreachable(address))?;
    let mut answer = Vec::with_capacity(STATUS_LINE_MAX);
    let mut chunk = [0_u8; STATUS_LINE_MAX];
    while !answer.contains(&b'\n') && answer.len() < STATUS_LINE_MAX {
        let read = socket
            .read(&mut chunk)
            .await
            .map_err(|_| HealthError::NotHttp)?;
        if read == 0 {
            break;
        }
        answer.extend_from_slice(chunk.get(..read).unwrap_or_default());
    }
    match status(&answer).ok_or(HealthError::NotHttp)? {
        200 => Ok(()),
        other => Err(HealthError::Status(other)),
    }
}

/// The status code of `HTTP/1.x NNN …`, or `None` when the answer does not begin with one.
fn status(answer: &[u8]) -> Option<u16> {
    let line = answer.split(|byte| *byte == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?;
    let mut parts = line.split(' ');
    if !parts.next()?.starts_with("HTTP/1.") {
        return None;
    }
    let code = parts.next()?;
    (code.len() == 3).then(|| code.parse().ok()).flatten()
}

#[cfg(test)]
#[path = "health_tests.rs"]
mod tests;
