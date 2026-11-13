//! Platform-native async file and folder dialogs using `rfd`.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use crate::core::spellcheck::UserDictionaryEntry;
use crate::core::typst_prose::normalize_full_language_tag;

/// Minimal spinning future executor used for waiting on asynchronous native dialogs
/// without pulling in full multi-threaded async runtime scaffolding for UI threads.
pub fn block_on_or_spin<F: Future>(mut fut: F) -> F::Output {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);

    let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
    let waker = unsafe { Waker::from_raw(raw_waker) };
    let mut cx = Context::from_waker(&waker);

    loop {
        let pinned = unsafe { Pin::new_unchecked(&mut fut) };
        match pinned.poll(&mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
}

/// Spawns an asynchronous native folder picker dialog on a background thread.
pub fn spawn_open_folder_dialog(
    start_dir: &Path,
    on_picked: impl FnOnce(PathBuf) + Send + 'static,
    repaint: impl Fn() + Send + 'static,
) {
    let start_dir = start_dir.to_path_buf();
    std::thread::spawn(move || {
        let task = async move {
            if let Some(folder) = rfd::AsyncFileDialog::new()
                .set_title("Open Typst Project Folder")
                .set_directory(&start_dir)
                .pick_folder()
                .await
            {
                on_picked(folder.path().to_path_buf());
                repaint();
            }
        };
        block_on_or_spin(task);
    });
}

/// Spawns an asynchronous native file save dialog on a background thread.
pub fn spawn_save_file_dialog(
    start_dir: &Path,
    default_name: &str,
    filter_name: &'static str,
    filter_ext: &'static str,
    bytes: Vec<u8>,
    on_complete: impl FnOnce(Result<PathBuf, std::io::Error>) + Send + 'static,
    repaint: impl Fn() + Send + 'static,
) {
    let start_dir = start_dir.to_path_buf();
    let default_name = default_name.to_string();
    std::thread::spawn(move || {
        let task = async move {
            if let Some(handle) = rfd::AsyncFileDialog::new()
                .set_title("Export Compiled PDF to Disk")
                .set_directory(&start_dir)
                .set_file_name(&default_name)
                .add_filter(filter_name, &[filter_ext])
                .save_file()
                .await
            {
                let out_path = handle.path().to_path_buf();
                let res = std::fs::write(&out_path, &bytes).map(|()| out_path);
                on_complete(res);
                repaint();
            }
        };
        block_on_or_spin(task);
    });
}

/// Spawns an asynchronous native file picker for selecting a Hunspell `.dic` or `.aff` file.
pub fn spawn_open_dictionary_dialog(
    start_dir: Option<&Path>,
    on_picked: impl FnOnce(Result<UserDictionaryEntry, String>) + Send + 'static,
    repaint: impl Fn() + Send + 'static,
) {
    let start_dir = start_dir.map(Path::to_path_buf);
    std::thread::spawn(move || {
        let task = async move {
            let mut dialog = rfd::AsyncFileDialog::new()
                .set_title("Import Hunspell Dictionary (.dic / .aff)")
                .add_filter("Hunspell Dictionary", &["dic", "aff"]);
            if let Some(dir) = start_dir {
                if dir.is_dir() {
                    dialog = dialog.set_directory(dir);
                }
            }
            if let Some(file) = dialog.pick_file().await {
                let res = resolve_dictionary_pair(file.path());
                on_picked(res);
                repaint();
            }
        };
        block_on_or_spin(task);
    });
}

/// Resolves the matching `.aff` + `.dic` companion pair from a single picked dictionary file.
pub fn resolve_dictionary_pair(picked: &Path) -> Result<UserDictionaryEntry, String> {
    let ext = picked
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if ext != "dic" && ext != "aff" {
        return Err(format!(
            "Selected file '{}' is not a .dic or .aff Hunspell dictionary file.",
            picked.display()
        ));
    }

    let parent = picked
        .parent()
        .ok_or_else(|| format!("Invalid dictionary file path: '{}'.", picked.display()))?;
    let stem = picked
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Invalid dictionary filename: '{}'.", picked.display()))?;

    let companion_ext = if ext == "dic" { "aff" } else { "dic" };
    let companion = parent.join(format!("{stem}.{companion_ext}"));
    if !companion.is_file() {
        return Err(format!(
            "Companion .{companion_ext} file not found next to '{}' (expected '{}').",
            picked.display(),
            companion.display()
        ));
    }

    let (aff_path, dic_path) = if ext == "aff" {
        (picked.to_path_buf(), companion)
    } else {
        (companion, picked.to_path_buf())
    };

    let lang_code = normalize_full_language_tag(stem).unwrap_or_else(|| stem.to_ascii_lowercase());
    Ok(UserDictionaryEntry {
        lang_code,
        label: stem.to_string(),
        aff_path,
        dic_path,
    })
}

#[cfg(test)]
#[path = "../tests/platform/dialogs_tests.rs"]
mod tests;
