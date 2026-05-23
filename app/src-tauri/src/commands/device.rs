//! Device status + refresh commands.

use serde::Serialize;

use crate::commands::error::AppError;
use crate::commands::state::AppState;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub connected: bool,
}

pub fn get_device_status(state: &AppState) -> DeviceStatus {
    DeviceStatus {
        connected: state.is_connected(),
    }
}

pub fn force_refresh(state: &AppState) -> Result<(), AppError> {
    state.force_refresh().map_err(|e| match e {
        crate::commands::state::ApplyError::NotFound => AppError::not_found("no profile"),
        crate::commands::state::ApplyError::Device(t) => t.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::error::AppErrorKind;
    use crate::mira::transport::MockTransport;
    use std::sync::Arc;

    #[test]
    fn status_reports_disconnected_when_no_transport() {
        let state = AppState::in_memory();
        assert_eq!(get_device_status(&state), DeviceStatus { connected: false });
    }

    #[test]
    fn status_reports_connected_after_attach() {
        let state = AppState::in_memory();
        state.attach_transport(Arc::new(MockTransport::new()));
        assert_eq!(get_device_status(&state), DeviceStatus { connected: true });
    }

    #[test]
    fn force_refresh_without_device_returns_device_not_connected() {
        let state = AppState::in_memory();
        let err = force_refresh(&state).unwrap_err();
        assert_eq!(err.kind, AppErrorKind::DeviceNotConnected);
    }
}
