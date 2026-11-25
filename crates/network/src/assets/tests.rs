use super::*;

const ENTRY: &[u8] = b"<!doctype html><html lang=en><title>Fixture</title><script type=module src=/assets/app-ASDF42_q.js></script>";
const JS_PATH: &str = "/assets/app-ASDF42_q.js";
const JS: &[u8] = b"export const fixture = true;";
const FILES: &[EmbeddedFile] = &[
    EmbeddedFile {
        key: "app-ASDF42_q.js",
        bytes: JS,
    },
    EmbeddedFile {
        key: "app-mj98YOUZ.css",
        bytes: b":root { color: black; }",
    },
    EmbeddedFile {
        key: "logo-mj98YOUR.avif",
        bytes: b"image fixture",
    },
    EmbeddedFile {
        key: "font-mj98YOUF.woff2",
        bytes: b"font fixture",
    },
];

fn fixture(authority: &str) -> (EmbeddedAssets, StaticResponsePolicy) {
    (
        EmbeddedAssets::new(ENTRY, FILES).unwrap(),
        StaticResponsePolicy::for_approved_authority(&authority.parse().unwrap()).unwrap(),
    )
}

fn request(method: Method, uri: &str) -> Request<&[u8]> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(&[] as &[u8])
        .unwrap()
}

#[test]
fn entry_has_no_store_and_security_headers_use_only_approved_authority() {
    let (assets, policy) = fixture("[2001:db8::2]:54321");
    let mut request = request(Method::GET, "/");
    request
        .headers_mut()
        .insert("host", HeaderValue::from_static("untrusted.invalid:2148"));
    request
        .headers_mut()
        .insert(IF_NONE_MATCH, HeaderValue::from_static("*"));
    let reply = assets.respond(&request, &policy);
    assert_eq!(reply.status(), StatusCode::OK);
    assert_eq!(*reply.body(), ENTRY);
    let headers = reply.headers();
    assert_eq!(headers[CONTENT_TYPE], "text/html; charset=utf-8");
    assert_eq!(headers[CACHE_CONTROL], "no-store");
    assert_eq!(headers["referrer-policy"], "no-referrer");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    assert_eq!(headers["x-frame-options"], "DENY");
    assert_eq!(headers["cross-origin-resource-policy"], "same-origin");
    assert_eq!(
        headers["content-security-policy"],
        "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self' ws://[2001:db8::2]:54321; frame-ancestors 'none'; base-uri 'none'; form-action 'none'; object-src 'none'"
    );
    assert!(!headers.contains_key(ETAG));
    assert!(!headers.contains_key("access-control-allow-origin"));
}

