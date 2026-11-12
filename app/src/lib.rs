use std::path::PathBuf;

pub mod app;
pub mod collab;
pub mod compiler;
pub mod core;
pub mod highlight;
pub mod platform;
pub mod project;
pub mod world;

pub fn parse_cli_args_from(args: &[String]) -> (Option<PathBuf>, Option<PathBuf>) {
    let mut screenshot_path = None;
    let mut folder_path = None;

    let mut idx = 1;
    while idx < args.len() {
        if args[idx] == "--screenshot" {
            if let Some(path_str) = args.get(idx + 1) {
                screenshot_path = Some(PathBuf::from(path_str));
                idx += 2;
                continue;
            }
        } else if let Some(rest) = args[idx].strip_prefix("--screenshot=") {
            screenshot_path = Some(PathBuf::from(rest));
            idx += 1;
            continue;
        } else if !args[idx].starts_with('-') && folder_path.is_none() {
            let candidate = PathBuf::from(&args[idx]);
            if candidate.is_dir() {
                folder_path = Some(candidate);
            }
        }
        idx += 1;
    }

    (folder_path, screenshot_path)
}
