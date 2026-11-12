//! Application background services and runners.
//!
//! - [`screenshot`]: Headless screenshot automation lifecycle runner.

pub mod screenshot;

pub use screenshot::handle_screenshot_lifecycle;
