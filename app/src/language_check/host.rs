//! Background worker actor, debounce scheduler, and UI channel bridge for offline spell & grammar checking.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::core::file_kind::FileKind;
use crate::core::settings::IdeSettings;
use crate::core::spellcheck::{LanguageCheckDiagnostic, UserDictionaryEntry};
use crate::language_check::downloader::{DictionaryDownloadOutcome, DictionaryManager};
use crate::language_check::engine::{DocumentCheckResult, LanguageCheckEngine, LanguageSource};
use crate::project::DiskProject;

enum WorkerCmd {
    CheckDocument {
        revision: u64,
        file: PathBuf,
        source: String,
        preferred_lang: String,
        ui_lang: String,
        grammar_enabled: bool,
        user_dictionaries: Vec<UserDictionaryEntry>,
        ignored_words: Vec<String>,
        dict_manager: DictionaryManager,
    },
    InvalidateDictionaries,
}

enum WorkerEvent {
    CheckCompleted {
        revision: u64,
        file: PathBuf,
        result: DocumentCheckResult,
    },
}

/// Coordinates background spell/grammar checking, dictionary downloads, and debouncing.
pub struct LanguageCheckHost {
    pub dict_manager: DictionaryManager,
    pub diagnostics: Vec<LanguageCheckDiagnostic>,
    pub effective_lang: String,
    pub lang_source: Option<LanguageSource>,
    pub missing_tier1_pack: Option<String>,
    pub checking: bool,
    pub last_duration_ms: f64,
    pub last_warning: Option<String>,
    pub checked_file: PathBuf,
    current_revision: u64,
    pending_check: bool,
    last_edit_time: f64,
    was_enabled: bool,
    cmd_tx: Sender<WorkerCmd>,
    event_rx: Receiver<WorkerEvent>,
    dl_tx: Sender<DictionaryDownloadOutcome>,
    dl_rx: Receiver<DictionaryDownloadOutcome>,
}

