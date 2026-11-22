//! Recently opened project folder MRU history and cross-platform display formatting.
//!
//! Tracks up to [`MAX_RECENT_PROJECTS`] unique project folder paths ordered by most-recently-used
//! without performing any filesystem I/O.

use std::path::{Component, Path, PathBuf};

#[cfg(test)]
#[path = "../tests/core/recent_projects_tests.rs"]
mod tests;

/// Maximum number of project folders retained in the recently opened list.
pub const MAX_RECENT_PROJECTS: usize = 10;

/// Ordered most-recently-used list of project folder paths.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RecentProjects {
    entries: Vec<PathBuf>,
}

impl RecentProjects {
    /// Creates an empty recently opened projects list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a deduplicated list from an iterator of paths, preserving first-seen order
    /// and capping the result at [`MAX_RECENT_PROJECTS`].
    pub fn from_paths(paths: impl IntoIterator<Item = PathBuf>) -> Self {
        let mut entries: Vec<PathBuf> = Vec::new();
        for path in paths {
            let Some(normalized) = normalize_lexical_path(&path) else {
                continue;
            };
            let key = lexical_key(&normalized);
            if entries.iter().any(|existing| lexical_key(existing) == key) {
                continue;
            }
            entries.push(normalized);
            if entries.len() >= MAX_RECENT_PROJECTS {
                break;
            }
        }
        Self { entries }
    }

    /// Returns the ordered project paths from most-recent (`index 0`) to oldest.
    pub fn entries(&self) -> &[PathBuf] {
        &self.entries
    }

    /// Returns `true` if no recent projects are recorded.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the most recently opened project path, if any.
    pub fn most_recent(&self) -> Option<&Path> {
        self.entries.first().map(PathBuf::as_path)
    }

    /// Records `path` as the most recently opened project.
    ///
    /// Empty paths are ignored. Trailing separators and lexical dot segments (`.` / `..`) are
    /// normalized cross-platform without filesystem I/O, any existing matching entry is moved to
    /// index `0`, and the list is truncated to [`MAX_RECENT_PROJECTS`].
    pub fn record(&mut self, path: PathBuf) {
        let Some(normalized) = normalize_lexical_path(&path) else {
            return;
        };
        let key = lexical_key(&normalized);
        self.entries.retain(|existing| lexical_key(existing) != key);
        self.entries.insert(0, normalized);
        self.entries.truncate(MAX_RECENT_PROJECTS);
    }

    /// Removes any entry lexically matching `path`.
    pub fn remove(&mut self, path: &Path) {
        let Some(normalized) = normalize_lexical_path(path) else {
            return;
        };
        let key = lexical_key(&normalized);
        self.entries.retain(|existing| lexical_key(existing) != key);
    }

    /// Clears all recorded recent project entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Formats a clean cross-platform display label for a project folder path.
    ///
    /// Returns `<folder_name> — <full_path>` when the path has a terminal folder name distinct
    /// from the full normalized path, or falls back to the full normalized path for roots and
    /// single-segment relative paths.
    pub fn format_entry_label(path: &Path) -> String {
        let Some(normalized) = normalize_lexical_path(path) else {
            return String::new();
        };

        if let Some(raw) = normalized.to_str() {
            let parsed = parse_utf8_lexical_path(raw);
            let folder_name = parsed
                .segments
                .last()
                .copied()
                .filter(|seg| *seg != "." && *seg != "..");
            return match folder_name {
                Some(name) if name != raw => format!("{name} — {raw}"),
                _ => raw.to_string(),
            };
        }

        let full_path = normalized.to_string_lossy();
        match normalized.file_name().map(|name| name.to_string_lossy()) {
            Some(name) if !name.is_empty() && name.as_ref() != full_path.as_ref() => {
                format!("{name} — {full_path}")
            }
            _ => full_path.into_owned(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LexicalKey {
    Utf8 {
        prefix: Option<String>,
        has_root: bool,
        segments: Vec<String>,
    },
    Os(PathBuf),
}

struct ParsedUtf8Path<'a> {
    prefix: Option<String>,
    has_root: bool,
    segments: Vec<&'a str>,
    use_backslash: bool,
}

fn parse_utf8_lexical_path(raw: &str) -> ParsedUtf8Path<'_> {
    let use_backslash = raw.contains('\\');
    let (prefix, has_root, rest) = split_prefix_and_root(raw);
    let mut segments: Vec<&str> = Vec::new();

    for seg in rest.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." => {
                if let Some(&last) = segments.last() {
                    if last != ".." {
                        segments.pop();
                    } else if !has_root && prefix.is_none() {
                        segments.push("..");
                    }
                } else if !has_root && prefix.is_none() {
                    segments.push("..");
                }
            }
            _ => segments.push(seg),
        }
    }

    ParsedUtf8Path {
        prefix,
        has_root,
        segments,
        use_backslash,
    }
}

