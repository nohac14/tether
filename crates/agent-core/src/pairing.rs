//! Serialized pairing, durable credential publication, and session revocation.

mod http;
pub use http::{PairingHttpBridge, PairingInbox, pairing_channel, pairing_channel_with_control};

use std::net::IpAddr;
use std::num::NonZeroU64;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tether_config::{
    DeviceCookie, DeviceId, DeviceRevision, DeviceRevocation, DeviceStoreStatus, MAX_DEVICES,
    SettingsStore, StorageError,
};
use tether_network::supervisor::{ConnectionAdmission, ConnectionAuthority};
use tether_pairing::{
    Approval, ApprovalDecision, ApprovalId, CompletionCapability, CompletionPoll, DeviceLabel,
    HostScope, Pairing, PairingError, QrToken,
};
use tether_session::actor::{
    CredentialAuthority, GlobalStop, SafetySignal, TransportGeneration, VerifiedConnection,
};

/// Live admission guard carried through verified HTTP and WebSocket handling.
pub trait PairingTransport {
    fn generation(&self) -> NonZeroU64;
    fn is_current(&self) -> bool;
}

impl<H> PairingTransport for ConnectionAuthority<H> {
    fn generation(&self) -> NonZeroU64 {
        self.generation()
    }

    fn is_current(&self) -> bool {
        self.is_current()
    }
}

impl PairingTransport for ConnectionAdmission {
    fn generation(&self) -> NonZeroU64 {
        self.generation()
    }
    fn is_current(&self) -> bool {
        self.is_current()
    }
}

/// Constructed by the transport bridge after peer, Host, and Origin admission.
/// The socket lease is checked again at every commit/publication boundary.
pub struct PairingPeer<'a> {
    pub transport: &'a dyn PairingTransport,
    pub host: &'a HostScope,
    pub address: IpAddr,
}

#[derive(Clone, Copy)]
pub struct PairingTime {
    pub monotonic: Instant,
    pub wall: SystemTime,
}

#[derive(Debug)]
pub enum PairingServiceError {
    StaleTransport,
    Unavailable,
    Pairing(PairingError),
    Storage(StorageError),
}

impl From<PairingError> for PairingServiceError {
    fn from(value: PairingError) -> Self {
        Self::Pairing(value)
    }
}

impl From<StorageError> for PairingServiceError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Completion {
    Pending,
    Published,
}

/// Run on the serialized storage service. The session actor remains available
/// to poll the reserved safety signals while a filesystem operation is running.
pub struct PairingCoordinator {
    pairing: Pairing,
    generation: Option<NonZeroU64>,
    listening: bool,
    credentials: Vec<(DeviceId, CredentialAuthority)>,
    safety: SafetySignal,
}

impl PairingCoordinator {
    #[must_use]
    pub fn new(safety: SafetySignal) -> Self {
        Self {
            pairing: Pairing::default(),
            generation: None,
            listening: false,
            credentials: Vec::new(),
            safety,
        }
    }

    /// Installs a newly published listener generation from the supervisor.
    ///
    /// # Errors
    /// Rejects a reused or retired generation.
    pub fn activate_transport(
        &mut self,
        generation: NonZeroU64,
    ) -> Result<(), PairingServiceError> {
        if self.listening || self.generation.is_some_and(|current| generation <= current) {
            return Err(PairingServiceError::StaleTransport);
        }
        self.pairing.invalidate();
        self.generation = Some(generation);
        self.listening = true;
        Ok(())
    }

    pub fn retire_transport(&mut self, generation: NonZeroU64) {
        if self.listening && self.generation == Some(generation) {
            self.listening = false;
            self.pairing.invalidate();
            self.safety.request(GlobalStop::RetireTransport);
        }
    }

    /// Local operation. `host` comes from the supervisor's published addresses.
    ///
    /// # Errors
    /// Rejects stale transport authority or unavailable pairing/storage state.
    pub fn issue_qr(
        &mut self,
        store: &SettingsStore,
        generation: NonZeroU64,
        now: Instant,
        host: HostScope,
    ) -> Result<QrToken, PairingServiceError> {
        self.check_generation(generation)?;
        ready_revision(store)?;
        Ok(self.pairing.issue_qr(now, host)?)
    }

