//! The mobile asset server's compile-time input. No runtime filesystem access.

use tether_network::assets::{AssetError, EmbeddedAssets, EmbeddedFile};

include!(concat!(env!("OUT_DIR"), "/tether-mobile/bundle.rs"));

pub(crate) fn load() -> Result<EmbeddedAssets, AssetError> {
    EmbeddedAssets::new(ENTRY, FILES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::{CACHE_CONTROL, CONTENT_TYPE, ETAG};
    use http::{Request, StatusCode};
    use tether_network::assets::StaticResponsePolicy;

    #[test]
    fn compiled_mobile_entry_and_vite_assets_serve_through_the_embedded_responder() {
        let assets = load().unwrap();
        let policy =
            StaticResponsePolicy::for_approved_authority(&"192.0.2.1:54321".parse().unwrap())
                .unwrap();
        let entry = assets.respond(&Request::new(&[]), &policy);
        assert_eq!(entry.status(), StatusCode::OK);
        assert_eq!(entry.headers()[CACHE_CONTROL], "no-store");
        let html = std::str::from_utf8(entry.body()).unwrap();
        assert!(html.contains("<title>Tether</title>"));

        for extension in [".js", ".css"] {
            let file = FILES
                .iter()
                .find(|file| file.key.ends_with(extension))
                .unwrap();
            let uri = format!("/assets/{}", file.key);
            assert!(html.contains(&uri));
            let request = Request::builder().uri(uri).body([].as_slice()).unwrap();
            let response = assets.respond(&request, &policy);
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(*response.body(), file.bytes);
            assert!(
                response.headers()[CACHE_CONTROL]
                    .to_str()
                    .unwrap()
                    .contains("immutable")
            );
            assert!(response.headers().contains_key(ETAG));
            let mime = if extension == ".js" {
                "text/javascript; charset=utf-8"
            } else {
                "text/css; charset=utf-8"
            };
            assert_eq!(response.headers()[CONTENT_TYPE], mime);
        }
        for path in [
            "/src/main.ts",
            "/.vite/manifest.json",
            "/assets/index.js.map",
        ] {
            let request = Request::builder().uri(path).body([].as_slice()).unwrap();
            assert_eq!(
                assets.respond(&request, &policy).status(),
                StatusCode::NOT_FOUND
            );
        }
    }
}
