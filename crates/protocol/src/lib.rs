//! Generated wire types and validated protocol-domain conversion boundary.
//!
//! Wire types alone do not validate bounds, capabilities, session state or
//! authorization. PR-004 owns that separate admission boundary. Never log wire
//! data or use raw generated decoding as permission to execute input.

/// Reproducibly generated, unvalidated version-one wire representation.
pub mod wire {
    include!("generated/tether.control.v1.rs");
}

#[cfg(test)]
mod tests {
    use super::wire;
    use prost::Message;

    #[test]
    fn encoding_roundtrips_text_and_full_width_counters_without_debug_disclosure() {
        let text = wire::TextInsertion {
            text: String::from("test-only \u{1f642} \u{65e5}\u{672c}"),
        };
        let action = wire::client_operation::Action::TextInsertion(text.clone());
        let envelope = wire::ClientEnvelope {
            protocol_major: 1,
            sequence: u64::MAX,
            session_epoch: u64::MAX - 1,
            payload: Some(wire::client_envelope::Payload::OperationBatch(
                wire::ClientOperationBatch {
                    operations: vec![wire::ClientOperation {
                        action: Some(action.clone()),
                        ..Default::default()
                    }],
                },
            )),
            ..Default::default()
        };
        let decoded = wire::ClientEnvelope::decode(envelope.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, envelope);
        for debug in [
            format!("{text:?}"),
            format!("{action:#?}"),
            format!("{envelope:#?}"),
        ] {
            assert_eq!(debug, "WireMessage(<redacted>)");
        }
        assert_eq!(
            format!("{:?}", wire::MouseButton::Left),
            "WireMessage(<redacted>)"
        );
    }

    #[test]
    fn atomic_click_flag_roundtrips_independently_of_positive_limit() {
        for available in [false, true] {
            let message = wire::CapabilitySnapshot {
                atomic_click_available: available,
                limits: Some(wire::ProtocolLimits {
                    maximum_atomic_click_count: 2,
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert_eq!(
                wire::CapabilitySnapshot::decode(message.encode_to_vec().as_slice()).unwrap(),
                message
            );
        }
    }

    #[test]
    fn malformed_wire_is_an_error_and_missing_payload_is_not_invented() {
        assert!(wire::ClientEnvelope::decode([0x80].as_slice()).is_err());
        assert!(
            wire::ClientEnvelope::decode([].as_slice())
                .unwrap()
                .payload
                .is_none()
        );
        // Unknown enum integers are retained at the wire boundary, not silently
        // promoted to a known/authorized operation. Domain rejection is PR-004.
        let action = wire::MediaCommandAction { command: 999 };
        let decoded = wire::MediaCommandAction::decode(action.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.command, 999);
        assert!(wire::MediaCommand::try_from(decoded.command).is_err());
    }
}
