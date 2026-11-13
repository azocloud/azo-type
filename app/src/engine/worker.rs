//! External Typst compiler process bridge supporting official CLI binaries and JSON-stdio workers.

mod cli_parse;
#[cfg(test)]
#[path = "../tests/engine/worker_tests.rs"]
mod tests;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;

use crate::core::outline::CompiledOutlineItem;
use crate::core::pdf::PdfExportConfig;
use crate::engine::types::{
    decode_rendered_page_dto, CompileSnapshot, DiagnosticItem, EngineCompileRequest,
    EngineCompileResponse, EngineExportPdfRequest, EngineExportPdfResponse, EngineInitHandshake,
    EngineMetadata, TypstEngine,
};
use crate::project::DiskProject;

use cli_parse::{
    decode_rendered_png_pages, parse_cli_diagnostics, parse_cli_outline_json, StagedWorkspace,
    TempDirGuard,
};

/// Execution protocol used to communicate with an external engine binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerMode {
    /// Standard cached official `typst` CLI executable (`typst compile`, `typst query`).
    TypstCli,
    /// Structured JSON-over-stdio worker executable speaking `EngineCompileRequest`/`Response`.
    JsonStdio,
}

/// [`TypstEngine`] implementation that executes an external versioned Typst binary.
pub struct ExternalWorkerEngine {
    version: String,
    binary_path: PathBuf,
    package_cache_dir: Option<PathBuf>,
    mode: WorkerMode,
}

impl ExternalWorkerEngine {
    /// Creates an engine backed by a cached official `typst` CLI executable.
    pub fn new(version: String, binary_path: PathBuf, package_cache_dir: Option<PathBuf>) -> Self {
        Self {
            version,
            binary_path,
            package_cache_dir,
            mode: WorkerMode::TypstCli,
        }
    }

