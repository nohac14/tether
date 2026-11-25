use super::*;
use crate::assets::EmbeddedFile;
use std::net::Ipv4Addr;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::AsyncWriteExt;

const HOST: &str = "192.0.2.20:54321";
const NAME: &str = "tether-123456abcdef.local";
const GENERATION: NonZeroU64 = NonZeroU64::MIN;
const ENTRY: &[u8] = b"<title>Fixture page</title>";
const JAVASCRIPT: &[u8] = b"export const ready = true;";
const PEER: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 21));

fn server() -> StaticHttpServer {
    StaticHttpServer::new(
        EmbeddedAssets::new(
            ENTRY,
            &[EmbeddedFile {
                key: "index-abcd1234.js",
                bytes: JAVASCRIPT,
            }],
        )
        .unwrap(),
    )
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[derive(Default)]
struct PairingProbe {
    accepted: AtomicUsize,
    retire_on_reply: bool,
}

impl PairingHttpService for &PairingProbe {
    type ControlProof = ();
    type Session = std::convert::Infallible;

    fn handle(&self, request: PairingRequest) -> impl Future<Output = PairingReply> + Send {
        self.accepted.fetch_add(1, Ordering::Relaxed);
        assert!(request.authority.is_current());
        assert_eq!(request.authority.generation(), GENERATION);
        assert_eq!(request.peer, PEER);
        assert_eq!(request.host.host_name(), "192.0.2.20");
        if self.retire_on_reply {
            request.authority.retire_for_test();
        }
        std::future::ready(match request.operation {
            PairingOperation::Submit { label, .. } => {
                assert_eq!(label.for_display(), "Phone");
                PairingReply::Rejected(PairingRejection::Gone)
            }
            PairingOperation::Complete { .. } => PairingReply::Pending,
        })
    }
}

fn pairing_request(
    server: &StaticHttpServer<&PairingProbe>,
    head: &str,
    body: &[u8],
) -> (String, Result<(), ServeError>) {
    runtime().block_on(async {
        let (mut client, stream) = tokio::io::duplex(64 * 1024);
        let scope = RequestAuthorities::new(HOST.parse().unwrap(), GENERATION, None).unwrap();
        let admission = ConnectionAdmission::test_only(GENERATION);
        let server_task = server.serve(stream, &scope, PEER, Some(&admission), || {
            admission.is_current()
        });
        let head = head.to_owned();
        let body = body.to_vec();
        let client_task = tokio::spawn(async move {
            client.write_all(head.as_bytes()).await.unwrap();
            tokio::task::yield_now().await;
            let _closed = client.write_all(&body).await;
            let _closed = client.shutdown().await;
            let mut received = String::new();
            timeout(HEADER_TIMEOUT, client.read_to_string(&mut received))
                .await
                .unwrap()
                .unwrap();
            received
        });
        let outcome = server_task.await;
        (client_task.await.unwrap(), outcome)
    })
}

fn pairing_head(route: &str, body: usize, headers: &str) -> String {
    format!(
        "POST /pairing/{route} HTTP/1.1\r\nHost: {HOST}\r\nContent-Type: application/json\r\nContent-Length: {body}\r\n{headers}\r\n"
    )
}

#[test]
fn pairing_polls_read_a_bounded_split_body_and_return_no_store() {
    let probe = PairingProbe::default();
    let service = StaticHttpServer::with_pairing(server().assets, &probe);
    let body = format!(r#"{{"completion":"{}"}}"#, "a".repeat(64));
    let origin = format!("Origin: http://{HOST}\r\n");
    let (response, outcome) = pairing_request(
        &service,
        &pairing_head("complete", body.len(), &origin),
        body.as_bytes(),
    );
    assert_eq!(outcome, Ok(()));
    assert!(response.starts_with("HTTP/1.1 202 "));
    assert!(response.contains("cache-control: no-store\r\n"));
    assert!(response.contains("referrer-policy: no-referrer\r\n"));
    assert!(response.contains("connection: close\r\n"));
    assert!(!response.contains("access-control-allow-origin"));
    assert_eq!(probe.accepted.load(Ordering::Relaxed), 1);
}

#[test]
fn pairing_rejects_bad_origin_framing_media_type_and_json_before_service() {
    let probe = PairingProbe::default();
    let service = StaticHttpServer::with_pairing(server().assets, &probe);
    let origin = format!("Origin: http://{HOST}\r\n");
    let wrong_origin = "Origin: http://192.0.2.30:54321\r\n";
    let correct = format!(r#"{{"completion":"{}"}}"#, "a".repeat(64));
    let duplicate = format!(
        r#"{{"completion":"{0}","completion":"{0}"}}"#,
        "a".repeat(64)
    );
    let invalid = r#"{"completion":"abc","other":1}"#;
    let trailing = format!("{correct}[]");
    let bad_cases = [
        (
            pairing_head("complete", correct.len(), wrong_origin),
            correct.as_bytes(),
            403,
        ),
        (
            pairing_head("complete", correct.len(), ""),
            correct.as_bytes(),
            403,
        ),
        (pairing_head("complete", 4097, &origin), b"".as_slice(), 413),
        (pairing_head("complete", 0, &origin), b"".as_slice(), 411),
        (
            pairing_head("complete", duplicate.len(), &origin),
            duplicate.as_bytes(),
            400,
        ),
        (
            pairing_head("complete", invalid.len(), &origin),
            invalid.as_bytes(),
            400,
        ),
        (
            pairing_head("complete", trailing.len(), &origin),
            trailing.as_bytes(),
            400,
        ),
        (
            pairing_head("complete", correct.len(), &origin),
            b"{".as_slice(),
            400,
        ),
        (
            pairing_head(
                "complete",
                correct.len(),
                &format!("{origin}Content-Type: application/json\r\n"),
            ),
            correct.as_bytes(),
            415,
        ),
    ];
    for (head, body, status) in bad_cases {
        let (response, _) = pairing_request(&service, &head, body);
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status} ")),
            "expected HTTP status {status}"
        );
    }
    assert_eq!(probe.accepted.load(Ordering::Relaxed), 0);
}

#[test]
fn pairing_retired_generation_cannot_publish_and_requests_have_a_separate_burst() {
    let probe = PairingProbe {
        retire_on_reply: true,
        ..PairingProbe::default()
    };
    let service = StaticHttpServer::with_pairing(server().assets, &probe);
    let body = format!(r#"{{"completion":"{}"}}"#, "a".repeat(64));
    let origin = format!("Origin: http://{HOST}\r\n");
    let (response, outcome) = pairing_request(
        &service,
        &pairing_head("complete", body.len(), &origin),
        body.as_bytes(),
    );
    assert_eq!(response, "");
    assert_eq!(outcome, Err(ServeError::Revoked));

    let probe = PairingProbe::default();
    let service = StaticHttpServer::with_pairing(server().assets, &probe);
    let body = format!(r#"{{"token":"{}","deviceName":"Phone"}}"#, "d".repeat(64));
    let head = pairing_head("request", body.len(), &origin);
    for _ in 0..3 {
        assert!(
            pairing_request(&service, &head, body.as_bytes())
                .0
                .starts_with("HTTP/1.1 410 ")
        );
    }
    assert!(
        pairing_request(&service, &head, body.as_bytes())
            .0
            .starts_with("HTTP/1.1 429 ")
    );
    assert_eq!(probe.accepted.load(Ordering::Relaxed), 3);
}

fn request(head: &str, advertised: Option<AdvertisedHost>) -> String {
    runtime().block_on(async {
        let (mut client, stream) = tokio::io::duplex(64 * 1024);
        let task = tokio::spawn(async move {
            let scope =
                RequestAuthorities::new(HOST.parse().unwrap(), GENERATION, advertised.as_ref())
                    .unwrap();
            server().serve(stream, &scope, PEER, None, || true).await
        });
        client.write_all(head.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap();
        let mut received = String::new();
        timeout(HEADER_TIMEOUT, client.read_to_string(&mut received))
            .await
            .unwrap()
            .unwrap();
        task.await.unwrap().unwrap();
        received
    })
}

#[test]
fn serves_exact_assets_with_head_and_conditional_http_framing() {
    let received = request(
        &format!(
            "GET / HTTP/1.1\r\nHost: {HOST}\r\n\r\nHEAD /assets/index-abcd1234.js HTTP/1.1\r\nHost: {HOST}\r\n\r\n"
        ),
        None,
    );
    let (html_headers, rest) = received.split_once("\r\n\r\n").unwrap();
    assert!(html_headers.contains("HTTP/1.1 200"));
    assert!(html_headers.contains("cache-control: no-store"));
    let (script_headers, rest) = rest
        .strip_prefix(std::str::from_utf8(ENTRY).unwrap())
        .unwrap()
        .split_once("\r\n\r\n")
        .unwrap();
    assert_eq!(rest, "");
    assert!(script_headers.contains("text/javascript; charset=utf-8"));
    assert!(script_headers.contains(&format!("content-length: {}", JAVASCRIPT.len())));
    let tag = script_headers
        .lines()
        .find_map(|line| line.strip_prefix("etag: "))
        .unwrap();
    let received = request(
        &format!(
            "GET /assets/index-abcd1234.js HTTP/1.1\r\nHost: {HOST}\r\nIf-None-Match: W/{tag}\r\nConnection: close\r\n\r\n"
        ),
        None,
    );
    assert!(received.starts_with("HTTP/1.1 304"));
    assert!(!received.contains("content-length:"));
    assert!(received.ends_with("\r\n\r\n"));
}

#[test]
fn denied_authority_never_echoes_host_or_origin() {
    let invalid = [
        String::new(),
        format!("Host: {HOST}\r\nHost: {HOST}\r\n"),
        format!("Host: {HOST},example.invalid\r\n"),
        "Host: example.invalid:54321\r\n".to_owned(),
        "Host: 192.0.2.020:54321\r\n".to_owned(),
        "Host: 192.0.2.20:154321\r\n".to_owned(),
        "Host: 192.0.2.20\r\n".to_owned(),
        format!("Host: {HOST}\r\nOrigin: null\r\n"),
        format!("Host: {HOST}\r\nOrigin: https://{HOST}\r\n"),
        format!("Host: {HOST}\r\nOrigin: http://{HOST}/\r\n"),
        format!("Host: {HOST}\r\nOrigin: http://evil.invalid:54321\r\n"),
        format!("Host: {HOST}\r\nOrigin: http://{HOST}\r\nOrigin: http://{HOST}\r\n"),
    ];
    for headers in invalid {
        let response = request(&format!("GET / HTTP/1.1\r\n{headers}\r\n"), None);
        assert!(response.starts_with("HTTP/1.1 403"), "{response}");
        assert!(response.contains("connection: close"));
        assert!(!response.contains("invalid"));
        assert!(!response.contains("Fixture page"));
    }
}

#[test]
fn advertised_aliases_require_current_generation_and_exact_origin_alias() {
    let host = format!("{NAME}:54321");
    let dotted = format!("{NAME}.:54321");
    let advertised = AdvertisedHost::for_generation(GENERATION, NAME, true).unwrap();
    let response = request(
        &format!(
            "GET / HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{}\r\n\r\n",
            host.to_ascii_uppercase()
        ),
        Some(advertised.clone()),
    );
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains(&format!("ws://{host};")));
    let response = request(
        &format!("GET / HTTP/1.1\r\nHost: {dotted}\r\nOrigin: http://{dotted}\r\n\r\n"),
        Some(advertised.clone()),
    );
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains(&format!("ws://{dotted};")));
    let response = request(
        &format!("GET / HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{dotted}\r\n\r\n"),
        Some(advertised),
    );
    assert!(response.starts_with("HTTP/1.1 403"));
    assert!(matches!(
        RequestAuthorities::new(
            HOST.parse().unwrap(),
            NonZeroU64::new(2).unwrap(),
            Some(&AdvertisedHost::for_generation(GENERATION, NAME, false).unwrap()),
        ),
        Err(AuthorityError::StaleGeneration)
    ));
}

#[test]
fn ipv6_canonical_forms_and_default_http_port_are_typed() {
    let scope = RequestAuthorities::new("[fd00::1]:80".parse().unwrap(), GENERATION, None).unwrap();
    let mut headers = http::HeaderMap::new();
    headers.insert(http::header::HOST, "[fd00:0:0:0:0:0:0:1]".parse().unwrap());
    headers.insert(http::header::ORIGIN, "http://[fd00::1]:80".parse().unwrap());
    assert!(scope.policy(&headers, false).is_some());
    headers.insert(http::header::ORIGIN, "http://[fd00::1]:81".parse().unwrap());
    assert!(scope.policy(&headers, false).is_none());
    headers.insert(http::header::HOST, "[fd00::1%1]".parse().unwrap());
    assert!(scope.policy(&headers, false).is_none());
}

#[test]
fn request_framing_rejects_ambiguous_lengths_bodies_and_proxy_targets() {
    let invalid = [
        format!("GET / HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: 3\r\n\r\nabc"),
        format!("GET / HTTP/1.1\r\nHost: {HOST}\r\nTransfer-Encoding: chunked\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {HOST}\r\nExpect: 100-continue\r\n\r\n"),
        format!("GET / HTTP/1.1\r\nHost: {HOST}\r\n Content-Length: 0\r\n\r\n"),
        format!("GET / HTTP/1.1\nHost: {HOST}\r\n\r\n"),
        format!("GET http://{HOST}/ HTTP/1.1\r\nHost: {HOST}\r\n\r\n"),
        format!("GET /#fragment HTTP/1.1\r\nHost: {HOST}\r\n\r\n"),
        format!("GET /?query HTTP/1.1\r\nHost: {HOST}\r\n\r\n"),
    ];
    for head in invalid {
        let response = request(&head, None);
        assert!(response.starts_with("HTTP/1.1 400"), "{response}");
        assert!(response.contains("connection: close"));
        assert!(!response.contains("Fixture page"));
    }
    assert!(
        request(
            &format!("GET / HTTP/1.1\r\nHost: {HOST}\r\nContent-Length: 0\r\n\r\n"),
            None
        )
        .starts_with("HTTP/1.1 200")
    );
    let method = request(
        &format!("POST / HTTP/1.1\r\nHost: {HOST}\r\nOrigin: http://{HOST}\r\n\r\n"),
        None,
    );
    assert!(method.starts_with("HTTP/1.1 405"));
    assert!(method.contains("allow: GET, HEAD"));
    assert!(
        request(
            &format!("GET /control HTTP/1.1\r\nHost: {HOST}\r\n\r\n"),
            None
        )
        .starts_with("HTTP/1.1 404")
    );
}

#[test]
fn control_upgrade_requires_the_exact_browser_authority_and_websocket_headers() {
    let origin = format!("Origin: http://{HOST}\r\n");
    let cookie = format!("Cookie: tether_device={}\r\n", "a".repeat(64));
    let head = format!(
        "GET /control HTTP/1.1\r\nHost: {HOST}\r\nConnection: keep-alive, Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: tether.control.v1\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n{origin}{cookie}\r\n"
    );
    let check = |head: &str, status| {
        let response = request(head, None);
        assert!(response.starts_with(&format!("HTTP/1.1 {status} ")));
        assert!(response.contains("cache-control: no-store\r\n"));
        assert!(response.contains("connection: close\r\n"));
        assert!(!response.contains("sec-websocket-accept"));
    };
    check(&head, 503);
    check(&head.replace(&origin, ""), 403);
    check(&head.replace(&origin, "Origin: null\r\n"), 403);
    check(
        &head.replace(&origin, "Origin: http://192.0.2.30:54321\r\n"),
        403,
    );
    check(&head.replace(&origin, &origin.repeat(2)), 403);
    check(
        &head.replace(&format!("Host: {HOST}"), "Host: other.local"),
        403,
    );
    check(&head.replace("/control", "/"), 400);
    check(&head.replace("GET ", "POST "), 400);
    check(&head.replace("HTTP/1.1", "HTTP/1.0"), 400);
    check(&head.replace("keep-alive, Upgrade", "close, Upgrade"), 400);
    check(
        &head.replace("keep-alive, Upgrade", "Upgrade, Upgrade"),
        400,
    );
    check(
        &head.replace(
            "Upgrade: websocket",
            "Upgrade: websocket\r\nUpgrade: websocket",
        ),
        400,
    );
    check(&head.replace("Version: 13", "Version: 12"), 400);
    check(
        &head.replace(
            "Protocol: tether.control.v1",
            "Protocol: tether.control.v1, other",
        ),
        400,
    );
    check(
        &head.replace("dGhlIHNhbXBsZSBub25jZQ==", "dGhlIHNhbXBsZSBub25jZR=="),
        400,
    );
    check(&head.replace(&cookie, ""), 401);
    check(&head.replace(&cookie, &cookie.repeat(2)), 401);
    check(
        &head.replace(&cookie, "Cookie: tether_device=invalid\r\n"),
        401,
    );
    check(
        &head.replace(
            &cookie,
            &format!(
                "Cookie: tether_device={0}; tether_device={0}\r\n",
                "a".repeat(64)
            ),
        ),
        401,
    );
    check(
        &head.replace(
            &cookie,
            &format!("Cookie: unrelated=v; tether_device={}\r\n", "a".repeat(64)),
        ),
        503,
    );
}

struct UpgradeProbe {
    available: bool,
    reserved: Arc<AtomicBool>,
    released: Arc<AtomicBool>,
}

struct ProbeSession(Arc<AtomicBool>);

impl ControlSession for ProbeSession {
    fn is_current(&self) -> bool {
        true
    }

    async fn run<I: AsyncRead + AsyncWrite + Unpin + Send>(
        &mut self,
        socket: &mut ControlSocket<'_, I>,
    ) {
        if let Ok(payload) = socket.receive().await {
            socket.send(payload.as_ref().to_vec()).await.unwrap();
        }
    }
}

impl Drop for ProbeSession {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl PairingHttpService for UpgradeProbe {
    type ControlProof = ();
    type Session = ProbeSession;

    fn handle(&self, _: PairingRequest) -> impl Future<Output = PairingReply> + Send {
        std::future::ready(PairingReply::Rejected(PairingRejection::Unavailable))
    }

    fn authorize_control(
        &self,
        request: ControlRequest,
    ) -> impl Future<Output = ControlAuthorization<((), Option<tether_config::DeviceCookie>)>> + Send
    {
        assert_eq!(request.peer, PEER);
        assert_eq!(request.authority.generation(), GENERATION);
        assert!(request.cookie().bytes().all(|byte| byte == b'a'));
        std::future::ready(ControlAuthorization::Verified(((), None)))
    }

    fn reserve_control(
        &self,
        (): (),
    ) -> impl Future<Output = ControlAuthorization<ProbeSession>> + Send {
        std::future::ready(if self.available {
            self.reserved.store(true, Ordering::Release);
            ControlAuthorization::Verified(ProbeSession(Arc::clone(&self.released)))
        } else {
            ControlAuthorization::RateLimited
        })
    }
}

#[test]
fn control_upgrade_writes_101_only_with_a_reservation_and_retains_prefetched_frames() {
    runtime().block_on(async {
        for (available, input) in [
            (false, [0x82, 0x82, 1, 2, 3, 4, b'o' ^ 1, b'k' ^ 2]),
            (true, [0x82, 0x82, 1, 2, 3, 4, b'o' ^ 1, b'k' ^ 2]),
            (true, [0x81, 0x82, 1, 2, 3, 4, b'o' ^ 1, b'k' ^ 2]),
            (true, [0x82, 0x02, b'o', b'k', 0, 0, 0, 0]),
        ] {
            let reserved = Arc::new(AtomicBool::new(false));
            let released = Arc::new(AtomicBool::new(false));
            let service = StaticHttpServer::with_pairing(
                server().assets,
                UpgradeProbe {
                    available,
                    reserved: Arc::clone(&reserved),
                    released: Arc::clone(&released),
                },
            );
            let (mut client, stream) = tokio::io::duplex(64 * 1024);
            let scope = RequestAuthorities::new(HOST.parse().unwrap(), GENERATION, None).unwrap();
            let admission = ConnectionAdmission::test_only(GENERATION);
            let server_task = service.serve(stream, &scope, PEER, Some(&admission), || true);
            let client_task = tokio::spawn(async move {
                let mut data = format!(
                    "GET /control HTTP/1.1\r\nHost: {HOST}\r\nOrigin: http://{HOST}\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: tether.control.v1\r\nSec-WebSocket-Extensions: permessage-deflate\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nCookie: tether_device={}\r\n\r\n",
                    "a".repeat(64)
                )
                .into_bytes();
                data.extend_from_slice(&input);
                client.write_all(&data).await.unwrap();
                let mut bytes = Vec::new();
                timeout(HEADER_TIMEOUT, client.read_to_end(&mut bytes)).await.unwrap().unwrap();
                bytes
            });
            assert_eq!(server_task.await, Ok(()));
            let bytes = client_task.await.unwrap();
            let end = bytes.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
            let (head, payload) = bytes.split_at(end);
            let head = std::str::from_utf8(head).unwrap();
            if available {
                assert!(head.starts_with("HTTP/1.1 101 "));
                assert!(head.contains("sec-websocket-accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"));
                assert!(head.contains("sec-websocket-protocol: tether.control.v1\r\n"));
                assert!(!head.contains("content-length"));
                assert!(!head.contains("sec-websocket-extensions"));
                assert_eq!(payload.starts_with(b"\x82\x02ok"), input[..2] == [0x82, 0x82]);
            } else {
                assert!(head.starts_with("HTTP/1.1 429 "));
                assert_eq!(payload, b"");
            }
            assert_eq!(reserved.load(Ordering::Acquire), available);
            assert_eq!(released.load(Ordering::Acquire), available);
        }
    });
}

#[test]
fn header_target_cookie_and_connection_limits_apply_on_wire() {
    let many = format!(
        "GET / HTTP/1.1\r\nHost: {HOST}\r\n{}\r\n",
        "X-Extra: a\r\n".repeat(32)
    );
    let large = format!(
        "GET / HTTP/1.1\r\nHost: {HOST}\r\nX-Extra: {}\r\n\r\n",
        "a".repeat(4_097)
    );
    let cookies = format!(
        "GET / HTTP/1.1\r\nHost: {HOST}\r\nCookie: {}\r\nCookie: a\r\n\r\n",
        "a".repeat(2_048)
    );
    let total = format!(
        "GET / HTTP/1.1\r\nHost: {HOST}\r\n{}\r\n",
        format!("X-Extra: {}\r\n", "a".repeat(4_000)).repeat(5)
    );
    for head in [many, large, cookies, total] {
        assert!(request(&head, None).starts_with("HTTP/1.1 431"));
    }
    let target = format!(
        "GET /{} HTTP/1.1\r\nHost: {HOST}\r\n\r\n",
        "a".repeat(1_024)
    );
    assert!(request(&target, None).starts_with("HTTP/1.1 414"));
    let pipeline = format!("GET / HTTP/1.1\r\nHost: {HOST}\r\n\r\n").repeat(MAX_REQUESTS + 1);
    let responses = request(&pipeline, None);
    assert_eq!(responses.matches("HTTP/1.1 200").count(), MAX_REQUESTS);
    assert_eq!(responses.matches("connection: close").count(), 1);
}

#[test]
fn partial_header_deadline_and_transport_revocation_are_bounded() {
    runtime().block_on(async {
        tokio::time::pause();
        let (mut client, stream) = tokio::io::duplex(4096);
        let task = tokio::spawn(async move {
            let scope = RequestAuthorities::new(HOST.parse().unwrap(), GENERATION, None).unwrap();
            server().serve(stream, &scope, PEER, None, || true).await
        });
        client.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(HEADER_TIMEOUT).await;
        let mut contents = String::new();
        client.read_to_string(&mut contents).await.unwrap();
        assert!(contents.starts_with("HTTP/1.1 408"));
        assert_eq!(task.await.unwrap(), Err(ServeError::Deadline));

        let active = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&active);
        let (mut client, stream) = tokio::io::duplex(4096);
        let task = tokio::spawn(async move {
            let scope = RequestAuthorities::new(HOST.parse().unwrap(), GENERATION, None).unwrap();
            server()
                .serve(stream, &scope, PEER, None, move || {
                    flag.load(Ordering::Acquire)
                })
                .await
        });
        client.write_all(b"GET / HTTP/1.1\r\n").await.unwrap();
        tokio::task::yield_now().await;
        active.store(false, Ordering::Release);
        client
            .write_all(format!("Host: {HOST}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut contents = String::new();
        client.read_to_string(&mut contents).await.unwrap();
        assert_eq!(contents, "");
        assert_eq!(task.await.unwrap(), Err(ServeError::Revoked));
    });
}
