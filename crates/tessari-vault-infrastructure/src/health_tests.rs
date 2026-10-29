use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::{HealthError, probe, probe_address};

const BUDGET: Duration = Duration::from_secs(2);

/// A listener that reads one request, hands its text back through the join handle, and answers `reply`.
async fn answering(reply: &'static [u8]) -> (SocketAddr, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address = listener.local_addr().expect("its address");
    let served = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("a connection");
        let mut request = vec![0_u8; 1024];
        let read = socket.read(&mut request).await.expect("a request");
        socket.write_all(reply).await.expect("the reply");
        String::from_utf8_lossy(&request[..read]).into_owned()
    });
    (address, served)
}

#[tokio::test]
async fn a_200_is_healthy_and_the_request_names_the_probe_route() {
    let (address, served) =
        answering(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\n{}").await;
    assert_eq!(probe(address, BUDGET).await, Ok(()));
    let request = served.await.expect("the server task");
    assert!(
        request.starts_with("GET /api/sign-in/methods HTTP/1.1\r\n"),
        "{request}"
    );
}

#[tokio::test]
async fn any_other_status_is_named() {
    let (address, _served) =
        answering(b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\n\r\n").await;
    assert_eq!(probe(address, BUDGET).await, Err(HealthError::Status(503)));
}

#[tokio::test]
async fn an_answer_that_is_not_http_is_named() {
    let (address, _served) = answering(b"SSH-2.0-OpenSSH_9.0\r\n").await;
    assert_eq!(probe(address, BUDGET).await, Err(HealthError::NotHttp));
}

#[tokio::test]
async fn nothing_listening_is_unreachable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address = listener.local_addr().expect("its address");
    drop(listener);
    assert_eq!(
        probe(address, BUDGET).await,
        Err(HealthError::Unreachable(address))
    );
}

#[tokio::test]
async fn a_peer_that_never_answers_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let address = listener.local_addr().expect("its address");
    // Accepts and then says nothing, until the probe gives up.
    let _held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.expect("a connection");
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(socket);
    });
    let budget = Duration::from_millis(200);
    assert_eq!(
        probe(address, budget).await,
        Err(HealthError::TimedOut(budget))
    );
}

#[test]
fn a_server_bound_to_every_address_is_asked_on_the_loopback() {
    let at = |text: &str| text.parse::<SocketAddr>().expect("an address");
    assert_eq!(probe_address(at("0.0.0.0:8080")), at("127.0.0.1:8080"));
    assert_eq!(probe_address(at("[::]:8080")), at("[::1]:8080"));
    assert_eq!(probe_address(at("10.0.0.7:9000")), at("10.0.0.7:9000"));
}
