use crate::supervisor::ConnectionAdmission;
use futures_util::{SinkExt, StreamExt};
use std::io;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::time::{Instant, timeout, timeout_at};
use tokio_websockets::{
    CloseCode, Config, Limits, Message, Payload, ServerBuilder, WebSocketStream,
};

const MAX_PAYLOAD: usize = 65_536;
const READ_CHUNK: usize = 4096;
const READ_DEADLINE: Duration = Duration::from_secs(5);
const WRITE_DEADLINE: Duration = Duration::from_secs(2);
const CLOSE_DEADLINE: Duration = Duration::from_secs(1);
const NS_PER_SECOND: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlSocketError {
    Closed,
    Deadline,
    InvalidMessage,
    Io,
}

/// One binary control channel. Construction is restricted to the verified
/// HTTP upgrade. Payload types and library errors are kept out of diagnostics.
pub struct ControlSocket<'a, I> {
    websocket: WebSocketStream<Ingress<'a, I>>,
    read_deadline: Option<Instant>,
    close_code: Option<CloseCode>,
}

impl<'a, I: AsyncRead + AsyncWrite + Unpin> ControlSocket<'a, I> {
    pub(in crate::http1) fn new(
        io: &'a mut I,
        prefix: &'a [u8],
        quota: &'a RawQuota,
        admission: &'a ConnectionAdmission,
    ) -> Self {
        let ingress = Ingress {
            io,
            prefix,
            global: quota,
            admission,
            connection: Bucket::new(128 * 1024, 256 * 1024),
        };
        let websocket = ServerBuilder::new()
            .config(Config::default().frame_size(MAX_PAYLOAD).flush_threshold(0))
            .limits(Limits::default().max_payload_len(Some(MAX_PAYLOAD)))
            .serve(ingress);
        Self {
            websocket,
            read_deadline: None,
            close_code: None,
        }
    }

    /// Read one bounded binary message. Control frames do not reset the
    /// application read deadline or count as a protobuf heartbeat.
    ///
    /// # Errors
    /// Returns a closed category and the caller must retire the session on failure.
    pub async fn receive(&mut self) -> Result<ControlPayload, ControlSocketError> {
        let deadline = *self
            .read_deadline
            .get_or_insert_with(|| Instant::now() + READ_DEADLINE);
        loop {
            let result = timeout_at(deadline, self.websocket.next())
                .await
                .map_err(|_| ControlSocketError::Deadline)?;
            match result {
                Some(Ok(message)) if message.is_binary() => {
                    self.read_deadline = None;
                    return Ok(ControlPayload(message.into_payload()));
                }
                Some(Ok(message)) if message.is_ping() => {
                    timeout_at(deadline, self.websocket.flush())
                        .await
                        .map_err(|_| ControlSocketError::Deadline)?
                        .map_err(|error| socket_error(&error))?;
                }
                Some(Ok(message)) if message.is_pong() => {}
                Some(Ok(message)) if message.is_close() => return Err(ControlSocketError::Closed),
                Some(Ok(_)) => return Err(ControlSocketError::InvalidMessage),
                Some(Err(error)) => return Err(socket_error(&error)),
                None => return Err(ControlSocketError::Closed),
            }
        }
    }

    /// Write and flush one bounded binary message.
    ///
    /// # Errors
    /// The caller must retire the connection on an oversized write or failure.
    pub async fn send(&mut self, bytes: Vec<u8>) -> Result<(), ControlSocketError> {
        if bytes.len() > MAX_PAYLOAD {
            return Err(ControlSocketError::InvalidMessage);
        }
        timeout(WRITE_DEADLINE, self.websocket.send(Message::binary(bytes)))
            .await
            .map_err(|_| ControlSocketError::Deadline)?
            .map_err(|error| socket_error(&error))
    }

    /// Selects an application close code without any payload-derived reason.
    pub fn set_close_code(&mut self, code: u16) {
        self.close_code = CloseCode::try_from(code)
            .ok()
            .filter(|value| !value.is_reserved());
    }