#[test]
fn typed_assets_are_immutable_and_head_has_the_get_length() {
    let (assets, policy) = fixture("192.0.2.1:54321");
    for (path, content_type, size) in [
        (JS_PATH, "text/javascript; charset=utf-8", JS.len()),
        ("/assets/app-mj98YOUZ.css", "text/css; charset=utf-8", 23),
        ("/assets/logo-mj98YOUR.avif", "image/avif", 13),
        ("/assets/font-mj98YOUF.woff2", "font/woff2", 12),
    ] {
        let get = request(Method::GET, path);
        let reply = assets.respond(&get, &policy);
        assert_eq!(reply.status(), StatusCode::OK);
        assert_eq!(reply.headers()[CONTENT_TYPE], content_type);
        assert_eq!(reply.headers()[CONTENT_LENGTH], size.to_string());
        assert_eq!(
            reply.headers()[CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(reply.body().len(), size);

        let head = request(Method::HEAD, path);
        let head = assets.respond(&head, &policy);
        assert_eq!(head.status(), StatusCode::OK);
        assert_eq!(head.headers(), reply.headers());
        assert_eq!(*head.body(), b"".as_slice());
    }
}

#[test]
fn validators_support_weak_comparison_and_quoted_commas_with_a_byte_budget() {
    let (assets, policy) = fixture("tether-a1b2c3d4e5f6.local.:54321");
    let mut request = request(Method::GET, JS_PATH);
    let baseline = assets.respond(&request, &policy);
    let tag = baseline.headers()[ETAG].to_str().unwrap();
    assert_eq!(
        tag,
        "\"2891320f35346524f47f79b031d358bb976b003f8ac1716abb6d66281fde6156\""
    );
    for matching in [
        tag.to_owned(),
        format!("\"different,with,commas\", W/{tag}"),
        "*".to_owned(),
    ] {
        request
            .headers_mut()
            .insert(IF_NONE_MATCH, matching.parse().unwrap());
        let reply = assets.respond(&request, &policy);
        assert_eq!(reply.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(reply.headers()[ETAG], tag);
        assert_eq!(
            reply.headers()[CACHE_CONTROL],
            baseline.headers()[CACHE_CONTROL]
        );
        assert!(!reply.headers().contains_key(CONTENT_LENGTH));
        assert_eq!(*reply.body(), b"".as_slice());
    }
    for nonmatching in [
        format!("{tag}broken"),
        format!("{tag}, unterminated"),
        "\"different\"".to_owned(),
        format!("{}*", " ".repeat(MAX_ETAG_HEADER_BYTES)),
    ] {
        request
            .headers_mut()
            .insert(IF_NONE_MATCH, nonmatching.parse().unwrap());
        assert_eq!(assets.respond(&request, &policy).status(), StatusCode::OK);
    }
    request
        .headers_mut()
        .insert(IF_NONE_MATCH, HeaderValue::from_static("\"other\""));
    request
        .headers_mut()
        .append(IF_NONE_MATCH, HeaderValue::from_str(tag).unwrap());
    *request.method_mut() = Method::HEAD;
    assert_eq!(
        assets.respond(&request, &policy).status(),
        StatusCode::NOT_MODIFIED
    );
}

#[test]
fn only_exact_paths_and_bodyless_get_or_head_are_served() {
    let (assets, policy) = fixture("192.0.2.1:54321");
    for path in [
        "/index.html",
        "/assets/",
        "/assets/app-ASDF42_q.js.map",
        "/assets/sub/app-ASDF42_q.js",
        "/assets/../app-ASDF42_q.js",
        "/assets/%2e%2e%2fapp-ASDF42_q.js",
        "/assets/app%2DASDF42_q.js",
        "/settings",
    ] {
        let reply = assets.respond(&request(Method::GET, path), &policy);
        assert_eq!(reply.status(), StatusCode::NOT_FOUND);
        assert_eq!(reply.headers()[CACHE_CONTROL], "no-store");
        assert_eq!(*reply.body(), b"".as_slice());
    }
    for target in [
        "http://192.0.2.1:54321/",
        "/?token=ignored",
        "/assets/app-ASDF42_q.js?x=y",
    ] {
        let reply = assets.respond(&request(Method::GET, target), &policy);
        assert_eq!(reply.status(), StatusCode::BAD_REQUEST);
        assert_eq!(*reply.body(), b"".as_slice());
    }
    let reply = assets.respond(&request(Method::POST, JS_PATH), &policy);
    assert_eq!(reply.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(reply.headers()[ALLOW], "GET, HEAD");
    assert_eq!(reply.headers()[CACHE_CONTROL], "no-store");

    let mut request = request(Method::GET, "/");
    *request.body_mut() = b"body fixture";
    assert_eq!(
        assets.respond(&request, &policy).status(),
        StatusCode::BAD_REQUEST
    );
    *request.body_mut() = b"";
    request
        .headers_mut()
        .insert(CONTENT_LENGTH, HeaderValue::from_static("1"));
    assert_eq!(
        assets.respond(&request, &policy).status(),
        StatusCode::BAD_REQUEST
    );
    request
        .headers_mut()
        .insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    assert_eq!(assets.respond(&request, &policy).status(), StatusCode::OK);
    request
        .headers_mut()
        .append(CONTENT_LENGTH, HeaderValue::from_static("0"));
    assert_eq!(
        assets.respond(&request, &policy).status(),
        StatusCode::BAD_REQUEST
    );
    request.headers_mut().remove(CONTENT_LENGTH);
    request
        .headers_mut()
        .insert(TRANSFER_ENCODING, HeaderValue::from_static("chunked"));
    assert_eq!(
        assets.respond(&request, &policy).status(),
        StatusCode::BAD_REQUEST
    );
}

#[test]
fn invalid_manifests_and_authorities_fail_before_serving() {
    static MAX_ASSET: [u8; MAX_ASSET_BYTES] = [0; MAX_ASSET_BYTES];
    static TOO_LARGE: [u8; MAX_ASSET_BYTES + 1] = [0; MAX_ASSET_BYTES + 1];
    for key in [
        "app.js",
        "app-abcdefg..js",
        "app-12345678.js.map",
        "app-ABCD!234.js",
        "../app-ABCDEFGH.js",
        "app-ABCDEFGH.html",
    ] {
        let files = [EmbeddedFile { key, bytes: JS }];
        assert!(matches!(
            EmbeddedAssets::new(ENTRY, &files),
            Err(AssetError::InvalidAssetKey)
        ));
    }
    assert!(matches!(
        EmbeddedAssets::new(&[], FILES),
        Err(AssetError::EmptyRepresentation)
    ));
    assert!(matches!(
        EmbeddedAssets::new(&TOO_LARGE, FILES),
        Err(AssetError::SizeLimit)
    ));
    let full = FILES
        .iter()
        .map(|file| EmbeddedFile {
            key: file.key,
            bytes: &MAX_ASSET,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        EmbeddedAssets::new(ENTRY, &full),
        Err(AssetError::SizeLimit)
    ));
    assert!(matches!(
        EmbeddedAssets::new(
            ENTRY,
            &[
                EmbeddedFile {
                    key: "app-ASDF42_q.js",
                    bytes: JS
                },
                EmbeddedFile {
                    key: "app-ASDF42_q.js",
                    bytes: JS
                }
            ]
        ),
        Err(AssetError::DuplicateAssetKey)
    ));
    for authority in [
        "example.local",
        "example.local:0",
        "someone@example.local:1234",
    ] {
        assert!(matches!(
            StaticResponsePolicy::for_approved_authority(&authority.parse().unwrap()),
            Err(AssetError::InvalidAuthority)
        ));
    }
}
