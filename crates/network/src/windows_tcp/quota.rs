//! Shared connection limits for all addresses in one listener generation.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, PoisonError};

const GLOBAL_LIMIT: usize = 64;
const PEER_LIMIT: usize = 8;

#[derive(Default)]
struct Counts {
    global: usize,
    peers: BTreeMap<IpAddr, usize>,
}

#[derive(Default)]
pub(super) struct ConnectionQuota(Arc<Mutex<Counts>>);

#[derive(Debug)]
pub(super) struct Unavailable;

impl ConnectionQuota {
    pub(super) fn reserve(&self, peer: IpAddr) -> Result<Option<ConnectionPermit>, Unavailable> {
        let peer = peer.to_canonical();
        let mut counts = self.0.lock().map_err(|_| Unavailable)?;
        if counts.global >= GLOBAL_LIMIT {
            return Ok(None);
        }
        let peer_count = counts.peers.entry(peer).or_default();
        if *peer_count >= PEER_LIMIT {
            return Ok(None);
        }
        *peer_count += 1;
        counts.global += 1;
        Ok(Some(ConnectionPermit {
            counts: Arc::clone(&self.0),
            peer,
        }))
    }
}

/// Holds an admission slot for the lifetime of a retained transport. Keep it
/// with the stream when handing the stream to an HTTP runtime.
#[must_use]
pub struct ConnectionPermit {
    counts: Arc<Mutex<Counts>>,
    peer: IpAddr,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        let mut counts = self.counts.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(peer_count) = counts.peers.get_mut(&self.peer) {
            *peer_count -= 1;
            if *peer_count == 0 {
                counts.peers.remove(&self.peer);
            }
            counts.global -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::thread;

    #[test]
    fn concurrent_mapped_and_ipv4_connections_share_one_peer_limit() {
        let quota = ConnectionQuota::default();
        let ip = Ipv4Addr::new(192, 0, 2, 1);
        let mut retained = thread::scope(|scope| {
            let workers: Vec<_> = (0..16)
                .map(|index| {
                    let quota = &quota;
                    scope.spawn(move || {
                        let peer = if index % 2 == 0 {
                            IpAddr::V4(ip)
                        } else {
                            ip.to_ipv6_mapped().into()
                        };
                        quota.reserve(peer).unwrap()
                    })
                })
                .collect();
            workers
                .into_iter()
                .filter_map(|worker| worker.join().unwrap())
                .collect::<Vec<_>>()
        });
        assert_eq!(retained.len(), PEER_LIMIT);
        assert!(quota.reserve(ip.into()).unwrap().is_none());
        drop(retained.pop());
        assert!(quota.reserve(ip.into()).unwrap().is_some());
    }

    #[test]
    fn different_peers_share_the_global_cap_and_old_generation_drops_cannot_free_it() {
        let old_generation = ConnectionQuota::default();
        let (start, end) = (1, u8::try_from(GLOBAL_LIMIT).unwrap());
        let addresses = (start..=end).map(|last| Ipv4Addr::new(192, 0, 2, last).into());
        let new_peer = Ipv4Addr::new(198, 51, 100, 5).into();
        let old = old_generation.reserve(new_peer).unwrap().unwrap();
        let current = ConnectionQuota::default();
        let mut retained: Vec<_> = addresses
            .map(|peer| current.reserve(peer).unwrap().unwrap())
            .collect();
        assert!(current.reserve(new_peer).unwrap().is_none());
        drop(old);
        assert!(current.reserve(new_peer).unwrap().is_none());
        drop(retained.pop());
        assert!(current.reserve(new_peer).unwrap().is_some());
    }
}
