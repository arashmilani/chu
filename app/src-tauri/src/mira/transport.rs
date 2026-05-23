/// A pluggable transport for HID feature reports.
///
/// The trait exists so the driver, domain, and command layers can be
/// tested without touching real hardware. Production code uses the
/// `hidapi`-backed impl; tests use [`MockTransport`].
pub trait HidTransport: Send + Sync {
    fn write_feature(&self, report: &[u8]) -> Result<(), TransportError>;
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("device returned NAK")]
    Nak,
    #[error("device disconnected")]
    Disconnected,
    #[error("io: {0}")]
    Io(String),
}

/// In-memory transport that records every write so tests can assert
/// the exact byte sequence the driver produced.
#[derive(Default)]
pub struct MockTransport {
    writes: std::sync::Mutex<Vec<Vec<u8>>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.writes.lock().expect("mock transport poisoned").clone()
    }
}

impl HidTransport for MockTransport {
    fn write_feature(&self, report: &[u8]) -> Result<(), TransportError> {
        self.writes
            .lock()
            .expect("mock transport poisoned")
            .push(report.to_vec());
        Ok(())
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
}
