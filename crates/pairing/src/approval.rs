use crate::secret::{SECRET_BYTES, SecretDigest};
use crate::{CompletionCapability, DeviceLabel, HostScope, QrToken};
use std::fmt;
use std::net::IpAddr;
use std::time::{Duration, Instant};

const LIFETIME: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_APPROVALS: usize = 8;
const MAX_APPROVALS_PER_PEER: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingError {
    Unavailable,
    ApprovalLimit,
    PollTooSoon,
    EntropyUnavailable,
    InvalidClock,
    ApprovalIdExhausted,
}

impl fmt::Display for PairingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "pairing request is unavailable",
            Self::ApprovalLimit => "pairing approval limit reached",
            Self::PollTooSoon => "pairing completion polled too soon",
            Self::EntropyUnavailable => "pairing randomness unavailable",
            Self::InvalidClock => "pairing clock unavailable",
            Self::ApprovalIdExhausted => "pairing approval identifiers exhausted",
        })
    }
}

impl std::error::Error for PairingError {}

/// Local IPC identifier. Knowledge of this identifier grants no LAN authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ApprovalId(u64);

impl ApprovalId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Untrusted display metadata and local decision status for one approval.
pub struct Approval {
    id: ApprovalId,
    host: HostScope,
    peer: IpAddr,
    label: DeviceLabel,
    expires: Instant,
    approved: bool,
}

impl Approval {
    #[must_use]
    pub const fn id(&self) -> ApprovalId {
        self.id
    }

    #[must_use]
    pub const fn host(&self) -> &HostScope {
        &self.host
    }

    #[must_use]
    pub const fn peer(&self) -> IpAddr {
        self.peer
    }

    #[must_use]
    pub const fn label(&self) -> &DeviceLabel {
        &self.label
    }

    #[must_use]
    pub const fn expires_at(&self) -> Instant {
        self.expires
    }

    #[must_use]
    pub const fn is_approved(&self) -> bool {
        self.approved
    }
}

