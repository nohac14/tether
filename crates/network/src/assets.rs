//! Responses backed by an embedded mobile build manifest.
//!
//! The listener authenticates the request authority and validates framing
//! before handing a bodyless asset request to this module.

use http::header::{
    ALLOW, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH, TRANSFER_ENCODING,
};
use http::uri::Authority;
use http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

const MAX_ASSET_BYTES: usize = 4 * 1024 * 1024;
const MAX_BUNDLE_BYTES: usize = 16 * 1024 * 1024;
const MAX_ASSETS: usize = 256;
const MAX_KEY_BYTES: usize = 128;
const MAX_TARGET_BYTES: usize = 1_024;
const MAX_ETAG_HEADER_BYTES: usize = 4_096;
const CSP_PREFIX: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self' ws://";
const CSP_SUFFIX: &str =
    "; frame-ancestors 'none'; base-uri 'none'; form-action 'none'; object-src 'none'";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetError {
    EmptyRepresentation,
    SizeLimit,
    InvalidAssetKey,
    DuplicateAssetKey,
    InvalidAuthority,
    InvalidValidator,
}

impl fmt::Display for AssetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyRepresentation => "embedded representation is empty",
            Self::SizeLimit => "embedded bundle exceeds its limits",
            Self::InvalidAssetKey => "embedded asset key is invalid",
            Self::DuplicateAssetKey => "embedded asset key is duplicated",
            Self::InvalidAuthority => "static response authority is invalid",
            Self::InvalidValidator => "static response content validator is invalid",
        })
    }
}

impl std::error::Error for AssetError {}

/// Build-supplied bytes and exact key under `/assets/`.
pub struct EmbeddedFile {
    pub key: &'static str,
    pub bytes: &'static [u8],
}

struct Asset {
    bytes: &'static [u8],
    mime: &'static str,
    etag: HeaderValue,
}

/// A mobile entry and bounded, immutable embedded assets. No filesystem handle.
pub struct EmbeddedAssets {
    entry: &'static [u8],
    assets: BTreeMap<&'static str, Asset>,
}

/// Security headers built from a selected allowlist authority at bind time.
pub struct StaticResponsePolicy {
    csp: HeaderValue,
}

impl StaticResponsePolicy {
    /// Uses the approved binding-plan authority, never an incoming header.
    ///
    /// # Errors
    /// Rejects an invalid or unsafe WebSocket authority for the CSP.
    pub fn for_approved_authority(authority: &Authority) -> Result<Self, AssetError> {
        let value = authority.as_str();
        if authority.port_u16().is_none_or(|port| port == 0)
            || authority.host().is_empty()
            || value.len() > MAX_KEY_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".-:[]".contains(&byte))
        {
            return Err(AssetError::InvalidAuthority);
        }
        let csp = HeaderValue::from_str(&format!("{CSP_PREFIX}{value}{CSP_SUFFIX}"))
            .map_err(|_| AssetError::InvalidAuthority)?;
        Ok(Self { csp })
    }

    pub(crate) fn response(
        &self,
        status: StatusCode,
        bytes: &'static [u8],
    ) -> Response<&'static [u8]> {
        let mut response = Response::new(bytes);
        *response.status_mut() = status;
        let headers = response.headers_mut();
        headers.insert("content-security-policy", self.csp.clone());
        headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
        headers.insert(
            "x-content-type-options",
            HeaderValue::from_static("nosniff"),
        );
        headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
        headers.insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

impl EmbeddedAssets {
    /// Validates the build manifest once and computes strong content validators.
    ///
    /// # Errors
    /// Rejects empty or oversized data, unsafe keys, unknown MIME types and
    /// duplicate manifest entries.
    pub fn new(entry: &'static [u8], files: &[EmbeddedFile]) -> Result<Self, AssetError> {
        validate_size(entry)?;
        if files.len() > MAX_ASSETS {
            return Err(AssetError::SizeLimit);
        }
        let mut assets = BTreeMap::new();
        let mut total = entry.len();
        for file in files {
            validate_size(file.bytes)?;
            total += file.bytes.len();
            if total > MAX_BUNDLE_BYTES {
                return Err(AssetError::SizeLimit);
            }
            let mime = asset_mime(file.key).ok_or(AssetError::InvalidAssetKey)?;
            let asset = Asset {
                bytes: file.bytes,
                mime,
                etag: content_etag(file.bytes)?,
            };
            if assets.insert(file.key, asset).is_some() {
                return Err(AssetError::DuplicateAssetKey);
            }
        }
        Ok(Self { entry, assets })
    }

    /// Handles only `/` and exact manifest keys. Call after independent peer,
    /// Host/Origin and HTTP framing checks. The adapter must reject streaming
    /// request bodies before adapting requests to this interface.
    #[must_use]
    pub fn respond(
        &self,
        request: &Request<&[u8]>,
        policy: &StaticResponsePolicy,
    ) -> Response<&'static [u8]> {
        let uri = request.uri();
        let headers = request.headers();
        let path = uri.path();
        if uri.scheme().is_some()
            || uri.authority().is_some()
            || uri.query().is_some()
            || path.len() > MAX_TARGET_BYTES
            || !has_no_body(request)
        {
            return policy.response(StatusCode::BAD_REQUEST, &[]);
        }