    /// Attempt a bounded close after the caller has requested session cleanup.
    pub async fn close(&mut self) {
        let _ = timeout(CLOSE_DEADLINE, async {
            if let Some(code) = self.close_code {
                let _ = self.websocket.send(Message::close(Some(code), "")).await;
            }
            self.websocket.close().await
        })
        .await;
    }
}

pub struct ControlPayload(Payload);

impl AsRef<[u8]> for ControlPayload {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

fn socket_error(error: &tokio_websockets::Error) -> ControlSocketError {
    match error {
        tokio_websockets::Error::Io(_) => ControlSocketError::Io,
        tokio_websockets::Error::AlreadyClosed => ControlSocketError::Closed,
        _ => ControlSocketError::InvalidMessage,
    }
}

pub(in crate::http1) struct RawQuota(Mutex<Bucket>);

impl RawQuota {
    pub(in crate::http1) const fn new() -> Self {
        Self(Mutex::new(Bucket::new(512 * 1024, 1024 * 1024)))
    }
}

struct Bucket {
    cap: u64,
    per_second: u64,
    credit: u64,
    last: Option<Instant>,
}

impl Bucket {
    const fn new(burst: u64, per_second: u64) -> Self {
        let cap = burst * NS_PER_SECOND;
        Self {
            cap,
            per_second,
            credit: cap,
            last: None,
        }
    }

    fn available(&mut self, now: Instant) -> usize {
        if let Some(last) = self.last {
            let elapsed = u64::try_from(now.duration_since(last).as_nanos()).unwrap_or(u64::MAX);
            self.credit = self
                .credit
                .saturating_add(elapsed.saturating_mul(self.per_second))
                .min(self.cap);
        }
        self.last = Some(now);
        usize::try_from(self.credit / NS_PER_SECOND).unwrap_or(usize::MAX)
    }

    fn charge(&mut self, bytes: usize) {
        self.credit -= u64::try_from(bytes).expect("bounded read length") * NS_PER_SECOND;
    }
}

struct Ingress<'a, I> {
    io: &'a mut I,
    prefix: &'a [u8],
    global: &'a RawQuota,
    admission: &'a ConnectionAdmission,
    connection: Bucket,
}

impl<I: AsyncRead + Unpin> AsyncRead for Ingress<'_, I> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dest: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if dest.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let this = self.get_mut();
        if !this.admission.is_current() {
            return Poll::Ready(Err(io::ErrorKind::PermissionDenied.into()));
        }
        let Ok(mut global) = this.global.0.lock() else {
            return Poll::Ready(Err(io::ErrorKind::PermissionDenied.into()));
        };
        let now = Instant::now();
        let size = this
            .connection
            .available(now)
            .min(global.available(now))
            .min(dest.remaining())
            .min(READ_CHUNK);
        if size == 0 {
            return Poll::Ready(Err(io::ErrorKind::PermissionDenied.into()));
        }
        let read = if this.prefix.is_empty() {
            let mut capped = ReadBuf::new(dest.initialize_unfilled_to(size));
            match Pin::new(&mut *this.io).poll_read(cx, &mut capped) {
                Poll::Ready(Ok(())) => {
                    let n = capped.filled().len();
                    dest.advance(n);
                    n
                }
                other => return other,
            }
        } else {
            let (prefix, rest) = this.prefix.split_at(this.prefix.len().min(size));
            dest.put_slice(prefix);
            this.prefix = rest;
            prefix.len()
        };
        this.connection.charge(read);
        global.charge(read);
        Poll::Ready(Ok(()))
    }
}

