//! Shared request credits for the bodyless HTTP asset service.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_PEERS: usize = 128;
const PEER_IDLE: Duration = Duration::from_secs(60);
const CREDIT: u64 = 1_000_000_000;
const PER_PEER: Budget = Budget {
    per_second: 30,
    interval_seconds: 1,
    burst: 60,
};
const GLOBAL: Budget = Budget {
    per_second: 240,
    interval_seconds: 1,
    burst: 480,
};

#[derive(Clone, Copy)]
struct Budget {
    per_second: u64,
    interval_seconds: u64,
    burst: u64,
}

impl Budget {
    const fn cost(self) -> u64 {
        self.interval_seconds * CREDIT
    }
}

struct Bucket {
    credit: u64,
    last: Instant,
}

impl Bucket {
    const fn full(now: Instant, budget: Budget) -> Self {
        Self {
            credit: budget.burst * budget.cost(),
            last: now,
        }
    }

    fn refill(&mut self, now: Instant, budget: Budget) -> bool {
        let Some(elapsed) = now.checked_duration_since(self.last) else {
            return false;
        };
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.credit = self
            .credit
            .saturating_add(nanos.saturating_mul(budget.per_second))
            .min(budget.burst * budget.cost());
        self.last = now;
        self.credit >= budget.cost()
    }
}

struct Requests {
    global: Bucket,
    peers: BTreeMap<IpAddr, Bucket>,
}

pub(super) struct RequestRate {
    state: Mutex<Option<Requests>>,
    peer: Budget,
    global: Budget,
}

impl RequestRate {
    pub(super) const fn new() -> Self {
        Self {
            state: Mutex::new(None),
            peer: PER_PEER,
            global: GLOBAL,
        }
    }

    pub(super) const fn pairing() -> Self {
        Self {
            state: Mutex::new(None),
            peer: Budget {
                per_second: 1,
                interval_seconds: 20,
                burst: 3,
            },
            global: Budget {
                per_second: 1,
                interval_seconds: 5,
                burst: 8,
            },
        }
    }

    pub(super) const fn control() -> Self {
        Self {
            state: Mutex::new(None),
            peer: Budget {
                per_second: 1,
                interval_seconds: 1,
                burst: 4,
            },
            global: GLOBAL,
        }
    }

    pub(super) fn admit(&self, peer: IpAddr, now: Instant) -> bool {
        let Ok(mut state) = self.state.lock() else {
            return false;
        };
        let requests = state.get_or_insert_with(|| Requests {
            global: Bucket::full(now, self.global),
            peers: BTreeMap::new(),
        });
        if !requests.global.refill(now, self.global) {
            return false;
        }
        let peer = peer.to_canonical();
        if requests.peers.len() >= MAX_PEERS && !requests.peers.contains_key(&peer) {
            // A static connection cannot stay active without requests for
            // this long under the header, response, and keep-alive deadlines.
            requests
                .peers
                .retain(|_, bucket| now.saturating_duration_since(bucket.last) < PEER_IDLE);
            if requests.peers.len() >= MAX_PEERS {
                return false;
            }
        }
        let bucket = requests
            .peers
            .entry(peer)
            .or_insert_with(|| Bucket::full(now, self.peer));
        if !bucket.refill(now, self.peer) {
            return false;
        }
        bucket.credit -= self.peer.cost();
        requests.global.credit -= self.global.cost();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn shared_global_and_canonical_peer_bursts_refill_without_eviction() {
        let now = Instant::now();
        let rate = RequestRate::new();
        for number in 1..=8 {
            let peer = Ipv4Addr::new(192, 0, 2, number);
            for _ in 0..PER_PEER.burst {
                assert!(rate.admit(peer.into(), now));
            }
            assert!(!rate.admit(peer.to_ipv6_mapped().into(), now));
        }
        let fresh = Ipv4Addr::new(192, 0, 2, 9).into();
        assert!(!rate.admit(fresh, now));
        assert!(rate.admit(fresh, now + Duration::from_secs(1)));
    }

    #[test]
    fn control_attempts_share_a_four_request_burst_per_canonical_peer() {
        let now = Instant::now();
        let rate = RequestRate::control();
        let peer = Ipv4Addr::new(192, 0, 2, 1);
        for _ in 0..4 {
            assert!(rate.admit(peer.into(), now));
        }
        assert!(!rate.admit(peer.to_ipv6_mapped().into(), now));
        assert!(rate.admit(peer.into(), now + Duration::from_secs(1)));
    }

    #[test]
    fn refuses_new_peer_at_capacity_until_inactive_entries_expire() {
        let now = Instant::now();
        let rate = RequestRate::new();
        for number in 1..=u8::try_from(MAX_PEERS).unwrap() {
            assert!(rate.admit(Ipv4Addr::new(192, 0, 2, number).into(), now));
        }
        let fresh = Ipv4Addr::new(198, 51, 100, 1).into();
        assert!(!rate.admit(fresh, now + Duration::from_secs(1)));
        assert!(rate.admit(fresh, now + PEER_IDLE));
    }
}
