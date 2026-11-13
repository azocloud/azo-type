//! Version-agnostic Typst engine abstraction, external worker bridge, and download manager.

pub mod manager;
pub mod types;
pub mod worker;

pub use manager::{EngineDownloadEvent, EngineHost, EngineInstallState, EngineManager};
pub use types::{
    CompileSnapshot, DiagnosticItem, EngineMetadata, PreviewCharBox, RenderedPageData,
    RenderedPageTexture, TypstEngine, upload_page_textures,
};
