//! `iviewer://` 自定义协议。
//!
//! - `iviewer://preview/{image_id}/{revision}`
//! - `iviewer://thumbnail/{image_id}/{size}/{revision}`
//!
//! URL 只包含授权句柄，不包含文件路径；handler 在 Rust 缓存/授权表中查找，
//! 因此前端无法通过拼接 URL 读取任意文件。响应是 WebView 可解码的 PNG，
//! 不经过 JSON/Base64。

use std::sync::Arc;

use tauri::http::{Request, Response, StatusCode, Uri, header};
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder, Wry};

use crate::preview::is_valid_image_id;
use crate::state::AppState;

const MAX_THUMBNAIL_SIZE: u32 = 512;

pub fn handle(
    context: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    let method = request.method().clone();
    let uri = request.uri().clone();

    // 同步 handler 会占用 WebView 的资源请求线程；编码放到独立线程。
    std::thread::spawn(move || {
        let response = if method == tauri::http::Method::GET {
            route(&app, &uri)
        } else {
            text_response(StatusCode::METHOD_NOT_ALLOWED, "method not allowed")
        };
        responder.respond(response);
    });
}

fn route(app: &AppHandle, uri: &Uri) -> Response<Vec<u8>> {
    match parse_path(uri.path()) {
        Some(Resource::Preview { image_id, revision }) => preview_response(app, image_id, revision),
        Some(Resource::Thumbnail { image_id, size }) => thumbnail_response(app, image_id, size),
        None => text_response(StatusCode::NOT_FOUND, "unknown resource"),
    }
}

fn preview_response(app: &AppHandle, image_id: &str, revision: u64) -> Response<Vec<u8>> {
    if !is_valid_image_id(image_id) {
        return text_response(StatusCode::BAD_REQUEST, "invalid image id");
    }
    let Some(entry) = app.state::<AppState>().previews.get(image_id, revision) else {
        return text_response(StatusCode::NOT_FOUND, "preview is not available");
    };

    let bytes = entry.png();
    if bytes.is_empty() {
        return text_response(StatusCode::INTERNAL_SERVER_ERROR, "png encoding failed");
    }
    png_response(bytes)
}

fn thumbnail_response(app: &AppHandle, image_id: &str, size: u32) -> Response<Vec<u8>> {
    if !is_valid_image_id(image_id) {
        return text_response(StatusCode::BAD_REQUEST, "invalid image id");
    }
    if size == 0 || size > MAX_THUMBNAIL_SIZE {
        return text_response(StatusCode::BAD_REQUEST, "invalid thumbnail size");
    }
    let state = app.state::<AppState>();
    let Some(path) = state.path_for(image_id) else {
        return text_response(StatusCode::NOT_FOUND, "image id is not registered");
    };
    let Some(bytes) = state.thumbnails.get_or_decode(&path, size) else {
        return text_response(StatusCode::NOT_FOUND, "thumbnail is not available");
    };
    png_response(bytes)
}

fn png_response(bytes: Arc<[u8]>) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "image/png")
        // revision / 修改时间已经进入 URL，可长期缓存；淘汰只影响下一次新 revision。
        .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header("X-Content-Type-Options", "nosniff")
        .body(bytes.as_ref().to_vec())
        .unwrap_or_else(|_| text_response(StatusCode::INTERNAL_SERVER_ERROR, "invalid response"))
}

fn text_response(status: StatusCode, message: &str) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .body(message.as_bytes().to_vec())
        .unwrap_or_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Resource<'a> {
    Preview { image_id: &'a str, revision: u64 },
    Thumbnail { image_id: &'a str, size: u32 },
}

/// 同时兼容 Windows 的 `http://iviewer.localhost/preview/...` 与
/// macOS/Linux 的 `iviewer://localhost/preview/...`：两者 URI path 都是 `/preview/...`。
fn parse_path(path: &str) -> Option<Resource<'_>> {
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
    match segments.as_slice() {
        ["preview", image_id, revision] if !image_id.is_empty() => Some(Resource::Preview {
            image_id,
            revision: revision.parse().ok()?,
        }),
        ["thumbnail", image_id, size, _revision] if !image_id.is_empty() => {
            Some(Resource::Thumbnail {
                image_id,
                size: size.parse().ok()?,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_preview_paths() {
        assert_eq!(
            parse_path("/preview/6f1c2c4e-0000-4000-8000-000000000000/7"),
            Some(Resource::Preview {
                image_id: "6f1c2c4e-0000-4000-8000-000000000000",
                revision: 7
            })
        );
        assert_eq!(parse_path("/preview/id/not-a-number"), None);
        assert_eq!(parse_path("/preview/id"), None);
    }

    #[test]
    fn parses_thumbnail_paths_and_rejects_bad_input() {
        assert_eq!(
            parse_path("/thumbnail/6f1c2c4e-0000-4000-8000-000000000000/128/3"),
            Some(Resource::Thumbnail {
                image_id: "6f1c2c4e-0000-4000-8000-000000000000",
                size: 128
            })
        );
        assert_eq!(parse_path("/thumbnail/id/128"), None);
        assert_eq!(parse_path("/file/C:/Windows/System32/config"), None);
    }
}