        let asset = path
            .strip_prefix("/assets/")
            .and_then(|key| self.assets.get(key));
        if path != "/" && asset.is_none() {
            return policy.response(StatusCode::NOT_FOUND, &[]);
        }
        if !matches!(*request.method(), Method::GET | Method::HEAD) {
            let mut response = policy.response(StatusCode::METHOD_NOT_ALLOWED, &[]);
            response
                .headers_mut()
                .insert(ALLOW, HeaderValue::from_static("GET, HEAD"));
            return response;
        }
        let (bytes, mime) = asset.map_or((self.entry, "text/html; charset=utf-8"), |item| {
            (item.bytes, item.mime)
        });
        let status = if asset.is_some_and(|item| matches_etag(headers, &item.etag)) {
            StatusCode::NOT_MODIFIED
        } else {
            StatusCode::OK
        };
        let body = if status == StatusCode::OK && request.method() == Method::GET {
            bytes
        } else {
            &[]
        };
        let mut response = policy.response(status, body);
        let headers = response.headers_mut();
        if let Some(item) = asset {
            headers.insert(ETAG, item.etag.clone());
            headers.insert(
                CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
        }
        if status == StatusCode::OK {
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(mime));
            headers.insert(CONTENT_LENGTH, HeaderValue::from(bytes.len()));
        }
        response
    }
}

fn validate_size(bytes: &[u8]) -> Result<(), AssetError> {
    if bytes.is_empty() {
        return Err(AssetError::EmptyRepresentation);
    }
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(AssetError::SizeLimit);
    }
    Ok(())
}

fn asset_mime(key: &str) -> Option<&'static str> {
    if key.len() > MAX_KEY_BYTES
        || key.starts_with('.')
        || key.contains("..")
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    {
        return None;
    }
    let (stem, extension) = key.rsplit_once('.')?;
    // Vite's default `[name]-[hash]` filename with an eight-character hash.
    let (name, hash) = stem.split_at_checked(stem.len().checked_sub(8)?)?;
    if name.len() <= 1
        || !name.ends_with('-')
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
    {
        return None;
    }
    Some(match extension {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "png" => "image/png",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => return None,
    })
}

fn has_no_body(request: &Request<&[u8]>) -> bool {
    let headers = request.headers();
    let mut lengths = headers.get_all(CONTENT_LENGTH).iter();
    request.body().is_empty()
        && !headers.contains_key(TRANSFER_ENCODING)
        && lengths.next().is_none_or(|value| value == "0")
        && lengths.next().is_none()
}

fn content_etag(bytes: &[u8]) -> Result<HeaderValue, AssetError> {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut encoded = [b'"'; 66];
    let (pairs, _) = encoded[1..65].as_chunks_mut::<2>();
    for (pair, byte) in pairs.iter_mut().zip(Sha256::digest(bytes)) {
        *pair = [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]];
    }
    HeaderValue::from_bytes(&encoded).map_err(|_| AssetError::InvalidValidator)
}

fn matches_etag(headers: &HeaderMap, etag: &HeaderValue) -> bool {
    let all = headers.get_all(IF_NONE_MATCH);
    let mut values = all.iter().peekable();
    let Some(first) = values.next() else {
        return false;
    };
    if first.len() > MAX_ETAG_HEADER_BYTES {
        return false;
    }
    if first.as_bytes().trim_ascii() == b"*" {
        return values.peek().is_none();
    }
    let mut remaining = MAX_ETAG_HEADER_BYTES + 1;
    let mut matched = false;
    for value in std::iter::once(first).chain(values) {
        let bytes = value.as_bytes();
        let Some(next) = remaining.checked_sub(bytes.len().saturating_add(1)) else {
            return false;
        };
        remaining = next;
        let Some(in_value) = match_validator_list(bytes, etag.as_bytes()) else {
            return false;
        };
        matched |= in_value;
    }
    matched
}

fn match_validator_list(mut bytes: &[u8], etag: &[u8]) -> Option<bool> {
    let mut matched = false;
    loop {
        bytes = bytes.trim_ascii_start();
        if bytes.is_empty() {
            return Some(matched);
        }
        if let Some(rest) = bytes.strip_prefix(b",") {
            bytes = rest;
            continue;
        }
        let quoted = bytes.strip_prefix(b"W/").unwrap_or(bytes);
        let inner = quoted.strip_prefix(b"\"")?;
        let closing = inner.iter().position(|byte| *byte == b'"')?;
        if !inner[..closing]
            .iter()
            .all(|byte| *byte == b'!' || (0x23..=0x7e).contains(byte) || *byte >= 0x80)
        {
            return None;
        }
        let (tag, rest) = quoted.split_at(closing + 2);
        matched |= tag == etag;
        let rest = rest.trim_ascii_start();
        if rest.is_empty() {
            return Some(matched);
        }
        bytes = rest.strip_prefix(b",")?;
    }
}

#[cfg(test)]
mod tests;
