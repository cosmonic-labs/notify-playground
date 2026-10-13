//! notify-playground — a hands-on tour of `cosmonic:notify`.
//!
//! Serves a single page that lets you compose any notification the interface
//! can express, fire it, and watch the user's answer come back. It is a WASI
//! 0.3 (p3) component: `cosmonic:notify@0.2.0` is all `async func`, and the
//! host serves those only to a component on the async export path. It exists to
//! be read as much as run: every host call it makes is in `notify.rs`, in
//! about forty lines, and the page explains what each control maps to.
//!
//! Routes:
//!   GET  /               the playground page
//!   GET  /capabilities   what this machine can actually render
//!   POST /post           fire-and-forget; returns the notification id
//!   POST /request        fire and WAIT for the user's answer
//!   POST /close          withdraw a notification by id
//!   GET  /events         drain the responses queued for this workload

mod notify;

use wasip3::http::types::{ErrorCode, Method, Request, Response};
use wasip3::http_compat::http_into_wasi_response;

struct Playground;

// `wasi:http/handler@0.3.0` — the p3 async handler. It has to be p3: the
// notify calls below are `async func`, and the host only drives those for a
// component whose export runs on the async path.
wasip3::http::service::export!(Playground);

impl wasip3::exports::http::handler::Guest for Playground {
    async fn handle(request: Request) -> Result<Response, ErrorCode> {
        let method = request.get_method();
        let path = request
            .get_path_with_query()
            .unwrap_or_default()
            .split('?')
            .next()
            .unwrap_or_default()
            .to_string();

        let response = match (&method, path.as_str()) {
            (Method::Get, "/") => html(include_str!("index.html")),
            (Method::Get, "/capabilities") => json(200, notify::capabilities_json().await),
            (Method::Get, "/events") => json(200, notify::events_json().await),
            (Method::Post, "/post") => match read_body(request).await {
                Ok(text) => {
                    let (status, payload) = notify::post_json(&text).await;
                    json(status, payload)
                }
                Err(resp) => resp,
            },
            (Method::Post, "/request") => match read_body(request).await {
                Ok(text) => {
                    let (status, payload) = notify::request_json(&text).await;
                    json(status, payload)
                }
                Err(resp) => resp,
            },
            (Method::Post, "/close") => match read_body(request).await {
                Ok(text) => {
                    let (status, payload) = notify::close_json(&text).await;
                    json(status, payload)
                }
                Err(resp) => resp,
            },
            _ => json(404, r#"{"error":"no such route"}"#.to_string()),
        };
        http_into_wasi_response(response)
    }
}

/// Read the whole request body as UTF-8, or the 400 to answer with instead.
///
/// A p3 body is a `stream<u8>` plus a trailers future. Drain the stream,
/// settle the trailers, then tell the host we are done with the body — each
/// handle closed in order, none dropped while the runtime is still waiting
/// on it.
async fn read_body(request: Request) -> Result<String, http::Response<String>> {
    let (done_tx, done_rx) = wasip3::wit_future::new(|| Ok(()));
    let (stream, trailers) = Request::consume_body(request, done_rx);
    let bytes = stream.collect().await;
    let trailers = trailers.await;
    let _ = done_tx.write(Ok(())).await;
    if let Err(e) = trailers {
        return Err(json(
            400,
            error_json(&format!("could not read the request body: {e:?}")),
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| json(400, error_json("the request body was not valid UTF-8")))
}

fn json(status: u16, body: String) -> http::Response<String> {
    http::Response::builder()
        .status(status)
        .header("content-type", "application/json")
        // The page is served from the same origin, so no CORS is needed; say
        // so explicitly rather than leaving a permissive default lying around.
        .header("cache-control", "no-store")
        .body(body)
        .expect("building a JSON response cannot fail")
}

fn html(body: &str) -> http::Response<String> {
    http::Response::builder()
        .status(200)
        .header("content-type", "text/html; charset=utf-8")
        .body(body.to_string())
        .expect("building an HTML response cannot fail")
}

pub(crate) fn error_json(message: &str) -> String {
    format!(
        r#"{{"error":{}}}"#,
        serde_json::to_string(message).unwrap_or_else(|_| "\"unprintable\"".into())
    )
}
