//! Frontend router
#[cfg(debug_assertions)]
pub fn make_frontend_router() -> axum::Router<crate::AppState> {
    use axum_reverse_proxy::ReverseProxy;
    use tracing::info;

    // 开发模式：反向代理到 Vite 开发服务器
    let vite_dev_server_url = "http://localhost:5173";
    info!(
        "Proxying non-API requests to Vite at {}",
        vite_dev_server_url
    );
    ReverseProxy::new("/", vite_dev_server_url).into()
}

#[cfg(not(debug_assertions))]
pub fn make_frontend_router() -> axum::Router<crate::AppState> {
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::HeaderMap;
    use axum::response::Response;
    use axum::routing::get;
    use rust_embed::RustEmbed;

    #[derive(RustEmbed)]
    #[folder = "web/dist"]
    struct StaticAssets;

    async fn serve_static(
        axum::extract::State(state): axum::extract::State<crate::AppState>,
        req: Request<Body>,
    ) -> Response {
        use axum::http::{HeaderValue, StatusCode, header};

        // Strip the base path prefix from the URI when serving under a subdirectory
        let raw_path = req.uri().path();
        let relative_path = if state.base_path.is_empty() {
            raw_path
        } else {
            let prefix = format!("/{}", state.base_path);
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
            None => match StaticAssets::get("not-found/index.html") {
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
        if is_html && !state.base_path.is_empty() {
            let html = String::from_utf8_lossy(&content.data);
            let base_path = &state.base_path;
            let script = format!(r#"<script>window.__APANITE_BASE__="/{base_path}"</script>"#);
            let modified = html.replace("</head>", &format!("{script}</head>"));
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(content.metadata.mimetype()).unwrap(),
            );
            headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            let body = Body::from(modified.into_bytes());
            let mut builder = Response::builder().status(if asset_exists {
                StatusCode::OK
            } else {
                StatusCode::NOT_FOUND
            });
            {
                let builder_headers = builder.headers_mut().unwrap();
                builder_headers.extend(headers);
            }
            return builder.body(body).unwrap();
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
            let mut builder = Response::builder().status(StatusCode::PARTIAL_CONTENT);
            {
                let builder_headers = builder.headers_mut().unwrap();
                builder_headers.extend(headers);
            }
            return builder.body(Body::from(slice.to_vec())).unwrap();
        }

        let body = Body::from(content.data);

        let mut builder = Response::builder().status(if asset_exists {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        });
        {
            let builder_headers = builder.headers_mut().unwrap();
            builder_headers.extend(headers);
        }
        builder.body(body).unwrap()
    }

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

    axum::routing::Router::new().fallback(get(serve_static))
}
