use super::{Completion, PairingCoordinator, PairingPeer, PairingServiceError, PairingTime};
use crate::control::{self, ActorSocket, ControlBridge, ControlInbox};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::{Instant, SystemTime};
use tether_config::{DeviceCookie, DeviceId, SettingsStore};
use tether_network::http1::{
    ControlAuthorization, ControlRequest, PairingHttpService, PairingOperation, PairingRejection,
    PairingReply, PairingRequest,
};
use tether_pairing::PairingError;
use tether_session::actor::VerifiedConnection;
use tokio::sync::oneshot;

const MAX_QUEUE: usize = 8;

struct Work {
    request: PairingRequest,
    response: oneshot::Sender<PairingReply>,
}

type ControlReply = ControlAuthorization<(VerifiedConnection<DeviceId>, Option<DeviceCookie>)>;

struct ControlWork {
    request: ControlRequest,
    response: oneshot::Sender<ControlReply>,
}

#[derive(Clone)]
pub struct PairingHttpBridge {
    pairing: SyncSender<Work>,
    control: SyncSender<ControlWork>,
    sessions: Option<ControlBridge>,
}

/// Owned by the serialized store service, independently of the input actor.
pub struct PairingInbox {
    pairing: Receiver<Work>,
    control: Receiver<ControlWork>,
    actor_clock_origin: Instant,
}

#[must_use]
pub fn pairing_channel(actor_clock_origin: Instant) -> (PairingHttpBridge, PairingInbox) {
    let (pairing_sender, pairing) = mpsc::sync_channel(MAX_QUEUE);
    let (control_sender, control) = mpsc::sync_channel(MAX_QUEUE);
    (
        PairingHttpBridge {
            pairing: pairing_sender,
            control: control_sender,
            sessions: None,
        },
        PairingInbox {
            pairing,
            control,
            actor_clock_origin,
        },
    )
}

/// Connects the protected store and session actor to the admitted HTTP server.
/// Both inboxes use the same actor monotonic origin.
#[must_use]
pub fn pairing_channel_with_control(
    actor_clock_origin: Instant,
) -> (PairingHttpBridge, PairingInbox, ControlInbox) {
    let (mut bridge, pairing) = pairing_channel(actor_clock_origin);
    let (sessions, control) = control::channel(actor_clock_origin);
    bridge.sessions = Some(sessions);
    (bridge, pairing, control)
}

impl PairingHttpService for PairingHttpBridge {
    type ControlProof = VerifiedConnection<DeviceId>;
    type Session = ActorSocket;

    async fn handle(&self, request: PairingRequest) -> PairingReply {
        let (response, result) = oneshot::channel();
        match self.pairing.try_send(Work { request, response }) {
            Ok(()) => result
                .await
                .unwrap_or(PairingReply::Rejected(PairingRejection::Unavailable)),
            Err(TrySendError::Full(_)) => PairingReply::Rejected(PairingRejection::RateLimited),
            Err(TrySendError::Disconnected(_)) => {
                PairingReply::Rejected(PairingRejection::Unavailable)
            }
        }
    }

    async fn authorize_control(&self, request: ControlRequest) -> ControlReply {
        let (response, result) = oneshot::channel();
        match self.control.try_send(ControlWork { request, response }) {
            Ok(()) => result.await.unwrap_or(ControlAuthorization::Unavailable),
            Err(TrySendError::Full(_)) => ControlAuthorization::RateLimited,
            Err(TrySendError::Disconnected(_)) => ControlAuthorization::Unavailable,
        }
    }

    async fn reserve_control(
        &self,
        proof: Self::ControlProof,
    ) -> ControlAuthorization<Self::Session> {
        match &self.sessions {
            Some(sessions) => sessions.reserve(proof).await,
            None => ControlAuthorization::Unavailable,
        }
    }
}

impl PairingInbox {
    /// Processes at most one queued request. All storage IO remains on this
    /// caller's thread. Retired and cancelled requests fail before mutation.
    pub fn process_one(
        &self,
        coordinator: &mut PairingCoordinator,
        store: &mut SettingsStore,
    ) -> bool {
        let Ok(work) = self.pairing.try_recv() else {
            return false;
        };
        Self::process(work, coordinator, store);
        true
    }

    /// Processes at most one control check. The composition root can poll
    /// both queues every tick so credential checks cannot starve pairing.
    pub fn process_control_one(
        &self,
        coordinator: &mut PairingCoordinator,
        store: &mut SettingsStore,
    ) -> bool {
        let Ok(ControlWork { request, response }) = self.control.try_recv() else {
            return false;
        };
        if response.is_closed() {
            return true;
        }
        let peer = PairingPeer {
            transport: &request.authority,
            host: &request.host,
            address: request.peer,
        };
        let wall = SystemTime::now();
        let reply =
            if let Some(elapsed) = Instant::now().checked_duration_since(self.actor_clock_origin) {
                match coordinator.renew_control(store, &peer, request.cookie(), wall, elapsed) {
                    Ok(Some((proof, renewal))) => {
                        ControlAuthorization::Verified((proof, Some(renewal)))
                    }
                    Ok(None) => ControlAuthorization::Rejected,
                    Err(_) => ControlAuthorization::Unavailable,
                }
            } else {
                ControlAuthorization::Unavailable
            };
        let _cancelled = response.send(reply);
        true
    }

    fn process(work: Work, coordinator: &mut PairingCoordinator, store: &mut SettingsStore) {
        if work.response.is_closed() {
            return;
        }
        let PairingRequest {
            authority,
            host,
            peer,
            operation,
        } = work.request;
        let peer = PairingPeer {
            transport: &authority,
            host: &host,
            address: peer,
        };
        match operation {
            PairingOperation::Submit { token, label } => {
                let reply = coordinator
                    .submit(store, &peer, Instant::now(), &token, label)
                    .map_or_else(|error| reject(&error), PairingReply::Submitted);
                let _cancelled = work.response.send(reply);
            }
            PairingOperation::Complete { completion } => {
                let mut response = Some(work.response);
                let result = coordinator.complete(
                    store,
                    &peer,
                    &completion,
                    || PairingTime {
                        monotonic: Instant::now(),
                        wall: SystemTime::now(),
                    },
                    |cookie| {
                        response.take().is_some_and(|reply| {
                            reply.send(PairingReply::Completed(cookie)).is_ok()
                        })
                    },
                );
                if let Some(response) = response {
                    let reply = match result {
                        Ok(Completion::Pending) => PairingReply::Pending,
                        Ok(Completion::Published) => {
                            PairingReply::Rejected(PairingRejection::Unavailable)
                        }
                        Err(error) => reject(&error),
                    };
                    let _cancelled = response.send(reply);
                }
            }
        }
    }
}

fn reject(error: &PairingServiceError) -> PairingReply {
    PairingReply::Rejected(match error {
        PairingServiceError::Pairing(PairingError::Unavailable) => PairingRejection::Gone,
        PairingServiceError::Pairing(PairingError::PollTooSoon | PairingError::ApprovalLimit) => {
            PairingRejection::RateLimited
        }
        _ => PairingRejection::Unavailable,
    })
}