    /// Called only after the HTTP adapter admits a pairing request.
    ///
    /// # Errors
    /// Rejects unavailable pairing state, stale authority or invalid claims.
    pub fn submit(
        &mut self,
        store: &SettingsStore,
        peer: &PairingPeer<'_>,
        now: Instant,
        token: &str,
        label: DeviceLabel,
    ) -> Result<CompletionCapability, PairingServiceError> {
        self.check_peer(peer)?;
        ready_revision(store)?;
        Ok(self
            .pairing
            .submit(now, peer.host, peer.address, token, label)?)
    }

    /// Local IPC can show each approval's untrusted label, numeric source,
    /// exact host and expiry. The iterator is bounded by the approval service.
    ///
    /// # Errors
    /// Rejects a stale dialog or regressed monotonic clock.
    pub fn approvals(
        &mut self,
        generation: NonZeroU64,
        now: Instant,
    ) -> Result<impl ExactSizeIterator<Item = &Approval>, PairingServiceError> {
        self.check_generation(generation)?;
        Ok(self.pairing.approvals(now)?)
    }

    /// Local IPC decision with a generation fence around stale dialogs.
    ///
    /// # Errors
    /// Rejects missing, expired or stale approval state.
    pub fn decide(
        &mut self,
        generation: NonZeroU64,
        now: Instant,
        id: ApprovalId,
        decision: ApprovalDecision,
    ) -> Result<(), PairingServiceError> {
        self.check_generation(generation)?;
        Ok(self.pairing.decide(now, id, decision)?)
    }

    /// Checks a completion and publishes a cookie only through a nonblocking
    /// response reservation. A false `publish` result means no caller can
    /// deliver or retain the response. The writer still checks its socket lease
    /// before sending accepted output.
    ///
    /// # Errors
    /// Reports lease loss, invalid pairing state, storage errors and output
    /// cancellation. Unpublished credentials are removed before reopening.
    pub fn complete(
        &mut self,
        store: &mut SettingsStore,
        peer: &PairingPeer<'_>,
        completion: &str,
        mut clock: impl FnMut() -> PairingTime,
        publish: impl FnOnce(DeviceCookie) -> bool,
    ) -> Result<Completion, PairingServiceError> {
        // Exclusive access reserves the sole storage mutation slot before claim.
        let revision = ready_revision(store)?;
        self.check_peer(peer)?;
        let polled = clock();
        let claimed =
            self.pairing
                .poll_completion(polled.monotonic, peer.host, peer.address, completion)?;
        let CompletionPoll::Approved(approval) = claimed else {
            return Ok(Completion::Pending);
        };
        self.check_peer(peer)?;
        let admitted = clock();
        if admitted.monotonic < polled.monotonic || admitted.wall < polled.wall {
            return Err(PairingServiceError::Unavailable);
        }
        self.check_peer(peer)?;
        let enrollment =
            store.enroll_device(revision, approval, admitted.monotonic, admitted.wall)?;
        let device = enrollment.device_id();
        let delivered = (|| {
            let finished = clock();
            if finished.monotonic < admitted.monotonic || finished.wall < admitted.wall {
                return Err(PairingServiceError::Unavailable);
            }
            self.check_peer(peer)?;
            let cookie =
                store.deliver_device_cookie(enrollment, finished.monotonic, finished.wall)?;
            self.check_peer(peer)?;
            if !publish(cookie) {
                return Err(PairingServiceError::Unavailable);
            }
            Ok(Completion::Published)
        })();
        if delivered.is_err() {
            let revision = ready_revision(store)?;
            self.revoke(store, Some(revision), DeviceRevocation::Device(device))?;
        }
        delivered
    }

    /// Converts an exact host-scoped cookie into a session registration proof.
    /// `elapsed` uses the session actor's monotonic origin. A queued proof stays
    /// fenced by its shared credential authority after this call returns.
    #[must_use]
    pub fn verify_control(
        &mut self,
        store: &SettingsStore,
        peer: &PairingPeer<'_>,
        cookie: &str,
        wall: SystemTime,
        elapsed: Duration,
    ) -> Option<VerifiedConnection<DeviceId>> {
        self.check_peer(peer).ok()?;
        let device = store.verify_device_cookie(peer.host, cookie, wall)?;
        let expires =
            UNIX_EPOCH.checked_add(Duration::from_secs(device.expires_at_unix_seconds()))?;
        let expiry = elapsed.checked_add(expires.duration_since(wall).ok()?)?;
        self.check_peer(peer).ok()?;
        let authority = self.credential(store, device.id())?;
        Some(VerifiedConnection {
            device: device.id(),
            authority_revision: NonZeroU64::new(device.revision().get())?,
            generation: TransportGeneration(peer.transport.generation()),
            expires_at: expiry,
            credential: authority,
        })
    }

