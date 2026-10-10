//! Deterministic, non-production utilities shared by Rust tests.
//!
//! This crate must only be used from test and development dependencies. In
//! particular, [`SeededRandom`] is predictable and is never a cryptographic
//! random source.

use std::fmt;
use std::fs::{self, DirBuilder};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static TEMPORARY_DIRECTORY_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Error returned when a fake-clock duration cannot fit its representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockOverflow;

impl fmt::Display for ClockOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fake monotonic clock overflow")
    }
}

impl std::error::Error for ClockOverflow {}

/// Cloneable monotonic clock controlled explicitly by a test.
#[derive(Clone, Debug, Default)]
pub struct FakeClock {
    nanoseconds: Arc<AtomicU64>,
}

impl FakeClock {
    /// Returns the current monotonic test time.
    #[must_use]
    pub fn now(&self) -> Duration {
        Duration::from_nanos(self.nanoseconds.load(Ordering::SeqCst))
    }

    /// Advances time, returning the new instant.
    ///
    /// # Errors
    ///
    /// Returns [`ClockOverflow`] if the duration or resulting instant cannot be
    /// represented in nanoseconds by a `u64`.
    pub fn advance(&self, duration: Duration) -> Result<Duration, ClockOverflow> {
        let increment = u64::try_from(duration.as_nanos()).map_err(|_| ClockOverflow)?;
        let previous = self
            .nanoseconds
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
                current.checked_add(increment)
            })
            .map_err(|_| ClockOverflow)?;
        let current = previous.checked_add(increment).ok_or(ClockOverflow)?;
        Ok(Duration::from_nanos(current))
    }
}

/// Predictable `SplitMix64` stream for deterministic tests.
#[derive(Clone, Debug)]
pub struct SeededRandom {
    state: u64,
}

impl SeededRandom {
    /// Creates a stream with an explicit seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Returns the next deterministic word.
    #[must_use]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// Fills bytes from the deterministic stream.
    pub fn fill_bytes(&mut self, destination: &mut [u8]) {
        for chunk in destination.chunks_mut(size_of::<u64>()) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
    }
}

/// Owner-only temporary directory removed on drop unless retained explicitly.
#[derive(Debug)]
pub struct PrivateTempDir {
    path: PathBuf,
    remove_on_drop: bool,
}

impl PrivateTempDir {
    /// Creates a directory below the operating-system temporary directory.
    ///
    /// # Errors
    ///
    /// Returns the last filesystem error if a unique private directory cannot
    /// be created.
    pub fn new() -> io::Result<Self> {
        Self::new_in(&std::env::temp_dir())
    }

    /// Creates a directory below `parent` without widening parent permissions.
    ///
    /// # Errors
    ///
    /// Returns the last filesystem error if a unique private directory cannot
    /// be created.
    pub fn new_in(parent: &Path) -> io::Result<Self> {
        let mut last_error = None;
        for _ in 0..64 {
            let sequence = TEMPORARY_DIRECTORY_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("tether-test-{}-{sequence}", std::process::id()));
            let mut builder = DirBuilder::new();
            configure_private_directory(&mut builder);
            match builder.create(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        remove_on_drop: true,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }
        Err(last_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "could not allocate unique Tether test directory",
            )
        }))
    }

    /// Returns the directory path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Prevents drop cleanup and returns ownership of the path.
    #[must_use]
    pub fn retain(mut self) -> PathBuf {
        self.remove_on_drop = false;
        self.path.clone()
    }
}

impl Drop for PrivateTempDir {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _cleanup_result = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(unix)]
fn configure_private_directory(builder: &mut DirBuilder) {
    use std::os::unix::fs::DirBuilderExt;
    builder.mode(0o700);
}

#[cfg(not(unix))]
fn configure_private_directory(_builder: &mut DirBuilder) {}

/// Non-sensitive component identifier accepted by test trace capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceComponent {
    Agent,
    Config,
    Discovery,
    Input,
    Network,
    Pairing,
    Protocol,
    Session,
}

/// Non-sensitive event identifier accepted by test trace capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceCode {
    Fault,
    Rejected,
    Saturated,
    Started,
    Stopped,
    TimedOut,
}

/// Typed numeric metadata that cannot carry text or input content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafeMetric {
    Connections(u32),
    ElapsedMicros(u64),
    QueueDepth(u32),
    Retries(u32),
}

/// A trace event whose type cannot contain free-form text or secret bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafeTraceEvent {
    pub component: TraceComponent,
    pub code: TraceCode,
    pub metric: Option<SafeMetric>,
}

