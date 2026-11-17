use prost::Message;
use serde_json::{Value, json};
use std::fmt::Write as _;
use tether_protocol::{validation::ValidationError as Error, wire};
pub mod cases;

#[derive(Clone)]
pub enum Envelope {
    Client(wire::ClientEnvelope),
    Server(wire::ServerEnvelope),
}

impl Envelope {
    pub fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Client(value) => value.encode_to_vec(),
            Self::Server(value) => value.encode_to_vec(),
        }
    }
    pub fn direction(&self) -> &'static str {
        match self {
            Self::Client(_) => "client",
            Self::Server(_) => "server",
        }
    }
}

pub struct Case {
    pub id: String,
    pub hello: bool,
    pub bytes: Vec<u8>,
    pub expected: Option<Envelope>,
    pub admission: Result<(), Error>,
}

impl Case {
    pub fn new(
        id: impl Into<String>,
        hello: bool,
        value: Envelope,
        admission: Result<(), Error>,
    ) -> Self {
        Self {
            id: id.into(),
            hello,
            bytes: value.bytes(),
            expected: Some(value),
            admission,
        }
    }
    pub fn json(&self) -> Value {
        json!({ "id": self.id, "direction": self.expected.as_ref().map_or("client", Envelope::direction), "hello": self.hello,
            "wire_hex": hex(&self.bytes), "known_encoding_hex": self.expected.as_ref().map(|value| hex(&value.bytes())),
            "admission": self.admission.map_or_else(|error| format!("{error:?}"), |()| "ok".into()) })
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

pub fn document(cases: &[Case]) -> String {
    let mut result = serde_json::to_string_pretty(
        &json!({"version": 1, "cases": cases.iter().map(Case::json).collect::<Vec<_>>() }),
    )
    .unwrap();
    result.push('\n');
    result
}
