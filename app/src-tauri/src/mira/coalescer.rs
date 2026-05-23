//! Coalesces rapid same-opcode writes into a single transport write.
//!
//! Why: a slider drag produces dozens of frames per second; without
//! coalescing the device floods, and the panel ghosts/stutters. The
//! window from the spec is 16ms — long enough to swallow a frame's
//! worth of input, short enough that a deliberate change still feels
//! immediate.
//!
//! How: writes are stored per-opcode in a pending buffer, replacing
//! any prior pending value for the same opcode. `flush_if_quiet(now)`
//! is the test seam — it drains the buffer to the transport only once
//! `now` is at least `window` past the last submission. Production
//! wires this to a background tick; tests drive `now` directly.
//!
//! Different opcodes don't interfere: submitting set_speed and
//! set_contrast keeps two independent pending entries.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::mira::transport::{HidTransport, TransportError};

/// Default coalescing window, per spec §6 validation rules.
pub const DEFAULT_WINDOW: Duration = Duration::from_millis(16);

pub struct Coalescer<T: HidTransport + ?Sized> {
    transport: Arc<T>,
    window: Duration,
    state: Mutex<State>,
}

struct State {
    pending: BTreeMap<u8, Vec<u8>>,
    last_submit: Option<Instant>,
}

impl<T: HidTransport + ?Sized> Coalescer<T> {
    pub fn new(transport: Arc<T>, window: Duration) -> Self {
        Self {
            transport,
            window,
            state: Mutex::new(State {
                pending: BTreeMap::new(),
                last_submit: None,
            }),
        }
    }

    /// Submit a frame. The opcode (byte 1, after the USB report ID
    /// prefix) is the key; a frame with the same opcode replaces the
    /// pending one.
    pub fn submit(&self, frame: Vec<u8>, now: Instant) {
        let opcode = frame.get(1).copied().unwrap_or(0);
        let mut state = self.state.lock().expect("coalescer poisoned");
        state.pending.insert(opcode, frame);
        state.last_submit = Some(now);
    }

    /// Flush pending frames to the transport iff the quiet window has
    /// elapsed since the last submission. Returns the number of frames
    /// actually written (0 if still inside the window).
    pub fn flush_if_quiet(&self, now: Instant) -> Result<usize, TransportError> {
        let mut state = self.state.lock().expect("coalescer poisoned");
        let last = match state.last_submit {
            Some(t) => t,
            None => return Ok(0),
        };
        if now.duration_since(last) < self.window {
            return Ok(0);
        }
        let drained: Vec<Vec<u8>> = std::mem::take(&mut state.pending).into_values().collect();
        state.last_submit = None;
        drop(state);

        for frame in &drained {
            self.transport.write_feature(frame)?;
        }
        Ok(drained.len())
    }

    /// Drain everything regardless of timing — for shutdown.
    pub fn flush_all(&self) -> Result<usize, TransportError> {
        let mut state = self.state.lock().expect("coalescer poisoned");
        let drained: Vec<Vec<u8>> = std::mem::take(&mut state.pending).into_values().collect();
        state.last_submit = None;
        drop(state);

        for frame in &drained {
            self.transport.write_feature(frame)?;
        }
        Ok(drained.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mira::encoder::{encode_set_contrast, encode_set_speed};
    use crate::mira::transport::MockTransport;

    fn fixture() -> (Arc<MockTransport>, Coalescer<MockTransport>, Instant) {
        let mock = Arc::new(MockTransport::new());
        let coalescer = Coalescer::new(mock.clone(), DEFAULT_WINDOW);
        (mock, coalescer, Instant::now())
    }

    #[test]
    fn rapid_same_opcode_collapses_to_latest_value() {
        let (mock, coalescer, t0) = fixture();
        coalescer.submit(encode_set_speed(3), t0);
        coalescer.submit(encode_set_speed(4), t0 + Duration::from_millis(5));
        coalescer.submit(encode_set_speed(5), t0 + Duration::from_millis(10));

        // Still inside the 16ms window since last submit (10ms).
        let flushed = coalescer
            .flush_if_quiet(t0 + Duration::from_millis(20))
            .unwrap();
        assert_eq!(flushed, 0);
        assert!(mock.writes().is_empty());

        // Past the window: 30 - 10 = 20ms >= 16ms.
        let flushed = coalescer
            .flush_if_quiet(t0 + Duration::from_millis(30))
            .unwrap();
        assert_eq!(flushed, 1);
        assert_eq!(mock.writes(), vec![encode_set_speed(5)]);
    }

    #[test]
    fn different_opcodes_do_not_coalesce() {
        let (mock, coalescer, t0) = fixture();
        coalescer.submit(encode_set_speed(7), t0);
        coalescer.submit(encode_set_contrast(12), t0);

        let flushed = coalescer
            .flush_if_quiet(t0 + Duration::from_millis(50))
            .unwrap();
        assert_eq!(flushed, 2);

        let writes = mock.writes();
        assert_eq!(writes.len(), 2);
        // BTreeMap iteration is ordered by opcode, so contrast (0x05)
        // sorts before speed (0x04)? No — 0x04 < 0x05, so speed first.
        assert!(writes.contains(&encode_set_speed(7)));
        assert!(writes.contains(&encode_set_contrast(12)));
    }

    #[test]
    fn flush_is_noop_with_no_submissions() {
        let (mock, coalescer, t0) = fixture();
        let flushed = coalescer
            .flush_if_quiet(t0 + Duration::from_secs(1))
            .unwrap();
        assert_eq!(flushed, 0);
        assert!(mock.writes().is_empty());
    }

    #[test]
    fn flush_all_ignores_window() {
        let (mock, coalescer, t0) = fixture();
        coalescer.submit(encode_set_speed(5), t0);
        // Way inside the window — but flush_all forces it out.
        let flushed = coalescer.flush_all().unwrap();
        assert_eq!(flushed, 1);
        assert_eq!(mock.writes(), vec![encode_set_speed(5)]);
    }

    #[test]
    fn flush_propagates_nak_from_transport() {
        let (mock, coalescer, t0) = fixture();
        mock.queue_result(Err(TransportError::Nak));
        coalescer.submit(encode_set_speed(5), t0);
        let err = coalescer
            .flush_if_quiet(t0 + Duration::from_millis(50))
            .unwrap_err();
        assert!(matches!(err, TransportError::Nak));
    }
}
