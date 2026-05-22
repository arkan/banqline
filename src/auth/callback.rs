use anyhow::Result;
use std::sync::mpsc;
use std::time::Duration;
use tiny_http::{Header, Response};

const CALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html><head><title>Authorization</title></head>
<body><p>Authorization complete. You can close this tab.</p></body>
</html>"#;

/// Result of an OAuth callback containing the authorization code on success
/// or an error description on failure.
#[derive(Debug, Clone)]
pub struct CallbackResult {
    /// The authorization code extracted from the `code` query parameter.
    pub code: Option<String>,
    /// An error description extracted from the `error` query parameter.
    pub error: Option<String>,
}

/// Receives the OAuth callback result from the temporary HTTP server.
///
/// Created by [`start_callback_server`]. Dropping this value shuts down
/// the server and joins the background thread.
pub struct CallbackReceiver {
    rx: mpsc::Receiver<CallbackResult>,
    _guard: CallbackGuard,
}

impl CallbackReceiver {
    /// Blocks until the OAuth callback arrives or the server is shut down.
    pub fn recv(&self) -> Result<CallbackResult, mpsc::RecvError> {
        self.rx.recv()
    }
}

/// Guard that signals shutdown to the server thread and joins it on drop.
struct CallbackGuard {
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Starts a temporary HTTP server on the given port to capture a single
/// OAuth callback request at `/callback`.
///
/// The server extracts `code` and `error` query parameters, responds with
/// a confirmation HTML page, sends the result on a channel, and stops.
///
/// Returns a [`CallbackReceiver`] that can be used to block on the result.
pub fn start_callback_server(port: u16) -> Result<CallbackReceiver> {
    let (tx, rx) = mpsc::channel::<CallbackResult>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    let addr = format!("0.0.0.0:{}", port);
    let server = tiny_http::Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("listening on port {}: {}", port, e))?;

    let handle = std::thread::spawn(move || {
        loop {
            match server.recv_timeout(Duration::from_millis(200)) {
                Ok(Some(request)) => {
                    let url = request.url().to_string();
                    let result = CallbackResult {
                        code: parse_query_param(&url, "code"),
                        error: parse_query_param(&url, "error"),
                    };

                    let response = build_html_response();

                    let _ = request.respond(response);
                    let _ = tx.send(result);
                    return;
                }
                Ok(None) => {
                    if shutdown_rx.try_recv().is_ok() {
                        return;
                    }
                }
                Err(_) => {
                    let _ = tx.send(CallbackResult {
                        code: None,
                        error: Some("callback server error".to_string()),
                    });
                    return;
                }
            }
        }
    });

    Ok(CallbackReceiver {
        rx,
        _guard: CallbackGuard {
            shutdown_tx,
            handle: Some(handle),
        },
    })
}

/// Extracts the value of a query parameter from a URL string.
fn parse_query_param(url: &str, param: &str) -> Option<String> {
    let query_start = url.find('?')?;
    let query = &url[query_start + 1..];
    let fragment_end = query.find('#').unwrap_or(query.len());
    let query = &query[..fragment_end];

    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        if parts.next()? == param {
            return parts.next().map(|v| v.to_string());
        }
    }
    None
}

/// Builds the HTML response with a Content-Type header.
fn build_html_response() -> Response<std::io::Cursor<Vec<u8>>> {
    let header: Header = "Content-Type: text/html; charset=utf-8"
        .parse()
        .expect("valid header literal");
    Response::from_string(CALLBACK_HTML).with_header(header)
}