impl fmt::Debug for Approval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Approval(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

/// An approved, single-use claim for the device-store coordinator.
/// Successful durable storage is a separate requirement for cookie delivery.
#[derive(Debug)]
pub struct ApprovedPairing(Approval);

impl ApprovedPairing {
    #[must_use]
    pub const fn approval(&self) -> &Approval {
        &self.0
    }
}

#[derive(Debug)]
pub enum CompletionPoll {
    Pending,
    Approved(ApprovedPairing),
}

struct Issued {
    digest: SecretDigest,
    host: HostScope,
    expires: Instant,
}

struct Pending {
    approval: Approval,
    completion: SecretDigest,
    next_poll: Instant,
}

/// One serialized instance shared across approved listeners. Callers supply
/// monotonic server time and transport-validated host and source information.
/// Only the local core calls issuance, decision and invalidation methods.
#[derive(Default)]
pub struct Pairing {
    issued: Option<Issued>,
    pending: Vec<Pending>,
    last_id: u64,
    last_observed: Option<Instant>,
}

impl Pairing {
    /// Retires the old QR and issues one scoped to the selected browser host.
    ///
    /// # Errors
    /// Returns an error on invalid server time or unavailable entropy.
    pub fn issue_qr(&mut self, now: Instant, host: HostScope) -> Result<QrToken, PairingError> {
        self.issue_with(now, host, random)
    }

    /// Spends one QR and admits an approval with a new completion capability.
    ///
    /// # Errors
    /// Rejects unavailable QR codes, saturated approval slots, or unavailable
    /// time, randomness or local approval identifiers.
    pub fn submit(
        &mut self,
        now: Instant,
        host: &HostScope,
        peer: IpAddr,
        token: &str,
        label: DeviceLabel,
    ) -> Result<CompletionCapability, PairingError> {
        self.submit_with(now, host, peer, token, label, random)
    }

    /// Enumerates still-live approvals for the desktop IPC coordinator.
    ///
    /// # Errors
    /// Returns an error and clears volatile pairing state if time regresses.
    pub fn approvals(
        &mut self,
        now: Instant,
    ) -> Result<impl ExactSizeIterator<Item = &Approval>, PairingError> {
        self.expire(now)?;
        Ok(self.pending.iter().map(|item| &item.approval))
    }

    /// Records a local desktop decision. A denial retires the entire request.
    ///
    /// # Errors
    /// Rejects unknown or expired approvals and invalid server time.
    pub fn decide(
        &mut self,
        now: Instant,
        id: ApprovalId,
        decision: ApprovalDecision,
    ) -> Result<(), PairingError> {
        self.expire(now)?;
        let index = self
            .pending
            .iter()
            .position(|item| item.approval.id == id)
            .ok_or(PairingError::Unavailable)?;
        match decision {
            ApprovalDecision::Approve => self.pending[index].approval.approved = true,
            ApprovalDecision::Deny => {
                self.pending.remove(index);
            }
        }
        Ok(())
    }

    /// Checks the scoped completion capability and claims an approved entry
    /// once for the paired-device coordinator.
    ///
    /// # Errors
    /// Rejects invalid time, excess polling, and unavailable capabilities.
    pub fn poll_completion(
        &mut self,
        now: Instant,
        host: &HostScope,
        peer: IpAddr,
        completion: &str,
    ) -> Result<CompletionPoll, PairingError> {
        self.expire(now)?;
        let candidate = SecretDigest::from_encoded(completion).ok_or(PairingError::Unavailable)?;
        let index = self
            .pending
            .iter()
            .position(|item| {
                item.approval.host == *host
                    && item.approval.peer == peer.to_canonical()
                    && item.completion.matches(&candidate)
            })
            .ok_or(PairingError::Unavailable)?;
        let item = &mut self.pending[index];
        if now < item.next_poll {
            return Err(PairingError::PollTooSoon);
        }
        if item.approval.approved {
            let approved = self.pending.remove(index).approval;
            return Ok(CompletionPoll::Approved(ApprovedPairing(approved)));
        }
        let next = now
            .checked_add(POLL_INTERVAL)
            .ok_or(PairingError::InvalidClock)?;
        item.next_poll = next;
        Ok(CompletionPoll::Pending)
    }

    /// Retires all QR and completion capabilities on stop, rebind or reset.
    /// Keep this instance so local approval identifiers cannot be reused.
    pub fn invalidate(&mut self) {
        self.issued = None;
        self.pending.clear();
    }

    fn issue_with(
        &mut self,
        now: Instant,
        host: HostScope,
        entropy: impl FnOnce() -> Result<[u8; SECRET_BYTES], PairingError>,
    ) -> Result<QrToken, PairingError> {
        self.expire(now)?;
        let previous = self.issued.take();
        let expires = deadline(now)?;
        let value = entropy()?;
        let digest = SecretDigest::of(&value);
        if previous.is_some_and(|entry| entry.digest.matches(&digest))
            || self
                .pending
                .iter()
                .any(|item| item.completion.matches(&digest))
        {
            return Err(PairingError::EntropyUnavailable);
        }
        self.issued = Some(Issued {
            digest,
            host,
            expires,
        });
        Ok(QrToken(value))
    }

    fn submit_with(
        &mut self,
        now: Instant,
        host: &HostScope,
        peer: IpAddr,
        token: &str,
        label: DeviceLabel,
        entropy: impl FnOnce() -> Result<[u8; SECRET_BYTES], PairingError>,
    ) -> Result<CompletionCapability, PairingError> {
        self.expire(now)?;
        let candidate = SecretDigest::from_encoded(token).ok_or(PairingError::Unavailable)?;
        let issued = self.issued.as_ref().ok_or(PairingError::Unavailable)?;
        if issued.host != *host || !issued.digest.matches(&candidate) {
            return Err(PairingError::Unavailable);
        }
        let peer = peer.to_canonical();
        if self.pending.len() >= MAX_APPROVALS
            || self
                .pending
                .iter()
                .filter(|item| item.approval.peer == peer)
                .count()
                >= MAX_APPROVALS_PER_PEER
        {
            return Err(PairingError::ApprovalLimit);
        }
        let id = self
            .last_id
            .checked_add(1)
            .ok_or(PairingError::ApprovalIdExhausted)?;
        let expires = deadline(now)?;
        let value = entropy()?;
        let completion = SecretDigest::of(&value);
        if completion.matches(&candidate)
            || self
                .pending
                .iter()
                .any(|item| item.completion.matches(&completion))
        {
            return Err(PairingError::EntropyUnavailable);
        }

        self.issued = None;
        self.last_id = id;
        self.pending.push(Pending {
            approval: Approval {
                id: ApprovalId(id),
                host: host.clone(),
                peer,
                label,
                expires,
                approved: false,
            },
            completion,
            next_poll: now,
        });
        Ok(CompletionCapability(value))
    }

    fn expire(&mut self, now: Instant) -> Result<(), PairingError> {
        if self.last_observed.is_some_and(|prior| now < prior) {
            self.invalidate();
            return Err(PairingError::InvalidClock);
        }
        self.last_observed = Some(now);
        if self
            .issued
            .as_ref()
            .is_some_and(|entry| now >= entry.expires)
        {
            self.issued = None;
        }
        self.pending.retain(|item| now < item.approval.expires);
        Ok(())
    }
}

fn deadline(now: Instant) -> Result<Instant, PairingError> {
    now.checked_add(LIFETIME).ok_or(PairingError::InvalidClock)
}

fn random() -> Result<[u8; SECRET_BYTES], PairingError> {
    let mut value = [0; SECRET_BYTES];
    getrandom::fill(&mut value).map_err(|_| PairingError::EntropyUnavailable)?;
    Ok(value)
}

#[cfg(test)]
mod tests;
