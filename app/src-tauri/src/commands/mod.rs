//! Command layer: the IPC surface the frontend invokes.
//!
//! Each command is a thin wrapper around an `AppState` method; the
//! Tauri attribute lives on a function in the parent crate so we can
//! keep the business logic unit-testable without spinning up a
//! webview. The wrappers translate errors to [`AppError`] before they
//! cross the IPC boundary.

pub mod device;
pub mod error;
pub mod profiles;
pub mod state;

pub use error::AppError;
pub use state::AppState;