impl LanguageCheckHost {
    /// Spawns the background checking thread (with an 8 MiB stack for `nlprule` regexes).
    pub fn new(ctx: Option<egui::Context>) -> Self {
        let dict_manager = DictionaryManager::new();
        let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCmd>();
        let (event_tx, event_rx) = mpsc::channel::<WorkerEvent>();
        let (dl_tx, dl_rx) = mpsc::channel::<DictionaryDownloadOutcome>();

        let _ = std::thread::Builder::new()
            .name("azo-language-check".to_string())
            .stack_size(8 * 1024 * 1024)
            .spawn(move || {
                let mut engine = LanguageCheckEngine::new();
                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        WorkerCmd::InvalidateDictionaries => engine.invalidate_dictionaries(),
                        WorkerCmd::CheckDocument {
                            revision,
                            file,
                            source,
                            preferred_lang,
                            ui_lang,
                            grammar_enabled,
                            user_dictionaries,
                            ignored_words,
                            dict_manager,
                        } => {
                            // Coalesce stale queued checks by draining to the newest check command.
                            let mut latest = (
                                revision,
                                file,
                                source,
                                preferred_lang,
                                ui_lang,
                                grammar_enabled,
                                user_dictionaries,
                                ignored_words,
                                dict_manager,
                            );
                            while let Ok(next_cmd) = cmd_rx.try_recv() {
                                match next_cmd {
                                    WorkerCmd::InvalidateDictionaries => {
                                        engine.invalidate_dictionaries()
                                    }
                                    WorkerCmd::CheckDocument {
                                        revision: r,
                                        file: f,
                                        source: s,
                                        preferred_lang: pl,
                                        ui_lang: ul,
                                        grammar_enabled: ge,
                                        user_dictionaries: ud,
                                        ignored_words: iw,
                                        dict_manager: dm,
                                    } => {
                                        latest = (r, f, s, pl, ul, ge, ud, iw, dm);
                                    }
                                }
                            }
                            let (r, f, s, pl, ul, ge, ud, iw, dm) = latest;
                            let result = engine.check_document(&s, &pl, &ul, ge, &ud, &iw, &dm);
                            let _ = event_tx.send(WorkerEvent::CheckCompleted {
                                revision: r,
                                file: f,
                                result,
                            });
                            if let Some(ref c) = ctx {
                                c.request_repaint();
                            }
                        }
                    }
                }
            });

        Self {
            dict_manager,
            diagnostics: Vec::new(),
            effective_lang: String::new(),
            lang_source: None,
            missing_tier1_pack: None,
            checking: false,
            last_duration_ms: 0.0,
            last_warning: None,
            checked_file: PathBuf::new(),
            current_revision: 0,
            pending_check: false,
            last_edit_time: 0.0,
            was_enabled: false,
            cmd_tx,
            event_rx,
            dl_tx,
            dl_rx,
        }
    }

    /// Marks the active document modified and schedules a debounced check when auto-check is on.
    pub fn on_text_edited(&mut self, ctx: &egui::Context, settings: &IdeSettings) {
        self.current_revision = self.current_revision.wrapping_add(1);
        if settings.spell_check_enabled && settings.auto_spell_check {
            self.pending_check = true;
            self.last_edit_time = ctx.input(|i| i.time);
        }
    }

    /// Triggers an immediate spell & grammar check of the active Typst buffer.
    pub fn check_now(&mut self, settings: &IdeSettings, project: &DiskProject, ui_lang: &str) {
        self.pending_check = false;
        if !settings.spell_check_enabled {
            self.clear_results();
            return;
        }
        let Some(buf) = project.active_buffer() else {
            self.clear_results();
            return;
        };
        if buf.kind != FileKind::Typst {
            self.clear_results();
            return;
        }
        let Some(ref text) = buf.text else {
            self.clear_results();
            return;
        };

        self.current_revision = self.current_revision.wrapping_add(1);
        self.checking = true;
        self.checked_file = buf.rel_path.clone();

        let _ = self.cmd_tx.send(WorkerCmd::CheckDocument {
            revision: self.current_revision,
            file: buf.rel_path.clone(),
            source: text.clone(),
            preferred_lang: settings.spell_check_language.clone(),
            ui_lang: ui_lang.to_string(),
            grammar_enabled: settings.grammar_check_enabled,
            user_dictionaries: settings.user_dictionaries.clone(),
            ignored_words: settings.ignored_words.clone(),
            dict_manager: self.dict_manager.clone(),
        });
    }

    /// Clears active diagnostics and missing-pack prompts (e.g., when disabled).
    pub fn clear_results(&mut self) {
        self.diagnostics.clear();
        self.missing_tier1_pack = None;
        self.checking = false;
        self.pending_check = false;
        self.last_warning = None;
    }

    /// Notifies the background worker to reload dictionaries after installing/removing packs.
    pub fn invalidate_dictionaries(&self) {
        let _ = self.cmd_tx.send(WorkerCmd::InvalidateDictionaries);
    }

    /// Starts downloading a Tier-1 language pack (`.aff`, `.dic`, and `nlprule` binaries).
    pub fn download_pack(&self, lang_code: &str, ctx: &egui::Context) {
        self.dict_manager
            .spawn_download_pack(lang_code, self.dl_tx.clone(), ctx.clone());
    }

    /// Removes a cached Tier-1 language pack and refreshes the active check state.
    pub fn remove_pack(
        &mut self,
        lang_code: &str,
        settings: &IdeSettings,
        project: &DiskProject,
        ui_lang: &str,
    ) -> Result<(), String> {
        self.dict_manager.remove_pack(lang_code)?;
        self.invalidate_dictionaries();
        if settings.spell_check_enabled {
            self.check_now(settings, project, ui_lang);
        }
        Ok(())
    }

    /// Polls background worker completions, download outcomes, and debounced check timers.
    /// Returns an optional status notice for the status bar.
    pub fn poll(
        &mut self,
        ctx: &egui::Context,
        settings: &IdeSettings,
        project: &DiskProject,
        ui_lang: &str,
    ) -> Option<String> {
        let mut status_notice = None;

        // Handle master enable/disable transitions cleanly.
        if !settings.spell_check_enabled {
            if self.was_enabled {
                self.was_enabled = false;
                self.clear_results();
            }
        } else if !self.was_enabled {
            self.was_enabled = true;
            self.check_now(settings, project, ui_lang);
        } else if project.active_file != self.checked_file && settings.auto_spell_check {
            self.check_now(settings, project, ui_lang);
        }

        // Poll dictionary download completions.
        while let Ok(outcome) = self.dl_rx.try_recv() {
            match outcome.result {
                Ok(()) => {
                    self.invalidate_dictionaries();
                    status_notice = Some(format!(
                        "Downloaded language pack '{}' for offline spell & grammar checking.",
                        outcome.lang_code
                    ));
                    if settings.spell_check_enabled {
                        self.check_now(settings, project, ui_lang);
                    }
                }
                Err(err) => {
                    status_notice = Some(format!("Language pack download failed: {err}"));
                }
            }
        }

        // Handle debounced auto-check timer.
        if self.pending_check && settings.spell_check_enabled && settings.auto_spell_check {
            let now = ctx.input(|i| i.time);
            let delay_secs = (settings.spell_check_debounce_ms as f64) / 1000.0;
            if now - self.last_edit_time >= delay_secs {
                self.check_now(settings, project, ui_lang);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(60));
            }
        }

        // Poll worker results.
        while let Ok(ev) = self.event_rx.try_recv() {
            match ev {
                WorkerEvent::CheckCompleted {
                    revision,
                    file,
                    result,
                } => {
                    if revision == self.current_revision && file == project.active_file {
                        self.checking = false;
                        self.diagnostics = result.diagnostics;
                        self.effective_lang = result.effective_lang;
                        self.lang_source = Some(result.lang_source);
                        self.missing_tier1_pack = result.missing_tier1_pack;
                        self.last_duration_ms = result.duration_ms;
                        self.last_warning = result.warning;
                    }
                }
            }
        }

        status_notice
    }
}

#[cfg(test)]
#[path = "../tests/language_check/host_tests.rs"]
mod tests;
