//! notify-playground — a hands-on tour of `cosmonic:notify`.
//!
//! Serves a single page that lets you compose any notification the interface
//! can express, fire it, and watch the user's answer come back. It exists to
//! be read as much as run: every host call it makes is in `notify.rs`, in
//! about forty lines, and the page explains what each control maps to.
//!
//! Routes:
//!   GET  /               the playground page
//!   GET  /capabilities   what this machine can actually render
//!   POST /post           fire-and-forget; returns the notification id
//!   POST /request        fire and WAIT for the user's answer
//!   POST /close          withdraw a notification by id
//!   GET  /events         drain the responses queued for this component

mod notify;

use std::io::Read;

use wasmcloud_component::http;

struct Playground;

http::export!(Playground);

impl http::Server for Playground {
    fn handle(
        request: http::IncomingRequest,
    ) -> http::Result<http::Response<impl http::OutgoingBody>> {
        let path = request.uri().path().to_string();
        let method = request.method().clone();
        let (_parts, body) = request.into_parts();

        let response = match (method.as_str(), path.as_str()) {
            ("GET", "/") => html(include_str!("index.html")),
            ("GET", "/capabilities") => json(200, notify::capabilities_json()),
            ("GET", "/events") => json(200, notify::events_json()),
            ("POST", "/post") => with_body(body, notify::post_json),
            ("POST", "/request") => with_body(body, notify::request_json),
            ("POST", "/close") => with_body(body, notify::close_json),
            _ => json(404, r#"{"error":"no such route"}"#.to_string()),
        };
        Ok(response)
    }
}

/// Read the request body and hand it to a handler that answers with JSON.
fn with_body(
    body: http::IncomingBody,
    handler: fn(&str) -> (u16, String),
) -> http::Response<String> {
    let mut body = body;
    let mut bytes = Vec::new();
    if let Err(e) = body.read_to_end(&mut bytes) {
        return json(
            400,
            error_json(&format!("could not read the request body: {e}")),
        );
    }
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return json(400, error_json("the request body was not valid UTF-8")),
    };
    let (status, payload) = handler(&text);
    json(status, payload)
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
