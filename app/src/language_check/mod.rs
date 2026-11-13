//! Offline spellchecking (`spellbook`), grammar checking (`nlprule`), language detection
//! (`lingua`), and on-demand Tier-1 dictionary download orchestration.

pub mod downloader;
pub mod engine;
pub mod host;

pub use downloader::{
    default_dictionaries_cache_dir, DictionaryDownloadOutcome, DictionaryDownloadProgress,
    DictionaryManager, DictionaryPackStatus, InstalledTier1Paths,
};
pub use engine::{DocumentCheckResult, LanguageCheckEngine, LanguageSource};
pub use host::LanguageCheckHost;
