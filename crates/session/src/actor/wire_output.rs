use super::{ControlRole, ControlSnapshot, SessionError, SessionOutput};
use prost::Message;
use tether_protocol::wire::{self, server_envelope::Payload};

/// One writer's server sequence and negotiated envelope limit.
pub struct SessionOutputEncoder {
    sequence: u64,
    maximum_bytes: usize,
    identifier: [u8; 16],
}

impl SessionOutputEncoder {
    /// Uses an ephemeral identifier supplied by the trusted connection owner.
    #[must_use]
    pub const fn new(identifier: [u8; 16]) -> Self {
        Self {
            sequence: 0,
            maximum_bytes: 0,
            identifier,
        }
    }

    /// Encodes one output in removal order from the session actor. The
    /// first output must be hello, followed by strictly sequenced updates.
    ///
    /// # Errors
    /// Returns a closed error on invalid phase, sequence exhaustion or an
    /// output that does not fit the negotiated message budget.
    pub fn encode(&mut self, output: SessionOutput) -> Result<Vec<u8>, SessionError> {
        let next = self.sequence.checked_add(1).ok_or(SessionError::Closed)?;
        let (epoch, payload, maximum_bytes) = match output {
            SessionOutput::Hello {
                negotiation,
                capabilities,
                control,
                heartbeat,
            } if self.sequence == 0 => {
                let limits = negotiation.limits().to_wire();
                let maximum_bytes = limits.maximum_encoded_message_bytes as usize;
                let payload = Payload::ServerHello(wire::ServerHello {
                    selected_protocol_major: 1,
                    selected_protocol_minor: 0,
                    connection_identifier: self.identifier.to_vec(),
                    heartbeat_interval_millis: u32::try_from(heartbeat.interval.as_millis())
                        .map_err(|_| SessionError::InternalFault)?,
                    heartbeat_timeout_millis: u32::try_from(heartbeat.timeout.as_millis())
                        .map_err(|_| SessionError::InternalFault)?,
                    limits: Some(limits),
                    capabilities: Some(*capabilities),
                    control_state: Some(control_state(control, next)),
                });
                (0, payload, maximum_bytes)
            }
            SessionOutput::Control(control) if self.sequence > 0 => (
                control.epoch,
                Payload::ControlState(control_state(control, next)),
                self.maximum_bytes,
            ),
            SessionOutput::Pong { epoch, identifier } if self.sequence > 0 => (
                epoch,
                Payload::HeartbeatPong(wire::HeartbeatPong {
                    heartbeat_identifier: identifier,
                }),
                self.maximum_bytes,
            ),
            _ => return Err(SessionError::InvalidMessage),
        };
        let envelope = wire::ServerEnvelope {
            protocol_major: 1,
            protocol_minor: 0,
            sequence: self.sequence,
            session_epoch: epoch,
            payload: Some(payload),
        };
        if envelope.encoded_len() > maximum_bytes {
            return Err(SessionError::Capacity);
        }
        self.sequence = next;
        self.maximum_bytes = maximum_bytes;
        Ok(envelope.encode_to_vec())
    }
}

const fn control_state(control: ControlSnapshot, revision: u64) -> wire::ControlState {
    let state = match control.role {
        ControlRole::NoController => wire::ControlStateKind::NoController,
        ControlRole::Standby => wire::ControlStateKind::Standby,
        ControlRole::Controlling => wire::ControlStateKind::Controlling,
        ControlRole::Suspended => wire::ControlStateKind::Suspended,
    };
    wire::ControlState {
        revision,
        state: state as i32,
        session_epoch: control.epoch,
        this_client_controls: matches!(control.role, ControlRole::Controlling),
    }
}