    /// Creates an engine using the structured JSON-stdio worker protocol.
    pub fn with_json_stdio(
        version: String,
        binary_path: PathBuf,
        package_cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            version,
            binary_path,
            package_cache_dir,
            mode: WorkerMode::JsonStdio,
        }
    }

    fn compile_via_cli(&self, project: &DiskProject, pixel_per_pt: f32) -> CompileSnapshot {
        let start = Instant::now();
        let stage = match StagedWorkspace::prepare(project) {
            Ok(s) => s,
            Err(err) => {
                return failed_snapshot(
                    project.revision,
                    pixel_per_pt,
                    start.elapsed().as_secs_f64() * 1000.0,
                    DiagnosticItem {
                        is_error: true,
                        file: project.effective_compile_entrypoint(),
                        line: None,
                        column: None,
                        message: format!("Failed to stage project workspace: {err}"),
                        hints: Vec::new(),
                    },
                );
            }
        };

        let out_dir = match TempDirGuard::new("azo-type-out") {
            Ok(d) => d,
            Err(err) => {
                return failed_snapshot(
                    project.revision,
                    pixel_per_pt,
                    start.elapsed().as_secs_f64() * 1000.0,
                    DiagnosticItem {
                        is_error: true,
                        file: stage.entry_rel.clone(),
                        line: None,
                        column: None,
                        message: format!("Failed to create preview output directory: {err}"),
                        hints: Vec::new(),
                    },
                );
            }
        };

        let scale = pixel_per_pt.clamp(1.0, 4.0);
        let ppi = format!("{:.2}", scale * 72.0);
        let out_pattern = out_dir.path().join("page-{0p}.png");

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("compile")
            .arg("--format")
            .arg("png")
            .arg("--ppi")
            .arg(&ppi)
            .arg("--root")
            .arg(&stage.root_dir);
        if let Some(cache) = self.existing_package_cache_dir() {
            cmd.arg("--package-cache-path").arg(cache);
        }
        cmd.arg(&stage.entry_abs).arg(&out_pattern);

        let output = match cmd.output() {
            Ok(out) => out,
            Err(err) => {
                return failed_snapshot(
                    project.revision,
                    pixel_per_pt,
                    start.elapsed().as_secs_f64() * 1000.0,
                    DiagnosticItem {
                        is_error: true,
                        file: stage.entry_rel.clone(),
                        line: None,
                        column: None,
                        message: format!(
                            "Failed to run Typst {} binary '{}': {err}",
                            self.version,
                            self.binary_path.display()
                        ),
                        hints: Vec::new(),
                    },
                );
            }
        };

        let stderr_text = String::from_utf8_lossy(&output.stderr);
        let mut diagnostics = parse_cli_diagnostics(
            &stderr_text,
            &stage.root_dir,
            &stage.entry_rel,
            !output.status.success(),
        );

        let (compiled_ok, pages, outline) = if output.status.success() {
            match decode_rendered_png_pages(out_dir.path(), scale) {
                Ok(pages) => {
                    let outline = self.query_cli_outline(&stage);
                    (true, pages, outline)
                }
                Err(err) => {
                    diagnostics.push(DiagnosticItem {
                        is_error: true,
                        file: stage.entry_rel.clone(),
                        line: None,
                        column: None,
                        message: err,
                        hints: Vec::new(),
                    });
                    (false, Vec::new(), Vec::new())
                }
            }
        } else {
            (false, Vec::new(), Vec::new())
        };

        CompileSnapshot {
            revision: project.revision,
            pixel_per_pt,
            compiled_ok,
            pages,
            outline,
            diagnostics,
            duration_ms: start.elapsed().as_secs_f64() * 1000.0,
        }
    }

    fn query_cli_outline(&self, stage: &StagedWorkspace) -> Vec<CompiledOutlineItem> {
        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("query").arg("--root").arg(&stage.root_dir);
        if let Some(cache) = self.existing_package_cache_dir() {
            cmd.arg("--package-cache-path").arg(cache);
        }
        cmd.arg(&stage.entry_abs).arg("heading");

        let Ok(out) = cmd.output() else {
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        parse_cli_outline_json(&stdout, &stage.entry_rel).unwrap_or_default()
    }

    fn export_pdf_via_cli(
        &self,
        project: &DiskProject,
        config: &PdfExportConfig,
    ) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
        let entry = project.effective_compile_entrypoint();
        if let Err(msg) = config.build_standards() {
            return Err(vec![DiagnosticItem {
                is_error: true,
                file: entry,
                line: None,
                column: None,
                message: msg,
                hints: vec![
                    "Select a PDF version compatible with the chosen PDF standard.".to_string(),
                ],
            }]);
        }

        let stage = StagedWorkspace::prepare(project).map_err(|err| {
            vec![DiagnosticItem {
                is_error: true,
                file: entry.clone(),
                line: None,
                column: None,
                message: format!("Failed to stage project workspace: {err}"),
                hints: Vec::new(),
            }]
        })?;

        let out_dir = TempDirGuard::new("azo-type-pdf").map_err(|err| {
            vec![DiagnosticItem {
                is_error: true,
                file: stage.entry_rel.clone(),
                line: None,
                column: None,
                message: format!("Failed to create PDF output directory: {err}"),
                hints: Vec::new(),
            }]
        })?;
        let pdf_path = out_dir.path().join("export.pdf");

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("compile")
            .arg("--format")
            .arg("pdf")
            .arg("--root")
            .arg(&stage.root_dir);
        if let Some(cache) = self.existing_package_cache_dir() {
            cmd.arg("--package-cache-path").arg(cache);
        }
        for std_flag in config.cli_pdf_standards() {
            cmd.arg("--pdf-standard").arg(std_flag);
        }
        cmd.arg(&stage.entry_abs).arg(&pdf_path);

        let output = cmd.output().map_err(|err| {
            vec![DiagnosticItem {
                is_error: true,
                file: stage.entry_rel.clone(),
                line: None,
                column: None,
                message: format!(
                    "Failed to run Typst {} binary '{}': {err}",
                    self.version,
                    self.binary_path.display()
                ),
                hints: Vec::new(),
            }]
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(parse_cli_diagnostics(
                &stderr,
                &stage.root_dir,
                &stage.entry_rel,
                true,
            ));
        }

        fs::read(&pdf_path).map_err(|err| {
            vec![DiagnosticItem {
                is_error: true,
                file: stage.entry_rel,
                line: None,
                column: None,
                message: format!("Failed to read exported PDF output: {err}"),
                hints: Vec::new(),
            }]
        })
    }

    fn compile_via_json_stdio(&self, project: &DiskProject, pixel_per_pt: f32) -> CompileSnapshot {
        let start = Instant::now();
        let entry = project.effective_compile_entrypoint();
        let req = EngineCompileRequest {
            revision: project.revision,
            pixel_per_pt,
            root_dir: project.root_dir.clone(),
            entry_file: entry.clone(),
            in_memory_files: collect_in_memory_files(project),
            package_cache_dir: self.package_cache_dir.clone(),
        };
        let handshake = EngineInitHandshake {
            expected_version: self.version.clone(),
            package_cache_dir: self.package_cache_dir.clone(),
            include_system_fonts: true,
        };

        match run_json_stdio_command::<_, EngineCompileResponse>(
            &self.binary_path,
            "compile",
            &handshake,
            &req,
        ) {
            Ok(resp) => {
                let mut diagnostics = resp.diagnostics;
                let mut pages = Vec::with_capacity(resp.pages.len());
                for dto in resp.pages {
                    match decode_rendered_page_dto(dto) {
                        Ok(page) => pages.push(page),
                        Err(err) => {
                            diagnostics.push(DiagnosticItem {
                                is_error: true,
                                file: entry.clone(),
                                line: None,
                                column: None,
                                message: err,
                                hints: Vec::new(),
                            });
                            return CompileSnapshot {
                                revision: project.revision,
                                pixel_per_pt,
                                compiled_ok: false,
                                pages: Vec::new(),
                                outline: Vec::new(),
                                diagnostics,
                                duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                            };
                        }
                    }
                }
                CompileSnapshot {
                    revision: project.revision,
                    pixel_per_pt,
                    compiled_ok: resp.compiled_ok,
                    pages,
                    outline: resp.outline,
                    diagnostics,
                    duration_ms: start.elapsed().as_secs_f64() * 1000.0,
                }
            }
            Err(err) => failed_snapshot(
                project.revision,
                pixel_per_pt,
                start.elapsed().as_secs_f64() * 1000.0,
                DiagnosticItem {
                    is_error: true,
                    file: entry,
                    line: None,
                    column: None,
                    message: err,
                    hints: Vec::new(),
                },
            ),
        }
    }

    fn export_pdf_via_json_stdio(
        &self,
        project: &DiskProject,
        config: &PdfExportConfig,
    ) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
        let entry = project.effective_compile_entrypoint();
        let req = EngineExportPdfRequest {
            root_dir: project.root_dir.clone(),
            entry_file: entry.clone(),
            in_memory_files: collect_in_memory_files(project),
            config: config.clone(),
            package_cache_dir: self.package_cache_dir.clone(),
        };
        let handshake = EngineInitHandshake {
            expected_version: self.version.clone(),
            package_cache_dir: self.package_cache_dir.clone(),
            include_system_fonts: true,
        };

        let resp = run_json_stdio_command::<_, EngineExportPdfResponse>(
            &self.binary_path,
            "export-pdf",
            &handshake,
            &req,
        )
        .map_err(|err| {
            vec![DiagnosticItem {
                is_error: true,
                file: entry.clone(),
                line: None,
                column: None,
                message: err,
                hints: Vec::new(),
            }]
        })?;

        match resp.pdf_base64 {
            Some(b64) => BASE64.decode(b64.as_bytes()).map_err(|e| {
                vec![DiagnosticItem {
                    is_error: true,
                    file: entry,
                    line: None,
                    column: None,
                    message: format!("Invalid base64 PDF response: {e}"),
                    hints: Vec::new(),
                }]
            }),
            None => Err(if resp.diagnostics.is_empty() {
                vec![DiagnosticItem {
                    is_error: true,
                    file: entry,
                    line: None,
                    column: None,
                    message: "External engine did not return PDF output.".to_string(),
                    hints: Vec::new(),
                }]
            } else {
                resp.diagnostics
            }),
        }
    }

    fn existing_package_cache_dir(&self) -> Option<&PathBuf> {
        self.package_cache_dir.as_ref().filter(|p| p.exists())
    }
}