fn split_prefix_and_root(raw: &str) -> (Option<String>, bool, &str) {
    // Windows UNC paths: `\\server\share\...` or `\\?\C:\...`
    if let Some(after_unc) = raw.strip_prefix(r"\\") {
        let mut parts = after_unc.splitn(3, ['\\', '/']);
        let first = parts.next().unwrap_or("");
        let second = parts.next().unwrap_or("");
        let tail = parts.next().unwrap_or("");

        if (first == "?" || first == ".")
            && second.len() == 2
            && second.as_bytes()[0].is_ascii_alphabetic()
            && second.as_bytes()[1] == b':'
        {
            return (
                Some(format!(r"\\{first}\{second}")),
                !tail.is_empty() || raw.len() > 2 + first.len() + 1 + second.len(),
                tail,
            );
        }

        if !first.is_empty() && !second.is_empty() {
            return (Some(format!(r"\\{first}\{second}")), true, tail);
        }
        if !first.is_empty() {
            return (Some(format!(r"\\{first}")), true, second);
        }
        return (Some(r"\\".to_string()), true, after_unc);
    }

    // Windows drive prefix: `C:\...`, `C:/...`, or `C:...`
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        let drive = &raw[..2];
        let rest = &raw[2..];
        let has_root = rest.starts_with('/') || rest.starts_with('\\');
        return (Some(drive.to_string()), has_root, rest);
    }

    // POSIX root `/...`
    if let Some(rest) = raw.strip_prefix('/') {
        return (None, true, rest);
    }

    // Single leading backslash treated as rooted when no UNC prefix
    if let Some(rest) = raw.strip_prefix('\\') {
        return (None, true, rest);
    }

    (None, false, raw)
}

fn normalize_lexical_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }

    if let Some(raw) = path.to_str() {
        if raw.trim().is_empty() {
            return None;
        }
        let parsed = parse_utf8_lexical_path(raw);
        let sep = if parsed.use_backslash { '\\' } else { '/' };
        let sep_str = if parsed.use_backslash { "\\" } else { "/" };
        let mut out = String::new();

        match (&parsed.prefix, parsed.has_root) {
            (Some(prefix), true) => {
                out.push_str(prefix);
                if parsed.segments.is_empty() {
                    if !prefix.starts_with(r"\\") {
                        out.push(sep);
                    }
                } else {
                    out.push(sep);
                    out.push_str(&parsed.segments.join(sep_str));
                }
            }
            (Some(prefix), false) => {
                out.push_str(prefix);
                out.push_str(&parsed.segments.join(sep_str));
            }
            (None, true) => {
                out.push(sep);
                out.push_str(&parsed.segments.join(sep_str));
            }
            (None, false) => {
                if parsed.segments.is_empty() {
                    out.push('.');
                } else {
                    out.push_str(&parsed.segments.join(sep_str));
                }
            }
        }

        return Some(PathBuf::from(out));
    }

    Some(normalize_os_components(path))
}

fn normalize_os_components(path: &Path) -> PathBuf {
    let mut stack: Vec<Component<'_>> = Vec::new();
    let mut has_root_or_prefix = false;

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match stack.last() {
                Some(Component::Normal(_)) => {
                    stack.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => stack.push(Component::ParentDir),
            },
            Component::RootDir | Component::Prefix(_) => {
                has_root_or_prefix = true;
                stack.push(component);
            }
            Component::Normal(_) => stack.push(component),
        }
    }

    if stack.is_empty() && !has_root_or_prefix {
        return PathBuf::from(".");
    }

    stack.iter().collect()
}

fn lexical_key(path: &Path) -> LexicalKey {
    if let Some(raw) = path.to_str() {
        let parsed = parse_utf8_lexical_path(raw);
        let prefix = parsed.prefix.map(|p| {
            let normalized = p.replace('\\', "/");
            if normalized.len() == 2 && normalized.as_bytes()[1] == b':' {
                normalized.to_ascii_uppercase()
            } else {
                normalized
            }
        });
        return LexicalKey::Utf8 {
            prefix,
            has_root: parsed.has_root,
            segments: parsed.segments.into_iter().map(str::to_string).collect(),
        };
    }

    LexicalKey::Os(normalize_os_components(path))
}