    /// Renews a valid credential while its admitted transport is current.
    ///
    /// # Errors
    /// Returns a closed storage error when the renewal cannot be committed.
    pub fn renew_control(
        &mut self,
        store: &mut SettingsStore,
        peer: &PairingPeer<'_>,
        cookie: &str,
        wall: SystemTime,
        elapsed: Duration,
    ) -> Result<Option<(VerifiedConnection<DeviceId>, DeviceCookie)>, StorageError> {
        if self.check_peer(peer).is_err() {
            return Ok(None);
        }
        let Some(renewal) = store.renew_device_cookie(peer.host, cookie, wall)? else {
            return Ok(None);
        };
        Ok(self
            .verify_control(store, peer, cookie, wall, elapsed)
            .map(|proof| (proof, renewal)))
    }

    /// Closes verification and notifies the session actor's reserved path
    /// before the durable revocation is attempted. A failed write keeps the
    /// storage gate closed and old session proofs revoked.
    ///
    /// # Errors
    /// Returns a stale revision, unavailable storage or durable commit failure.
    pub fn revoke(
        &mut self,
        store: &mut SettingsStore,
        expected: Option<DeviceRevision>,
        scope: DeviceRevocation,
    ) -> Result<DeviceRevision, PairingServiceError> {
        let request = store.begin_device_revocation(expected, scope)?;
        match request.scope() {
            DeviceRevocation::Device(device) => {
                if let Some(index) = self.credentials.iter().position(|(id, _)| *id == device) {
                    let (_, authority) = self.credentials.swap_remove(index);
                    authority.revoke();
                }
            }
            DeviceRevocation::All => {
                self.pairing.invalidate();
                self.close_credentials();
                self.safety.request(GlobalStop::RevokeDevices);
            }
        }
        Ok(store.commit_device_revocation(request)?)
    }

    fn credential(
        &mut self,
        store: &SettingsStore,
        device: DeviceId,
    ) -> Option<CredentialAuthority> {
        if let Some((_, authority)) = self.credentials.iter().find(|(id, _)| *id == device) {
            return Some(authority.clone());
        }
        if self.credentials.len() == MAX_DEVICES {
            let current = store.devices_snapshot()?.devices();
            self.credentials.retain(|(id, authority)| {
                let present = current.iter().any(|record| record.id() == *id);
                if !present {
                    authority.revoke();
                }
                present
            });
            if self.credentials.len() == MAX_DEVICES {
                return None;
            }
        }
        let authority = CredentialAuthority::active();
        self.credentials.push((device, authority.clone()));
        Some(authority)
    }

    fn check_generation(&self, generation: NonZeroU64) -> Result<(), PairingServiceError> {
        if self.listening && self.generation == Some(generation) {
            Ok(())
        } else {
            Err(PairingServiceError::StaleTransport)
        }
    }

    fn check_peer(&self, peer: &PairingPeer<'_>) -> Result<(), PairingServiceError> {
        self.check_generation(peer.transport.generation())?;
        if peer.transport.is_current() {
            Ok(())
        } else {
            Err(PairingServiceError::StaleTransport)
        }
    }

    fn close_credentials(&mut self) {
        for (_, authority) in self.credentials.drain(..) {
            authority.revoke();
        }
    }
}

impl Drop for PairingCoordinator {
    fn drop(&mut self) {
        self.close_credentials();
    }
}

fn ready_revision(store: &SettingsStore) -> Result<DeviceRevision, PairingServiceError> {
    if store.device_store_status() != DeviceStoreStatus::Ready {
        return Err(PairingServiceError::Unavailable);
    }
    store
        .devices_snapshot()
        .map(tether_config::DevicesSnapshot::revision)
        .ok_or(PairingServiceError::Unavailable)
}

#[cfg(test)]
mod tests;
