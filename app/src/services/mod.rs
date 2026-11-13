//! Application background services and runners.
//!
//! - [`persistence`]: Persistent settings, language, and zoom state via `eframe::Storage`.
//! - [`screenshot`]: Headless screenshot automation lifecycle runner.

pub mod persistence;
pub mod screenshot;

pub use persistence::{
    flush_if_changed, load_persisted_state, save_persisted_parts, save_persisted_state,
    PersistedAppState,
};
pub use screenshot::handle_screenshot_lifecycle;
