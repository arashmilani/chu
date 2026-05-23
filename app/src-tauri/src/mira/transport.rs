/// A pluggable transport for HID feature reports.
///
/// The trait exists so the driver, domain, and command layers can be
/// tested without touching real hardware. Production code uses the
/// `hidapi`-backed impl; tests use [`MockTransport`].
pub trait HidTransport: Send + Sync {
    fn write_feature(&self, report: &[u8]) -> Result<(), TransportError>;

    /// Read a feature report. Returns the bytes received (excluding the
    /// report ID prefix). Not all Mira firmware versions support reads —
    /// callers should handle [`TransportError::Unsupported`].
    fn read_feature(&self, buf: &mut [u8]) -> Result<usize, TransportError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("device returned NAK")]
    Nak,
    #[error("device disconnected")]
    Disconnected,
    #[error("operation not supported by this transport")]
    Unsupported,
    #[error("hid: {0}")]
    Hid(String),
    #[error("io: {0}")]
    Io(String),
}

impl From<hidapi::HidError> for TransportError {
    fn from(value: hidapi::HidError) -> Self {
        // hidapi's error type doesn't expose enough structure to
        // distinguish NAK from a generic write failure; the driver
        // layer maps known sentinels to richer variants.
        TransportError::Hid(value.to_string())
    }
}

/// In-memory transport that records every write so tests can assert
/// the exact byte sequence the driver produced.
#[derive(Default)]
pub struct MockTransport {
    state: std::sync::Mutex<MockState>,
}

#[derive(Default)]
struct MockState {
    writes: Vec<Vec<u8>>,
    next_results: std::collections::VecDeque<Result<(), TransportError>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.state
            .lock()
            .expect("mock transport poisoned")
            .writes
            .clone()
    }

    /// Queue a single result that the next `write_feature` call will
    /// return. Useful for scripting NAKs or disconnects in tests.
    pub fn queue_result(&self, result: Result<(), TransportError>) {
        self.state
            .lock()
            .expect("mock transport poisoned")
            .next_results
            .push_back(result);
    }
}

impl HidTransport for MockTransport {
    fn write_feature(&self, report: &[u8]) -> Result<(), TransportError> {
        let mut state = self.state.lock().expect("mock transport poisoned");
        let result = state.next_results.pop_front().unwrap_or(Ok(()));
        // Record the attempt regardless of outcome so tests can
        // verify what was sent before the error surfaced.
        state.writes.push(report.to_vec());
        result
    }

    fn read_feature(&self, _buf: &mut [u8]) -> Result<usize, TransportError> {
        Err(TransportError::Unsupported)
    }
}

/// `hidapi`-backed transport. Owns the open device handle.
///
/// `hidapi::HidDevice` is `Send` but not `Sync` (the C handle isn't
/// thread-safe), so we wrap it in a `Mutex` to satisfy the trait bound
/// and serialize concurrent writes from background tasks.
pub struct HidApiTransport {
    device: std::sync::Mutex<hidapi::HidDevice>,
}

impl HidApiTransport {
    pub fn open(api: &hidapi::HidApi, vid: u16, pid: u16) -> Result<Self, TransportError> {
        let device = api.open(vid, pid)?;
        Ok(Self {
            device: std::sync::Mutex::new(device),
        })
    }
}

impl HidTransport for HidApiTransport {
    fn write_feature(&self, report: &[u8]) -> Result<(), TransportError> {
        let device = self.device.lock().expect("hid transport poisoned");
        device.write(report)?;
        Ok(())
    }

    fn read_feature(&self, buf: &mut [u8]) -> Result<usize, TransportError> {
        let device = self.device.lock().expect("hid transport poisoned");
        Ok(device.read(buf)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_transport_starts_empty() {
        let mock = MockTransport::new();
        assert!(mock.writes().is_empty());
    }

    #[test]
    fn mock_transport_records_writes() {
        let mock = MockTransport::new();
        mock.write_feature(&[0x00, 0x01]).unwrap();
        mock.write_feature(&[0x00, 0x04, 0x07]).unwrap();
        assert_eq!(
            mock.writes(),
            vec![vec![0x00, 0x01], vec![0x00, 0x04, 0x07]]
        );
    }

    #[test]
    fn mock_transport_read_returns_unsupported() {
        let mock = MockTransport::new();
        let mut buf = [0u8; 8];
        assert!(matches!(
            mock.read_feature(&mut buf),
            Err(TransportError::Unsupported)
        ));
    }

    #[test]
    fn queued_nak_is_returned_from_next_write() {
        let mock = MockTransport::new();
        mock.queue_result(Err(TransportError::Nak));
        let err = mock.write_feature(&[0x00, 0x04, 0x07]).unwrap_err();
        assert!(matches!(err, TransportError::Nak));
        // The attempted write is still recorded so tests can verify
        // *what* was sent when the NAK happened.
        assert_eq!(mock.writes(), vec![vec![0x00, 0x04, 0x07]]);
    }

    #[test]
    fn queued_results_apply_in_order() {
        let mock = MockTransport::new();
        mock.queue_result(Ok(()));
        mock.queue_result(Err(TransportError::Disconnected));
        mock.queue_result(Ok(()));
        assert!(mock.write_feature(&[0x00, 0x01]).is_ok());
        assert!(matches!(
            mock.write_feature(&[0x00, 0x01]),
            Err(TransportError::Disconnected)
        ));
        assert!(mock.write_feature(&[0x00, 0x01]).is_ok());
    }
}
