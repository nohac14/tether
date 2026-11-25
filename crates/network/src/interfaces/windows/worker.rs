//! One bounded native request at a time. A late reply can never grant authority.

use super::{InterfaceError, WindowsRecord};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::task::Poll;
use std::thread;
use std::time::{Duration, Instant};

pub(super) type Sample = Result<Vec<WindowsRecord>, InterfaceError>;

struct Refresh {
    deadline: Instant,
    submitted: bool,
}

pub(super) struct SampleWorker {
    requests: SyncSender<SyncSender<Sample>>,
    reply: Option<Receiver<Sample>>,
    refresh: Option<Refresh>,
}

impl SampleWorker {
    pub(super) fn start(
        mut collect: impl FnMut() -> Sample + Send + 'static,
    ) -> Result<Self, InterfaceError> {
        let (requests, work) = sync_channel::<SyncSender<Sample>>(1);
        // The worker exits when its sole request sender drops. An in-flight OS
        // call may finish later, so dropping the provider never joins it.
        thread::Builder::new()
            .name("tether-windows-interface-inventory".to_owned())
            .spawn(move || {
                while let Ok(reply) = work.recv() {
                    let _ = reply.try_send(collect());
                }
            })
            .map_err(|_| InterfaceError::ProviderUnavailable)?;
        Ok(Self {
            requests,
            reply: None,
            refresh: None,
        })
    }

    pub(super) fn begin(&mut self, now: Instant, timeout: Duration) -> Result<(), InterfaceError> {
        if self.refresh.is_some() {
            return Err(InterfaceError::RefreshInProgress);
        }
        let deadline = now
            .checked_add(timeout)
            .ok_or(InterfaceError::ProviderTimedOut)?;
        self.refresh = Some(Refresh {
            deadline,
            submitted: false,
        });
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.refresh = None;
    }

    pub(super) fn poll(&mut self, now: Instant) -> Poll<Sample> {
        let Some(mut refresh) = self.refresh.take() else {
            return Poll::Ready(Err(InterfaceError::RefreshNotStarted));
        };
        if now >= refresh.deadline {
            return Poll::Ready(Err(InterfaceError::ProviderTimedOut));
        }
        if let Some(reply) = &self.reply {
            match reply.try_recv() {
                Ok(sample) => {
                    self.reply = None;
                    if refresh.submitted {
                        return Poll::Ready(sample);
                    }
                    // The previous caller timed out or cancelled. That result
                    // only frees the worker slot and cannot publish authority.
                }
                Err(TryRecvError::Empty) => {
                    self.refresh = Some(refresh);
                    return Poll::Pending;
                }
                Err(TryRecvError::Disconnected) => {
                    self.reply = None;
                    return Poll::Ready(Err(InterfaceError::ProviderUnavailable));
                }
            }
        }
        let (reply, result) = sync_channel(1);
        if self.requests.try_send(reply).is_err() {
            return Poll::Ready(Err(InterfaceError::ProviderUnavailable));
        }
        self.reply = Some(result);
        refresh.submitted = true;
        self.refresh = Some(refresh);
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_out_work_owns_the_only_slot_until_drained_and_is_never_reused() {
        const TEST_DEADLINE: Duration = Duration::from_secs(2);
        let (entered, started) = sync_channel(1);
        let (release, blocked) = sync_channel(1);
        let mut first = true;
        let mut worker = SampleWorker::start(move || {
            if first {
                first = false;
                entered.try_send(()).unwrap();
                blocked.recv_timeout(TEST_DEADLINE).unwrap();
                return Err(InterfaceError::ProviderChanged);
            }
            Ok(Vec::new())
        })
        .unwrap();

        let now = Instant::now();
        worker.begin(now, TEST_DEADLINE).unwrap();
        assert!(worker.poll(now).is_pending());
        started.recv_timeout(TEST_DEADLINE).unwrap();
        assert!(matches!(
            worker.poll(now + TEST_DEADLINE),
            Poll::Ready(Err(InterfaceError::ProviderTimedOut))
        ));
        worker.begin(now + TEST_DEADLINE, TEST_DEADLINE).unwrap();
        assert!(worker.poll(now + TEST_DEADLINE).is_pending());
        release.try_send(()).unwrap();
        let wait_started = Instant::now();
        loop {
            if let Poll::Ready(result) = worker.poll(now + TEST_DEADLINE) {
                assert!(result.unwrap().is_empty());
                break;
            }
            assert!(wait_started.elapsed() < TEST_DEADLINE);
            thread::yield_now();
        }
    }
}
