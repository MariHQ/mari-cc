//! Local web console (`mari console`).
//!
//! A synchronous, thread-per-connection HTTP server that serves the built web
//! console (embedded from `console/dist`) plus a small JSON API over the same
//! repository configuration the CLI uses. It is single-user and localhost-only.

mod api;

use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use tiny_http::{Header, Method, Response, Server};

/// The built console SPA, baked into the binary so `mari console` is
/// self-contained. Rebuilt by `npm --prefix console run build`.
static CONSOLE_DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/console/dist");

/// `mari console [--port N] [--open]`.
pub fn run(port: Option<u16>, open: bool) -> Result<i32> {
    let port = port.unwrap_or(4319);
    let addr = format!("127.0.0.1:{port}");
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("could not bind {addr}: {e}"))
        .context("failed to start the console server")?;
    api::register_current();
    let url = format!("http://{addr}/console");
    println!("mari console → {url}");
    println!(
        "  {} word lists; press Ctrl-C to stop",
        crate::detector::lists::registry().len()
    );
    if open {
        open_browser(&url);
    }

    // The console is local and low-volume, so a thread per request is sufficient.
    for request in server.incoming_requests() {
        std::thread::spawn(move || {
            if let Err(e) = dispatch(request) {
                eprintln!("console: request error: {e:#}");
            }
        });
    }
    Ok(0)
}

fn dispatch(request: tiny_http::Request) -> Result<()> {
    let raw = request.url().to_string();
    let (path, query) = match raw.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw.clone(), String::new()),
    };
    let method = request.method().clone();

    // API + runtime config + local-session stubs are JSON; everything else is a
    // static asset (with SPA fallback to index.html).
    if path == "/config.json" {
        return respond_json(request, 200, api::runtime_config());
    }
    if path.starts_with("/auth/") {
        return respond_json(request, 200, api::auth(&path));
    }
    if path.starts_with("/api/") {
        return handle_api(request, &method, &path, &query);
    }
    serve_static(request, &path)
}

fn handle_api(
    mut request: tiny_http::Request,
    method: &Method,
    path: &str,
    query: &str,
) -> Result<()> {
    // Read the body up front for mutating verbs.
    let body = if matches!(
        method,
        Method::Post | Method::Put | Method::Delete | Method::Patch
    ) {
        let mut s = String::new();
        std::io::Read::read_to_string(request.as_reader(), &mut s).ok();
        s
    } else {
        String::new()
    };
    let ctx = api::Ctx {
        method: method.clone(),
        path: path.to_string(),
        query: query.to_string(),
        body,
    };
    match api::route(&ctx) {
        Ok((status, value)) => respond_json(request, status, value),
        Err(e) => respond_json(
            request,
            400,
            serde_json::json!({ "error": format!("{e:#}") }),
        ),
    }
}

fn respond_json(request: tiny_http::Request, status: u16, value: serde_json::Value) -> Result<()> {
    let body = serde_json::to_string(&value)?;
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(json_header())
        .with_header(nocache_header());
    request.respond(response).ok();
    Ok(())
}

fn serve_static(request: tiny_http::Request, path: &str) -> Result<()> {
    // Normalize: "/" and "/console/*" both serve index.html (SPA). Asset paths
    // like "/assets/foo.js" map straight into the embedded dist.
    let rel = path.trim_start_matches('/');
    let candidate = if rel.is_empty() || rel == "console" || rel.starts_with("console/") {
        None
    } else {
        CONSOLE_DIST.get_file(rel)
    };

    let (bytes, content_type) = match candidate {
        Some(file) => (file.contents().to_vec(), content_type_for(rel)),
        None => match CONSOLE_DIST.get_file("index.html") {
            Some(index) => (index.contents().to_vec(), "text/html; charset=utf-8"),
            None => {
                let response = Response::from_string("console not built").with_status_code(404);
                request.respond(response).ok();
                return Ok(());
            }
        },
    };

    let header = Header::from_bytes(b"Content-Type".as_ref(), content_type.as_bytes())
        .expect("valid content-type header");
    let response = Response::from_data(bytes).with_header(header);
    request.respond(response).ok();
    Ok(())
}

fn content_type_for(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

fn json_header() -> Header {
    Header::from_bytes(
        b"Content-Type".as_ref(),
        b"application/json; charset=utf-8".as_ref(),
    )
    .expect("valid json header")
}

fn nocache_header() -> Header {
    Header::from_bytes(b"Cache-Control".as_ref(), b"no-store".as_ref())
        .expect("valid cache-control header")
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(target_os = "linux")]
    let cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    let cmd = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let cmd = "open";
    let _ = std::process::Command::new(cmd).arg(url).spawn();
}
