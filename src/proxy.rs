use std::time::Duration;

use anyhow::{Context, Result};
use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::redact::{Stats, scrub_body};

const UPSTREAM: &str = "https://api.anthropic.com";
const MAX_BODY: usize = 64 * 1024 * 1024;
const HOP_HEADERS: &[&str] =
    &["host", "connection", "keep-alive", "transfer-encoding", "content-length", "proxy-connection", "upgrade", "te", "trailer"];

pub fn start(port: u16) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let client = reqwest::Client::builder().connect_timeout(Duration::from_secs(30)).build()?;
        let app = Router::new().fallback(forward).with_state(client);
        let listener =
            tokio::net::TcpListener::bind(("127.0.0.1", port)).await.with_context(|| format!("could not listen on 127.0.0.1:{port}"))?;
        println!("eyesoff is listening on http://127.0.0.1:{port}");
        axum::serve(listener, app).await?;
        Ok(())
    })
}

async fn forward(State(client): State<reqwest::Client>, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path_and_query().map_or("/", |p| p.as_str()).to_string();
    let Ok(mut body) = axum::body::to_bytes(body, MAX_BODY).await else {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "eyesoff could not read the request body");
    };

    let is_json = parts.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.contains("json"));
    let mut stats = Stats::default();
    if is_json && !body.is_empty() {
        let raw = body.clone();
        match tokio::task::spawn_blocking(move || scrub_body(&raw)).await {
            Ok(Ok((scrubbed, found))) => {
                body = Bytes::from(scrubbed);
                stats = found;
            }
            _ if path.starts_with("/v1/messages") => {
                return error(StatusCode::BAD_REQUEST, "eyesoff could not parse the request, so it was not sent");
            }
            _ => {}
        }
    }

    let upstream = client
        .request(parts.method.clone(), format!("{UPSTREAM}{path}"))
        .headers(without_hop_headers(&parts.headers))
        .body(body)
        .send()
        .await;
    let upstream = match upstream {
        Ok(response) => response,
        Err(e) => return error(StatusCode::BAD_GATEWAY, &format!("eyesoff could not reach {UPSTREAM}: {e}")),
    };

    let hidden = stats.summary().map(|s| format!("  hid {s}")).unwrap_or_default();
    println!("{} {} {}{hidden}", parts.method, path, upstream.status().as_u16());

    let mut response = Response::builder().status(upstream.status());
    for (name, value) in &without_hop_headers(upstream.headers()) {
        response = response.header(name, value);
    }
    response.body(Body::from_stream(upstream.bytes_stream())).unwrap_or_else(|e| error(StatusCode::BAD_GATEWAY, &e.to_string()))
}

fn without_hop_headers(headers: &HeaderMap) -> HeaderMap {
    let mut headers = headers.clone();
    for name in HOP_HEADERS {
        headers.remove(*name);
    }
    headers
}

fn error(status: StatusCode, message: &str) -> Response {
    let body = json!({"type": "error", "error": {"type": "eyesoff_error", "message": message}});
    (status, [(header::CONTENT_TYPE, "application/json")], body.to_string()).into_response()
}
