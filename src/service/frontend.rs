//! Frontend service: Vite proxy in development, embedded assets in release builds.

// The dependency is used by the embedded frontend in release builds.
#[cfg(debug_assertions)]
use rust_embed as _;

#[cfg(debug_assertions)]
pub async fn serve(
    state: &ntex::web::AppState<crate::AppState>,
    _: (),
    request: ntex::web::HttpRequest,
    payload: ntex::web::types::Payload,
) -> crate::service::Result<ntex::web::HttpResponse> {
    use ntex::web::HttpResponse;

    let path = if state.cfg.service.path.is_empty() {
        request.path()
    } else {
        request
            .path()
            .strip_prefix(&format!("/{}", state.cfg.service.path))
            .unwrap_or(request.path())
    };
    let path = if path.is_empty() { "/" } else { path };
    let query = request.query_string();
    let url = if query.is_empty() {
        format!("http://localhost:5173{path}")
    } else {
        format!("http://localhost:5173{path}?{query}")
    };
    let client = ntex::client::Client::new();
    let mut outgoing = client
        .request(request.method().clone(), url)
        .no_decompress();
    for (name, value) in request.headers().iter() {
        if !is_hop_header(name.as_str()) && name != "host" {
            outgoing = outgoing.header(name, value);
        }
    }
    let upstream = outgoing.send_stream(payload).await.map_err(|error| {
        tracing::warn!("Vite proxy failed: {error}");
        crate::service::Error::error(502, "Vite development server is unavailable")
    })?;
    let mut response = HttpResponse::builder(upstream.status());
    for (name, value) in upstream.headers().iter() {
        if !is_hop_header(name.as_str()) {
            response.header(name, value);
        }
    }
    Ok(response.streaming(upstream))
}

#[cfg(debug_assertions)]
fn is_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;
#[cfg(not(debug_assertions))]
#[derive(RustEmbed)]
#[folder = "web/dist"]
struct StaticAssets;

#[cfg(not(debug_assertions))]
pub async fn serve(
    state: &ntex::web::AppState<crate::AppState>,
    _: (),
    req: ntex::web::HttpRequest,
) -> ntex::web::HttpResponse {
    use ntex::http::{
        HeaderMap, StatusCode,
        header::{self, HeaderValue},
    };
    use ntex::web::HttpResponse as Response;

    // Strip the base path prefix from the URI when serving under a subdirectory
    let raw_path = req.path();
    let relative_path = if state.cfg.service.path.is_empty() {
        raw_path
    } else {
        let prefix = format!("/{}", state.cfg.service.path);
        raw_path.strip_prefix(&prefix).unwrap_or(raw_path)
    };
    let path = relative_path.trim_start_matches('/');
    let mut asset_path = if path.is_empty() {
        "index.html".to_string()
    } else {
        path.to_string()
    };

    // 检查路径是否为文件夹
    if !asset_path.contains('.') {
        // 先检查是否有同名 .html 文件（MPA 路由，如 /login -> login.html）
        let page_path = format!("{}.html", asset_path);
        let has_page = StaticAssets::iter().any(|p| p.as_ref() == page_path);
        if has_page {
            asset_path = page_path;
        } else {
            // 再检查是否有同名文件夹下的 index.html
            let folder_path = if asset_path.ends_with('/') {
                asset_path.clone()
            } else {
                format!("{}/", asset_path)
            };
            let index_path = format!("{}index.html", folder_path);
            let has_index = StaticAssets::iter().any(|p| p.as_ref() == index_path);
            if has_index {
                asset_path = index_path;
            }
        }
    }

    // Try to get the file from embedded assets; fallback for 404
    let (asset_exists, content) = match StaticAssets::get(asset_path.as_str()) {
        Some(c) => (true, c),
        None => match StaticAssets::get("not-found.html") {
            Some(c) => (false, c),
            None => {
                // When not_found/index.html has not been created yet, use a minimal
                // inline fallback so the server doesn't crash.
                (
                    false,
                    rust_embed::EmbeddedFile {
                        data: std::borrow::Cow::Borrowed(
                            b"<!DOCTYPE html><title>Not Found</title><h1>404</h1>",
                        ),
                        metadata: rust_embed::Metadata::__rust_embed_new(
                            [0u8; 32],
                            None,
                            None,
                            "text/html; charset=utf-8",
                        ),
                    },
                )
            }
        },
    };
    let mut headers = HeaderMap::new();

    // Inject base path into HTML files
    let is_html = content.metadata.mimetype().starts_with("text/html");
    if is_html && !state.cfg.service.path.is_empty() {
        let html = String::from_utf8_lossy(&content.data);
        let base_path = serde_json::to_string(&format!("/{}", state.cfg.service.path)).unwrap();
        let app_title = serde_json::to_string(&state.cfg.service.name).unwrap();
        let script = format!(
            concat!(
                r#"<script>window.__APHANITE_BASE={};"#,
                r#" window.__APHANITE_NAME={};</script>"#
            ),
            base_path, app_title
        );
        let modified = html.replace("</head>", &format!("{script}</head>"));
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_str(content.metadata.mimetype()).unwrap(),
        );
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        let body = modified.into_bytes();
        let mut builder = Response::builder(if asset_exists {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        });
        {
            for (name, value) in headers.iter() {
                builder.header(name, value);
            }
        }
        return builder.body(body);
    }

    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(if asset_exists {
            content.metadata.mimetype()
        } else {
            "text/html"
        })
        .unwrap(),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );

    // Range 支持
    if let Some(range_header) = req.headers().get(header::RANGE)
        && let Ok(range_str) = range_header.to_str()
        && let Some((start, end)) = parse_range(range_str, content.data.len())
    {
        let slice = &content.data[start..end];
        headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!(
                "bytes {}-{}/{}",
                start,
                end - 1,
                content.data.len()
            ))
            .unwrap(),
        );
        let mut builder = Response::builder(StatusCode::PARTIAL_CONTENT);
        {
            for (name, value) in headers.iter() {
                builder.header(name, value);
            }
        }
        return builder.body(slice.to_vec());
    }

    let body = content.data.into_owned();

    let mut builder = Response::builder(if asset_exists {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    });
    {
        for (name, value) in headers.iter() {
            builder.header(name, value);
        }
    }
    builder.body(body)
}

#[cfg(not(debug_assertions))]
fn parse_range(range: &str, total_len: usize) -> Option<(usize, usize)> {
    if !range.starts_with("bytes=") {
        return None;
    }
    let parts: Vec<&str> = range[6..].split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parts[0].parse::<usize>().ok()?;
    let end = parts[1].parse::<usize>().ok().unwrap_or(total_len - 1);
    if start >= total_len || end >= total_len || start > end {
        return None;
    }
    Some((start, end + 1))
}
