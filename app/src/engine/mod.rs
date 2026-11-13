//! Version-agnostic Typst engine abstraction, external worker bridge, and download manager.

pub mod manager;
pub mod types;
pub mod worker;

pub use manager::{
    ensure_tls_crypto_provider, DownloadProgress, DownloadStage, EngineDownloadEvent, EngineHost,
    EngineInstallState, EngineManager,
};
pub use types::{
    upload_page_textures, CompileSnapshot, DiagnosticItem, EngineMetadata, PreviewCharBox,
    RenderedPageData, RenderedPageTexture, TypstEngine,
};
