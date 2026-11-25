use std::time::Duration;

const SECOND: u64 = 1_000_000_000;

struct Bucket {
    rate: u64,
    ceiling: u64,
    credit: u64,
}

impl Bucket {
    const fn new(rate: u64, burst: u64) -> Self {
        let ceiling = burst * SECOND;
        Self {
            rate,
            ceiling,
            credit: ceiling,
        }
    }

    fn replenish(&mut self, nanos: u64) {
        self.credit = self
            .credit
            .saturating_add(nanos.saturating_mul(self.rate))
            .min(self.ceiling);
    }
}

/// Runtime rate accounting lives on the same single writer as the validated
/// operation count. Both scopes are charged together before admission commit.
pub(super) struct Rate {
    messages: Bucket,
    operations: Bucket,
    last: Duration,
}

impl Rate {
    pub(super) const fn connection(now: Duration) -> Self {
        Self {
            messages: Bucket::new(240, 120),
            operations: Bucket::new(1024, 256),
            last: now,
        }
    }

    pub(super) const fn shared(now: Duration) -> Self {
        Self {
            messages: Bucket::new(960, 480),
            operations: Bucket::new(4096, 1024),
            last: now,
        }
    }

    pub(super) fn admit(&mut self, shared: &mut Self, now: Duration, operations: usize) -> bool {
        let Some(cost) = u64::try_from(operations)
            .ok()
            .and_then(|count| count.checked_mul(SECOND))
        else {
            return false;
        };
        if !self.replenish(now)
            || !shared.replenish(now)
            || self.messages.credit < SECOND
            || shared.messages.credit < SECOND
            || self.operations.credit < cost
            || shared.operations.credit < cost
        {
            return false;
        }
        for rate in [self, shared] {
            rate.messages.credit -= SECOND;
            rate.operations.credit -= cost;
        }
        true
    }

    fn replenish(&mut self, now: Duration) -> bool {
        let Some(elapsed) = now.checked_sub(self.last) else {
            return false;
        };
        self.last = now;
        let nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.messages.replenish(nanos);
        self.operations.replenish(nanos);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_operation_budget_is_shared_across_connections() {
        let now = Duration::ZERO;
        let mut shared = Rate::shared(now);
        for _ in 0..4 {
            let mut peer = Rate::connection(now);
            assert!(peer.admit(&mut shared, now, 256));
            assert!(!peer.admit(&mut shared, now, 1));
        }
        let mut later = Rate::connection(now);
        assert!(!later.admit(&mut shared, now, 1));
        assert!(later.admit(&mut shared, now, 0));
        assert!(later.admit(&mut shared, Duration::from_millis(1), 4));
    }
}