impl<I: AsyncWrite + Unpin> AsyncWrite for Ingress<'_, I> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut *self.io).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut *self.io).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn masked_frame(opcode: u8, size: usize) -> Vec<u8> {
        let mut frame = vec![opcode];
        if size < 126 {
            frame.push(0x80 | u8::try_from(size).unwrap());
        } else if let Ok(size) = u16::try_from(size) {
            frame.push(0xfe);
            frame.extend_from_slice(&size.to_be_bytes());
        } else {
            frame.push(0xff);
            frame.extend_from_slice(&u64::try_from(size).unwrap().to_be_bytes());
        }
        frame.extend_from_slice(&[1, 2, 3, 4]);
        frame.resize(frame.len() + size, 7);
        frame
    }

    #[test]
    fn individual_and_reassembled_payloads_share_the_exact_ceiling() {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap()
            .block_on(async {
                let scope = ConnectionAdmission::test_only(std::num::NonZeroU64::MIN);
                let quota = RawQuota::new();
                let mut oversize = masked_frame(0x02, MAX_PAYLOAD / 2);
                oversize.extend_from_slice(&masked_frame(0x80, MAX_PAYLOAD / 2 + 1));
                for (input, accepted) in [
                    (masked_frame(0x82, MAX_PAYLOAD), true),
                    (masked_frame(0x82, MAX_PAYLOAD + 1), false),
                    (oversize, false),
                ] {
                    let (mut client, mut stream) = tokio::io::duplex(MAX_PAYLOAD + 64);
                    client.write_all(&input).await.unwrap();
                    let mut socket = ControlSocket::new(&mut stream, &[], &quota, &scope);
                    match socket.receive().await {
                        Ok(payload) => {
                            assert!(accepted);
                            assert_eq!(payload.as_ref().len(), MAX_PAYLOAD);
                        }
                        Err(error) => {
                            assert!(!accepted);
                            assert_eq!(error, ControlSocketError::InvalidMessage);
                        }
                    }
                }
            });
    }

    #[test]
    fn prefetched_bytes_are_charged_to_both_budgets_and_revocation_stops_reads() {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(async {
                let scope = ConnectionAdmission::test_only(std::num::NonZeroU64::MIN);
                let quota = RawQuota(Mutex::new(Bucket::new(4, 4)));
                let mut source = tokio::io::empty();
                let mut ingress = Ingress {
                    io: &mut source,
                    prefix: b"12345678",
                    global: &quota,
                    admission: &scope,
                    connection: Bucket::new(2, 2),
                };
                let mut destination = [0; 4];
                assert_eq!(ingress.read(&mut destination).await.unwrap(), 2);
                assert_eq!(&destination[..2], b"12");
                assert_eq!(
                    ingress.read(&mut destination).await.unwrap_err().kind(),
                    io::ErrorKind::PermissionDenied
                );
                ingress.connection = Bucket::new(8, 8);
                assert_eq!(ingress.read(&mut destination).await.unwrap(), 2);
                assert_eq!(&destination[..2], b"34");
                assert_eq!(
                    ingress.read(&mut destination).await.unwrap_err().kind(),
                    io::ErrorKind::PermissionDenied
                );
                tokio::time::advance(Duration::from_secs(1)).await;
                scope.retire_for_test();
                assert_eq!(
                    ingress.read(&mut destination).await.unwrap_err().kind(),
                    io::ErrorKind::PermissionDenied
                );
            });
    }

    #[test]
    fn cancelling_a_pending_read_does_not_extend_its_deadline() {
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap()
            .block_on(async {
                let scope = ConnectionAdmission::test_only(std::num::NonZeroU64::MIN);
                let quota = RawQuota::new();
                let (_client, mut stream) = tokio::io::duplex(128);
                let mut socket = ControlSocket::new(&mut stream, &[], &quota, &scope);
                assert!(
                    timeout(Duration::from_secs(4), socket.receive())
                        .await
                        .is_err()
                );
                let started = Instant::now();
                assert!(matches!(
                    socket.receive().await,
                    Err(ControlSocketError::Deadline)
                ));
                assert_eq!(started.elapsed(), Duration::from_secs(1));
            });
    }
}