/// Bounded in-memory trace capture for assertions on sanitized events.
#[derive(Clone, Debug)]
pub struct SanitizedTraceCapture {
    events: Arc<Mutex<Vec<SafeTraceEvent>>>,
    capacity: usize,
    dropped: Arc<AtomicU64>,
}

impl SanitizedTraceCapture {
    /// Creates a capture with a non-zero event capacity.
    ///
    /// # Panics
    ///
    /// Panics when `capacity` is zero because an always-saturated capture would
    /// hide test mistakes.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "trace capacity must be non-zero");
        Self {
            events: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
            capacity,
            dropped: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Records an event, returning false if the bounded capture is saturated.
    pub fn record(&self, event: SafeTraceEvent) -> bool {
        let mut events = self
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if events.len() == self.capacity {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            false
        } else {
            events.push(event);
            true
        }
    }

    /// Returns an owned snapshot for deterministic assertions.
    #[must_use]
    pub fn snapshot(&self) -> Vec<SafeTraceEvent> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Returns the number of events rejected due to saturation.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Kind of operating-system integration replaced by a no-op in tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoopOsHandleKind {
    Autostart,
    PermissionMonitor,
    SessionMonitor,
    Tray,
    WakeLock,
}

/// Cloneable no-op handle that exposes lifecycle state without OS side effects.
#[derive(Clone, Debug)]
pub struct NoopOsHandle {
    kind: NoopOsHandleKind,
    active: Arc<AtomicBool>,
}

impl NoopOsHandle {
    /// Creates an inactive handle of the requested kind.
    #[must_use]
    pub fn new(kind: NoopOsHandleKind) -> Self {
        Self {
            kind,
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Activates only the in-memory test state.
    pub fn activate(&self) {
        self.active.store(true, Ordering::SeqCst);
    }

    /// Deactivates only the in-memory test state.
    pub fn deactivate(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    /// Returns the integration kind.
    #[must_use]
    pub const fn kind(&self) -> NoopOsHandleKind {
        self.kind
    }

    /// Returns the in-memory lifecycle state.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_advances_monotonically_and_detects_overflow() {
        let clock = FakeClock::default();
        assert_eq!(clock.now(), Duration::ZERO);
        assert_eq!(
            clock.advance(Duration::from_millis(5)),
            Ok(Duration::from_millis(5))
        );
        assert_eq!(clock.now(), Duration::from_millis(5));
        assert_eq!(clock.advance(Duration::MAX), Err(ClockOverflow));
        assert_eq!(clock.now(), Duration::from_millis(5));
    }

    #[test]
    fn fake_clock_clones_share_the_same_monotonic_state() {
        let clock = FakeClock::default();
        let clone = clock.clone();
        clone
            .advance(Duration::from_nanos(7))
            .expect("advance succeeds");
        assert_eq!(clock.now(), Duration::from_nanos(7));
    }

    #[test]
    fn seeded_random_is_reproducible_and_fills_partial_chunks() {
        let mut first = SeededRandom::new(42);
        let mut second = SeededRandom::new(42);
        assert_eq!(first.next_u64(), second.next_u64());

        let mut first_bytes = [0_u8; 13];
        let mut second_bytes = [0_u8; 13];
        first.fill_bytes(&mut first_bytes);
        second.fill_bytes(&mut second_bytes);
        assert_eq!(first_bytes, second_bytes);
        assert_ne!(first_bytes, [0_u8; 13]);
    }

    #[test]
    fn private_temp_directory_is_created_and_removed() {
        let path;
        {
            let directory = PrivateTempDir::new().expect("private directory is created");
            path = directory.path().to_path_buf();
            assert!(path.is_dir());
        }
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn private_temp_directory_has_owner_only_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = PrivateTempDir::new().expect("private directory is created");
        let mode = fs::metadata(directory.path())
            .expect("metadata is readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[test]
    fn sanitized_capture_is_bounded_and_counts_drops() {
        let capture = SanitizedTraceCapture::new(1);
        let event = SafeTraceEvent {
            component: TraceComponent::Session,
            code: TraceCode::Started,
            metric: Some(SafeMetric::Connections(1)),
        };
        assert!(capture.record(event));
        assert!(!capture.record(event));
        assert_eq!(capture.snapshot(), vec![event]);
        assert_eq!(capture.dropped(), 1);
    }

    #[test]
    fn no_op_handles_share_state_without_os_side_effects() {
        let handle = NoopOsHandle::new(NoopOsHandleKind::WakeLock);
        let clone = handle.clone();
        assert_eq!(handle.kind(), NoopOsHandleKind::WakeLock);
        assert!(!handle.is_active());
        clone.activate();
        assert!(handle.is_active());
        handle.deactivate();
        assert!(!clone.is_active());
    }
}
