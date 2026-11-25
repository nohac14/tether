use super::authority::RequestAuthorities;
use super::control::{ControlAuthorization, ControlRequest, ControlSession};
use crate::assets::StaticResponsePolicy;
use crate::supervisor::ConnectionAdmission;
use http::header::{CACHE_CONTROL, CONNECTION, CONTENT_TYPE, HeaderValue, SET_COOKIE};
use http::{Method, Request, Response, StatusCode, Version};
use serde::Deserialize;
use std::future::Future;
use std::net::IpAddr;
use tether_config::DeviceCookie;
use tether_pairing::{CompletionCapability, DeviceLabel, HostScope};

pub(super) const MAX_BODY: usize = 4096;

/// A bounded request accepted by the LAN adapter. Inputs are deliberately
/// absent from diagnostics. The generation is revocable during queued work.
pub struct PairingRequest {
    pub authority: ConnectionAdmission,
    pub host: HostScope,
    pub peer: IpAddr,
    pub operation: PairingOperation,
}

pub enum PairingOperation {
    Submit { token: String, label: DeviceLabel },
    Complete { completion: String },
}

pub enum PairingReply {
    Pending,
    Submitted(CompletionCapability),
    Completed(DeviceCookie),
    Rejected(PairingRejection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingRejection {
    Gone,
    RateLimited,
    Unavailable,
}

/// Connects the HTTP route to the serialized local pairing service. The
/// implementation must make bounded admission and be cancellation-aware.
pub trait PairingHttpService {
    type ControlProof: Send;
    type Session: ControlSession;

    fn handle(&self, request: PairingRequest) -> impl Future<Output = PairingReply> + Send;

    fn authorize_control(
        &self,
        _request: ControlRequest,
    ) -> impl Future<Output = ControlAuthorization<(Self::ControlProof, Option<DeviceCookie>)>> + Send
    {
        std::future::ready(ControlAuthorization::Unavailable)
    }

    fn reserve_control(
        &self,
        _proof: Self::ControlProof,
    ) -> impl Future<Output = ControlAuthorization<Self::Session>> + Send {
        std::future::ready(ControlAuthorization::Unavailable)
    }
}

pub struct StaticOnly;

impl PairingHttpService for StaticOnly {
    type ControlProof = ();
    type Session = std::convert::Infallible;

    fn handle(&self, _: PairingRequest) -> impl Future<Output = PairingReply> + Send {
        std::future::ready(PairingReply::Rejected(PairingRejection::Unavailable))
    }
}

#[derive(Clone, Copy)]
pub(super) enum Route {
    Request,
    Complete,
}

impl Route {
    pub(super) fn from_head(request: &Request<&[u8]>) -> Option<Self> {
        if request.method() != Method::POST || request.version() != Version::HTTP_11 {
            return None;
        }
        match request.uri().path() {
            "/pairing/request" => Some(Self::Request),
            "/pairing/complete" => Some(Self::Complete),
            _ => None,
        }
    }

    pub(super) fn validate_header(request: &Request<&[u8]>) -> Result<(), StatusCode> {
        let mut values = request.headers().get_all(CONTENT_TYPE).iter();
        if values
            .next()
            .is_none_or(|value| !value.as_bytes().eq_ignore_ascii_case(b"application/json"))
            || values.next().is_some()
        {
            Err(StatusCode::UNSUPPORTED_MEDIA_TYPE)
        } else {
            Ok(())
        }
    }

    pub(super) fn decode(self, bytes: &[u8]) -> Result<PairingOperation, StatusCode> {
        let bad = StatusCode::BAD_REQUEST;
        if bytes.is_empty() || bytes.len() > MAX_BODY {
            return Err(bad);
        }
        match self {
            Self::Request => {
                let payload: Submit = serde_json::from_slice(bytes).map_err(|_| bad)?;
                let label = DeviceLabel::new(&payload.device_name).map_err(|_| bad)?;
                if !canonical_secret(&payload.token) {
                    return Err(bad);
                }
                Ok(PairingOperation::Submit {
                    token: payload.token,
                    label,
                })
            }
            Self::Complete => {
                let payload: Complete = serde_json::from_slice(bytes).map_err(|_| bad)?;
                if !canonical_secret(&payload.completion) {
                    return Err(bad);
                }
                Ok(PairingOperation::Complete {
                    completion: payload.completion,
                })
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Submit {
    token: String,
    device_name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Complete {
    completion: String,
}

fn canonical_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn response(policy: &StaticResponsePolicy, reply: PairingReply) -> Response<Vec<u8>> {
    let mut response = match reply {
        PairingReply::Pending => empty(policy, StatusCode::ACCEPTED),
        PairingReply::Submitted(capability) => {
            let mut response = empty(policy, StatusCode::ACCEPTED);
            let completion = capability.encode_for_response_body();
            *response.body_mut() = format!(r#"{{"completion":"{completion}"}}"#).into_bytes();
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            response
        }
        PairingReply::Completed(cookie) => {
            let Ok(header) = HeaderValue::from_str(cookie.as_set_cookie()) else {
                return empty(policy, StatusCode::SERVICE_UNAVAILABLE);
            };
            let mut response = empty(policy, StatusCode::OK);
            response.headers_mut().insert(SET_COOKIE, header);
            response
        }
        PairingReply::Rejected(reason) => empty(
            policy,
            match reason {
                PairingRejection::Gone => StatusCode::GONE,
                PairingRejection::RateLimited => StatusCode::TOO_MANY_REQUESTS,
                PairingRejection::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            },
        ),
    };
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(super) fn empty(policy: &StaticResponsePolicy, status: StatusCode) -> Response<Vec<u8>> {
    let mut response = policy.response(status, &[]).map(|_| Vec::new());
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    response
}

pub(super) fn admit(
    request: &Request<&[u8]>,
    authorities: &RequestAuthorities,
    admission: &ConnectionAdmission,
    peer: IpAddr,
    operation: PairingOperation,
) -> Option<PairingRequest> {
    if !admission.is_current() {
        return None;
    }
    Some(PairingRequest {
        host: authorities.pairing_host(request.headers())?.clone(),
        peer: peer.to_canonical(),
        authority: admission.clone(),
        operation,
    })
}
