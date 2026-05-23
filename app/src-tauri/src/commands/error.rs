//! Typed error for the command layer.
//!
//! Serializes as `{ "kind": "...", "message": "..." }` so the
//! frontend can pattern-match on kind without parsing English.

use serde::Serialize;

use crate::domain::persistence::PersistenceError;
use crate::domain::store::ProfileError;
use crate::mira::transport::TransportError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AppErrorKind {
    NotFound,
    ReadOnly,
    InvalidInput,
    DeviceNotConnected,
    DeviceNak,
    PersistenceFailed,
    Internal,
}

impl AppError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::NotFound,
            message: msg.into(),
        }
    }
    pub fn read_only(msg: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::ReadOnly,
            message: msg.into(),
        }
    }
    pub fn invalid_input(msg: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::InvalidInput,
            message: msg.into(),
        }
    }
    pub fn device_not_connected() -> Self {
        Self {
            kind: AppErrorKind::DeviceNotConnected,
            message: "no Mira device is connected".to_string(),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            kind: AppErrorKind::Internal,
            message: msg.into(),
        }
    }
}

impl From<ProfileError> for AppError {
    fn from(e: ProfileError) -> Self {
        match e {
            ProfileError::NotFound => AppError::not_found(e.to_string()),
            ProfileError::ReadOnly => AppError::read_only(e.to_string()),
            ProfileError::InvalidPosition(..) | ProfileError::EmptyName => {
                AppError::invalid_input(e.to_string())
            }
        }
    }
}

impl From<TransportError> for AppError {
    fn from(e: TransportError) -> Self {
        match e {
            TransportError::Nak => AppError {
                kind: AppErrorKind::DeviceNak,
                message: e.to_string(),
            },
            TransportError::Disconnected => AppError::device_not_connected(),
            _ => AppError::internal(e.to_string()),
        }
    }
}

impl From<PersistenceError> for AppError {
    fn from(e: PersistenceError) -> Self {
        AppError {
            kind: AppErrorKind::PersistenceFailed,
            message: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_kind_and_message_object() {
        let err = AppError::not_found("profile missing");
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, r#"{"kind":"not-found","message":"profile missing"}"#);
    }

    #[test]
    fn read_only_profile_error_maps_through_from() {
        let err: AppError = ProfileError::ReadOnly.into();
        assert_eq!(err.kind, AppErrorKind::ReadOnly);
    }

    #[test]
    fn transport_nak_maps_to_device_nak() {
        let err: AppError = TransportError::Nak.into();
        assert_eq!(err.kind, AppErrorKind::DeviceNak);
    }

    #[test]
    fn transport_disconnected_maps_to_device_not_connected() {
        let err: AppError = TransportError::Disconnected.into();
        assert_eq!(err.kind, AppErrorKind::DeviceNotConnected);
    }
}
