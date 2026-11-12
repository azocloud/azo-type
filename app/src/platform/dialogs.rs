//! Platform-native async file and folder dialogs using `rfd`.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

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
