//! Bounded, in-memory pairing state and credential digest verification.

mod approval;
mod identity;
mod secret;

pub use approval::{
    Approval, ApprovalDecision, ApprovalId, ApprovedPairing, CompletionPoll, Pairing, PairingError,
};
pub use identity::{DeviceLabel, HostScope, PairingIdentityError};
pub use secret::{CompletionCapability, CredentialDigest, QrToken};