impl TypstEngine for ExternalWorkerEngine {
    fn metadata(&self) -> EngineMetadata {
        EngineMetadata {
            version: self.version.clone(),
            is_builtin: false,
            display_label: format!("{} (Cached)", self.version),
        }
    }

    fn compile_project(&self, project: &DiskProject, pixel_per_pt: f32) -> CompileSnapshot {
        match self.mode {
            WorkerMode::TypstCli => self.compile_via_cli(project, pixel_per_pt),
            WorkerMode::JsonStdio => self.compile_via_json_stdio(project, pixel_per_pt),
        }
    }

    fn export_pdf_bytes(
        &self,
        project: &DiskProject,
        config: &PdfExportConfig,
    ) -> Result<Vec<u8>, Vec<DiagnosticItem>> {
        match self.mode {
            WorkerMode::TypstCli => self.export_pdf_via_cli(project, config),
            WorkerMode::JsonStdio => self.export_pdf_via_json_stdio(project, config),
        }
    }
}

fn failed_snapshot(
    revision: u64,
    pixel_per_pt: f32,
    duration_ms: f64,
    diag: DiagnosticItem,
) -> CompileSnapshot {
    CompileSnapshot {
        revision,
        pixel_per_pt,
        compiled_ok: false,
        pages: Vec::new(),
        outline: Vec::new(),
        diagnostics: vec![diag],
        duration_ms,
    }
}

fn collect_in_memory_files(project: &DiskProject) -> Vec<(PathBuf, Vec<u8>)> {
    project
        .buffers
        .iter()
        .filter_map(|(path, buf)| {
            buf.text
                .as_ref()
                .map(|t| (path.clone(), t.as_bytes().to_vec()))
        })
        .collect()
}

fn run_json_stdio_command<Req: serde::Serialize, Resp: serde::de::DeserializeOwned>(
    binary_path: &Path,
    command_name: &str,
    handshake: &EngineInitHandshake,
    request: &Req,
) -> Result<Resp, String> {
    let mut child = Command::new(binary_path)
        .arg("--worker-stdio")
        .arg(command_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "Failed to spawn JSON worker '{}': {e}",
                binary_path.display()
            )
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        let init_line = serde_json::to_string(handshake)
            .map_err(|e| format!("Failed to serialize worker handshake: {e}"))?;
        let req_line = serde_json::to_string(request)
            .map_err(|e| format!("Failed to serialize worker request: {e}"))?;
        writeln!(stdin, "{init_line}")
            .and_then(|()| writeln!(stdin, "{req_line}"))
            .map_err(|e| format!("Failed writing to worker stdin: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed waiting for worker process: {e}"))?;
    if !output.status.success() {
        let err_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Worker exited with {}: {}",
            output.status,
            err_msg.trim()
        ));
    }

    serde_json::from_slice::<Resp>(&output.stdout)
        .map_err(|e| format!("Failed to parse worker JSON response: {e}"))
}
