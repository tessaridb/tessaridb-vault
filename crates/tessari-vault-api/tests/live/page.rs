//! The web application the server carries in its binary: the page and its assets, the same committed bytes for anyone,
//! each answered under a policy that lets the page run only its own script and style, talk only to this origin and
//! never be framed.

use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use tower::ServiceExt;

use crate::world::World;

async fn get(world: &World, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let request = Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .expect("a request");
    let response = world.app.clone().oneshot(request).await.expect("an answer");
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = axum::body::to_bytes(response.into_body(), 4 << 20)
        .await
        .expect("a body");
    (status, headers, bytes.to_vec())
}

fn committed(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join(name);
    std::fs::read(path).expect("the committed asset")
}

#[tokio::test]
#[ignore = "needs TESSARIDB_BIN"]
async fn the_page_and_its_assets_are_served_from_the_binary_under_a_closed_policy() {
    let world = World::new().await;
    for (path, file, kind) in [
        ("/", "index.html", "text/html; charset=utf-8"),
        ("/app.js", "app.js", "text/javascript; charset=utf-8"),
        ("/app.css", "app.css", "text/css; charset=utf-8"),
        ("/favicon.svg", "favicon.svg", "image/svg+xml"),
    ] {
        let (status, headers, bytes) = get(&world, path).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(bytes, committed(file), "{path} is not the committed {file}");
        assert_eq!(headers["content-type"], kind, "{path}");
        let policy = headers["content-security-policy"].to_str().expect("text");
        for directive in [
            "default-src 'none'",
            "script-src 'self'",
            "connect-src 'self'",
            "frame-ancestors 'none'",
        ] {
            assert!(policy.contains(directive), "{path}: {policy}");
        }
        assert_eq!(headers["x-content-type-options"], "nosniff", "{path}");
        assert_eq!(headers["referrer-policy"], "no-referrer", "{path}");
    }
    let (status, _, _) = get(&world, "/nowhere.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
