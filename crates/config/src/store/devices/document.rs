use super::super::{EnvelopeHeader, StorageError, StorageProblem, document_error, parse_json};
use super::{
    CREDENTIAL_LIFETIME_SECONDS, DeviceId, DeviceRevision, DevicesSnapshot, FILE_LIMIT, FORMAT,
    MAX_DEVICES, PairedDevice, SCHEMA_VERSION, invalid_document, unsupported_schema,
};
use crate::identity::{decode_lower_hex, encode_lower_hex};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::net::IpAddr;
use tether_pairing::{CredentialDigest, DeviceLabel, HostScope};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    format: String,
    schema_version: u32,
    revision: u64,
    payload: Payload,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Payload {
    installation_id: String,
    #[serde(deserialize_with = "bounded_records")]
    devices: Vec<DeviceEntry>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DeviceEntry {
    id: String,
    label: String,
    host: String,
    credential_sha256: String,
    issued_at: u64,
    expires_at: u64,
    last_seen: Option<u64>,
}

fn bounded_records<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<DeviceEntry>, D::Error> {
    struct Entries;

    impl<'de> Visitor<'de> for Entries {
        type Value = Vec<DeviceEntry>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a bounded list of paired devices")
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut records = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(MAX_DEVICES));
            while let Some(record) = seq.next_element()? {
                if records.len() == MAX_DEVICES {
                    return Err(de::Error::custom("paired-device count exceeds limit"));
                }
                records.push(record);
            }
            Ok(records)
        }
    }

    deserializer.deserialize_seq(Entries)
}

pub(super) fn encode_snapshot(
    snapshot: &DevicesSnapshot,
    installation: &str,
) -> Result<Vec<u8>, StorageError> {
    let payload = Payload {
        installation_id: installation.to_owned(),
        devices: snapshot
            .records
            .iter()
            .map(|device| DeviceEntry {
                id: device.id.for_local_admin(),
                label: device.label.for_display().to_owned(),
                host: device.host.host_name(),
                credential_sha256: encode_lower_hex(device.digest.as_stored_sha256()),
                issued_at: device.issued_at,
                expires_at: device.expires_at,
                last_seen: device.last_seen,
            })
            .collect(),
    };
    let envelope = Envelope {
        format: FORMAT.to_owned(),
        schema_version: SCHEMA_VERSION,
        revision: snapshot.revision.get(),
        payload,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|_| invalid_document())?;
    check_size(&bytes)?;
    Ok(bytes)
}

pub(super) fn decode_snapshot(
    bytes: &[u8],
    installation: &str,
) -> Result<DevicesSnapshot, StorageError> {
    check_size(bytes)?;
    let header: EnvelopeHeader = parse_json(bytes)?;
    if header.format != FORMAT || header.revision == 0 || header.schema_version == 0 {
        return Err(invalid_document());
    }
    if !matches!(header.schema_version, 1 | SCHEMA_VERSION) {
        return Err(unsupported_schema(header.schema_version));
    }
    let envelope: Envelope = parse_json(bytes)?;
    if envelope.payload.installation_id != installation {
        return Err(invalid_document());
    }
    let mut records: Vec<PairedDevice> = Vec::with_capacity(envelope.payload.devices.len());
    for entry in &envelope.payload.devices {
        let record = decode_device(entry, header.schema_version)?;
        if records
            .iter()
            .any(|other| other.id == record.id || other.digest.matches_digest(&record.digest))
        {
            return Err(invalid_document());
        }
        records.push(record);
    }
    Ok(DevicesSnapshot {
        revision: DeviceRevision(envelope.revision),
        records,
    })
}

fn decode_device(entry: &DeviceEntry, version: u32) -> Result<PairedDevice, StorageError> {
    let id = decode_lower_hex::<16>(&entry.id)
        .filter(|id| *id != [0; 16])
        .map(DeviceId)
        .ok_or_else(invalid_document)?;
    let host = entry.host.parse::<IpAddr>().map_or_else(
        |_| HostScope::local(&entry.host).map_err(|_| invalid_document()),
        |address| Ok(HostScope::ip(address)),
    )?;
    let label = DeviceLabel::new(&entry.label).map_err(|_| invalid_document())?;
    let digest = decode_lower_hex::<32>(&entry.credential_sha256)
        .map(CredentialDigest::from_stored_sha256)
        .ok_or_else(invalid_document)?;
    let original_expiry = entry.issued_at.checked_add(CREDENTIAL_LIFETIME_SECONDS);
    let renewed_expiry = entry
        .last_seen
        .and_then(|seen| seen.checked_add(CREDENTIAL_LIFETIME_SECONDS));
    if host.host_name() != entry.host
        || (original_expiry != Some(entry.expires_at)
            && (version == 1 || renewed_expiry != Some(entry.expires_at)))
        || entry
            .last_seen
            .is_some_and(|seen| !(entry.issued_at..entry.expires_at).contains(&seen))
    {
        return Err(invalid_document());
    }
    Ok(PairedDevice {
        id,
        label,
        host,
        digest,
        issued_at: entry.issued_at,
        expires_at: entry.expires_at,
        last_seen: entry.last_seen,
    })
}

fn check_size(bytes: &[u8]) -> Result<(), StorageError> {
    if bytes.len() > FILE_LIMIT {
        return Err(document_error(StorageProblem::EncodedLimitExceeded));
    }
    Ok(())
}
