use std::sync::Mutex;
use std::time::Duration;

use reviewer_server::{build_app_state, serve_with_graceful_shutdown};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

async fn setup_env(temp: &tempfile::TempDir) {
    std::env::set_var("DATA_ROOT_DIR", temp.path());
    let yaml_path = temp.path().join("projects.yaml");
    std::fs::write(&yaml_path, "projects: []\n").expect("write yaml");
    std::env::set_var("PROJECTS_CONFIG", &yaml_path);
}

/// Send a minimal HTTP/1.1 GET and return the response's status line.
/// Avoids adding a new HTTP client dependency just for this test.
async fn http_get_status_line(addr: std::net::SocketAddr, path: &str) -> std::io::Result<String> {
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    let response = String::from_utf8_lossy(&buf);
    let status_line = response.lines().next().unwrap_or_default().to_string();
    Ok(status_line)
}

/// End-to-end: once the shutdown signal fires, the HTTP listener stops
/// accepting new connections and the serve future resolves well within the
/// 15s hard deadline when there is no in-flight work to drain.
#[tokio::test]
async fn shutdown_signal_stops_new_connections_and_exits_promptly() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let state = build_app_state().await.expect("build app state");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    // Fire the shutdown signal shortly after serve starts, instead of
    // waiting on a real OS signal.
    let signal = |shutdown: CancellationToken| async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown.cancel();
    };

    let started = std::time::Instant::now();
    let serve_result = tokio::time::timeout(
        Duration::from_secs(15),
        serve_with_graceful_shutdown(listener, state, signal),
    )
    .await;
    let elapsed = started.elapsed();

    assert!(
        serve_result.is_ok(),
        "serve must resolve within the 15s hard deadline"
    );
    serve_result.unwrap().expect("serve result");
    assert!(
        elapsed < Duration::from_secs(15),
        "expected prompt shutdown with no in-flight work, took {elapsed:?}"
    );

    // The listener has been consumed by axum::serve and shut down; new
    // connections to the same address must now be refused.
    let connect_result = tokio::net::TcpStream::connect(addr).await;
    assert!(
        connect_result.is_err(),
        "expected connection to be refused after shutdown"
    );
}

/// If in-flight work never finishes draining, the process must still exit
/// at the 15s hard deadline rather than hang forever. Simulated by a signal
/// future that cancels the shutdown token but then hangs indefinitely
/// itself (standing in for "cleanup never converges") — `axum::serve`'s own
/// graceful shutdown has nothing to drain here, so the hard-deadline branch
/// is what must end the `tokio::select!`.
#[tokio::test]
async fn shutdown_hard_deadline_forces_exit_when_signal_future_hangs() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let state = build_app_state().await.expect("build app state");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");

    // Cancel almost immediately (so the hard-deadline clock starts early),
    // then hang forever instead of letting `with_graceful_shutdown`'s
    // future resolve. This stands in for cleanup that never converges.
    let signal = |shutdown: CancellationToken| async move {
        shutdown.cancel();
        std::future::pending::<()>().await;
    };

    let started = std::time::Instant::now();
    let serve_result = tokio::time::timeout(
        Duration::from_secs(17),
        serve_with_graceful_shutdown(listener, state, signal),
    )
    .await;
    let elapsed = started.elapsed();

    assert!(
        serve_result.is_ok(),
        "serve_with_graceful_shutdown must return by the hard deadline, not hang past it"
    );
    assert!(
        elapsed >= Duration::from_secs(15) && elapsed < Duration::from_secs(17),
        "expected the 15s hard deadline to be what ends shutdown, took {elapsed:?}"
    );
}

/// Sanity check that `serve_with_graceful_shutdown` actually serves HTTP
/// traffic before shutdown begins (guards against a vacuous "stops
/// accepting connections" test where nothing was ever accepted at all).
#[tokio::test]
async fn server_accepts_requests_before_shutdown_begins() {
    let _guard = ENV_TEST_LOCK.lock().expect("env test lock");
    let temp = tempfile::tempdir().expect("tempdir");
    setup_env(&temp).await;

    let state = build_app_state().await.expect("build app state");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");

    let signal = |shutdown: CancellationToken| async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        shutdown.cancel();
    };

    let server = tokio::spawn(serve_with_graceful_shutdown(listener, state, signal));

    // Give the server a moment to start accepting, then hit /health.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let status_line = http_get_status_line(addr, "/health")
        .await
        .expect("health request");
    assert!(
        status_line.contains("200"),
        "expected 200 OK, got: {status_line}"
    );

    server.await.expect("join").expect("serve result");
}
