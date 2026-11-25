//! HTTP/1 wire bounds around the maintained head parser.

use http::header::{
    CONNECTION, CONTENT_ENCODING, CONTENT_LENGTH, COOKIE, DATE, EXPECT, TRANSFER_ENCODING, UPGRADE,
};
use http::{HeaderName, HeaderValue, Method, Request, Response, StatusCode, Uri, Version};
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub(super) const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_HEADERS: usize = 32;
const MAX_VALUE_BYTES: usize = 4 * 1024;
const MAX_COOKIE_BYTES: usize = 2 * 1024;
const MAX_TARGET_BYTES: usize = 1_024;
const END_OF_HEAD: &[u8] = b"\r\n\r\n";
const YEAR_10000_UNIX_SECONDS: u64 = 253_402_300_800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HeaderReadError {
    Rejected(StatusCode),
    Socket(io::ErrorKind),
}

pub(super) async fn read_head<I: AsyncRead + Unpin>(
    io: &mut I,
    buffer: &mut [u8],
    used: &mut usize,
) -> Result<Option<usize>, HeaderReadError> {
    let mut searched = 0;
    loop {
        if let Some(found) = buffer[searched..*used]
            .windows(END_OF_HEAD.len())
            .position(|bytes| bytes == END_OF_HEAD)
        {
            return Ok(Some(searched + found + END_OF_HEAD.len()));
        }
        if *used == buffer.len() {
            return Err(HeaderReadError::Rejected(
                StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE,
            ));
        }
        searched = used.saturating_sub(END_OF_HEAD.len() - 1);
        let size = io
            .read(&mut buffer[*used..])
            .await
            .map_err(|error| HeaderReadError::Socket(error.kind()))?;
        if size == 0 {
            return if *used == 0 {
                Ok(None)
            } else {
                Err(HeaderReadError::Rejected(StatusCode::BAD_REQUEST))
            };
        }
        *used += size;
    }
}

pub(super) struct Frame {
    pub request: Request<&'static [u8]>,
    pub close: bool,
    pub body_bytes: usize,
}

pub(super) fn parse_head(bytes: &[u8]) -> Result<Frame, StatusCode> {
    let bad = StatusCode::BAD_REQUEST;
    if bytes.first().is_none_or(u8::is_ascii_whitespace)
        || bytes.split_inclusive(|&byte| byte == b'\n').any(|line| {
            line.strip_suffix(b"\r\n")
                .is_none_or(|line| line.contains(&b'\r'))
        })
    {
        return Err(bad);
    }
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut head = httparse::Request::new(&mut headers);
    match head.parse(bytes) {
        Ok(httparse::Status::Complete(end)) if end == bytes.len() => {}
        Err(httparse::Error::TooManyHeaders) => {
            return Err(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
        }
        _ => return Err(bad),
    }
    let target = head.path.ok_or(bad)?;
    if target.len() > MAX_TARGET_BYTES {
        return Err(StatusCode::URI_TOO_LONG);
    }
    if !target.starts_with('/') || target.starts_with("//") || target.contains(['?', '#']) {
        return Err(bad);
    }
    let uri = target.parse::<Uri>().map_err(|_| bad)?;
    let method = Method::from_bytes(head.method.ok_or(bad)?.as_bytes()).map_err(|_| bad)?;
    let version = match head.version {
        Some(1) => Version::HTTP_11,
        Some(0) => Version::HTTP_10,
        _ => return Err(bad),
    };
    let mut request = Request::new([].as_slice());
    *request.method_mut() = method;
    *request.uri_mut() = uri;
    *request.version_mut() = version;
    let mut cookie_bytes = 0;
    let mut length = None;
    let mut close = version == Version::HTTP_10;
    for header in head.headers {
        if header.value.len() > MAX_VALUE_BYTES {
            return Err(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
        }
        let name = HeaderName::from_bytes(header.name.as_bytes()).map_err(|_| bad)?;
        match name {
            TRANSFER_ENCODING | CONTENT_ENCODING | EXPECT => return Err(bad),
            UPGRADE if target != super::control::PATH => return Err(bad),
            CONTENT_LENGTH => {
                if length.is_some() {
                    return Err(bad);
                }
                length = Some(content_length(header.value)?);
            }
            COOKIE => {
                cookie_bytes += header.value.len();
                if cookie_bytes > MAX_COOKIE_BYTES {
                    return Err(StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
                }
            }
            CONNECTION => {
                close |= header
                    .value
                    .split(|&byte| byte == b',')
                    .any(|token| token.trim_ascii().eq_ignore_ascii_case(b"close"));
            }
            _ => {}
        }
        // Preserve duplicates so authority checks can reject ambiguous values.
        let value = HeaderValue::from_bytes(header.value).map_err(|_| bad)?;
        request.headers_mut().append(name, value);
    }
    let body_bytes = length.unwrap_or_default();
    if super::pairing::Route::from_head(&request).is_some() {
        if body_bytes == 0 {
            return Err(StatusCode::LENGTH_REQUIRED);
        }
        if body_bytes > super::pairing::MAX_BODY {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
    } else if body_bytes != 0 {
        return Err(bad);
    }
    Ok(Frame {
        request,
        close,
        body_bytes,
    })
}

fn content_length(value: &[u8]) -> Result<usize, StatusCode> {
    let bad = StatusCode::BAD_REQUEST;
    if value.is_empty()
        || !value.iter().all(u8::is_ascii_digit)
        || (value.len() > 1 && value[0] == b'0')
    {
        return Err(bad);
    }
    if value.len() > 4 {
        return Ok(super::pairing::MAX_BODY + 1);
    }
    std::str::from_utf8(value)
        .map_err(|_| bad)?
        .parse()
        .map_err(|_| bad)
}

pub(super) async fn write_response<I: AsyncWrite + Unpin, B: AsRef<[u8]>>(
    io: &mut I,
    mut response: Response<B>,
) -> io::Result<()> {
    if !response.headers().contains_key(CONTENT_LENGTH)
        && response.status() != StatusCode::NOT_MODIFIED
        && !response.status().is_informational()
    {
        let length = HeaderValue::from(response.body().as_ref().len());
        response.headers_mut().insert(CONTENT_LENGTH, length);
    }
    let status = response.status();
    let now = SystemTime::now();
    if now
        .duration_since(UNIX_EPOCH)
        .is_ok_and(|elapsed| elapsed.as_secs() < YEAR_10000_UNIX_SECONDS)
        && let Ok(date) = HeaderValue::from_str(&httpdate::fmt_http_date(now))
    {
        response.headers_mut().insert(DATE, date);
    }
    let mut head = Vec::with_capacity(512);
    let version = if response.version() == Version::HTTP_10 {
        "HTTP/1.0"
    } else {
        "HTTP/1.1"
    };
    write!(
        head,
        "{version} {} {}\r\n",
        status.as_str(),
        status.canonical_reason().unwrap_or_default()
    )?;
    for (name, value) in response.headers() {
        head.extend_from_slice(name.as_str().as_bytes());
        head.extend_from_slice(b": ");
        head.extend_from_slice(value.as_bytes());
        head.extend_from_slice(b"\r\n");
    }
    head.extend_from_slice(b"\r\n");
    io.write_all(&head).await?;
    io.write_all(response.body().as_ref()).await?;
    io.flush().await?;
    Ok(())
}
