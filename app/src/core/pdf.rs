//! PDF export configuration, PDF versions, and PDF/A archival standards.

use typst_pdf::{PdfStandard, PdfStandards};

#[cfg(test)]
mod tests;

/// Selectable PDF version in the Export PDF dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfVersionChoice {
    V1_4,
    V1_5,
    V1_6,
    V1_7,
    V2_0,
}

impl PdfVersionChoice {
    pub const ALL: &'static [PdfVersionChoice] =
        &[Self::V1_4, Self::V1_5, Self::V1_6, Self::V1_7, Self::V2_0];

    pub fn label(self) -> &'static str {
        match self {
            Self::V1_4 => "PDF 1.4",
            Self::V1_5 => "PDF 1.5",
            Self::V1_6 => "PDF 1.6",
            Self::V1_7 => "PDF 1.7 (Default)",
            Self::V2_0 => "PDF 2.0",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            Self::V1_4 => "1.4",
            Self::V1_5 => "1.5",
            Self::V1_6 => "1.6",
            Self::V1_7 => "1.7",
            Self::V2_0 => "2.0",
        }
    }

    pub fn to_standard(self) -> PdfStandard {
        match self {
            Self::V1_4 => PdfStandard::V_1_4,
            Self::V1_5 => PdfStandard::V_1_5,
            Self::V1_6 => PdfStandard::V_1_6,
            Self::V1_7 => PdfStandard::V_1_7,
            Self::V2_0 => PdfStandard::V_2_0,
        }
    }
}

/// Selectable PDF archival (PDF/A) standard in the Export PDF dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdfArchivalStandard {
    None,
    A1b,
    A1a,
    A2b,
    A2u,
    A2a,
    A3b,
    A3u,
    A3a,
    A4,
    A4f,
    A4e,
}

impl PdfArchivalStandard {
    pub const ALL: &'static [PdfArchivalStandard] = &[
        Self::None,
        Self::A1b,
        Self::A1a,
        Self::A2b,
        Self::A2u,
        Self::A2a,
        Self::A3b,
        Self::A3u,
        Self::A3a,
        Self::A4,
        Self::A4f,
        Self::A4e,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::None => "None (Standard PDF)",
            Self::A1b => "PDF/A-1b (ISO 19005-1 Level B — Basic Archival)",
            Self::A1a => "PDF/A-1a (ISO 19005-1 Level A — Accessible Archival)",
            Self::A2b => "PDF/A-2b (ISO 19005-2 Level B — Basic Archival)",
            Self::A2u => "PDF/A-2u (ISO 19005-2 Level U — Unicode Archival)",
            Self::A2a => "PDF/A-2a (ISO 19005-2 Level A — Accessible Archival)",
            Self::A3b => "PDF/A-3b (ISO 19005-3 Level B — Embedded Files)",
            Self::A3u => "PDF/A-3u (ISO 19005-3 Level U — Unicode + Embedded)",
            Self::A3a => "PDF/A-3a (ISO 19005-3 Level A — Accessible + Embedded)",
            Self::A4 => "PDF/A-4 (ISO 19005-4 — Modern PDF 2.0 Archival)",
            Self::A4f => "PDF/A-4f (ISO 19005-4 — Archival with Embedded Files)",
            Self::A4e => "PDF/A-4e (ISO 19005-4 — Engineering Archival)",
        }
    }

    pub fn code_label(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::A1b => "PDF/A-1b",
            Self::A1a => "PDF/A-1a",
            Self::A2b => "PDF/A-2b",
            Self::A2u => "PDF/A-2u",
            Self::A2a => "PDF/A-2a",
            Self::A3b => "PDF/A-3b",
            Self::A3u => "PDF/A-3u",
            Self::A3a => "PDF/A-3a",
            Self::A4 => "PDF/A-4",
            Self::A4f => "PDF/A-4f",
            Self::A4e => "PDF/A-4e",
        }
    }

    pub fn to_standard(self) -> Option<PdfStandard> {
        match self {
            Self::None => None,
            Self::A1b => Some(PdfStandard::A_1b),
            Self::A1a => Some(PdfStandard::A_1a),
            Self::A2b => Some(PdfStandard::A_2b),
            Self::A2u => Some(PdfStandard::A_2u),
            Self::A2a => Some(PdfStandard::A_2a),
            Self::A3b => Some(PdfStandard::A_3b),
            Self::A3u => Some(PdfStandard::A_3u),
            Self::A3a => Some(PdfStandard::A_3a),
            Self::A4 => Some(PdfStandard::A_4),
            Self::A4f => Some(PdfStandard::A_4f),
            Self::A4e => Some(PdfStandard::A_4e),
        }
    }

    /// Returns the recommended PDF version for this archival standard.
    pub fn recommended_version(self) -> Option<PdfVersionChoice> {
        match self {
            Self::None => None,
            Self::A1b | Self::A1a => Some(PdfVersionChoice::V1_4),
            Self::A2b | Self::A2u | Self::A2a | Self::A3b | Self::A3u | Self::A3a => {
                Some(PdfVersionChoice::V1_7)
            }
            Self::A4 | Self::A4f | Self::A4e => Some(PdfVersionChoice::V2_0),
        }
    }
}

/// Configuration for PDF export chosen by the user in the Export PDF dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfExportConfig {
    pub version: PdfVersionChoice,
    pub archival_standard: PdfArchivalStandard,
    pub pdf_ua1: bool,
    pub tagged: bool,
}

impl Default for PdfExportConfig {
    fn default() -> Self {
        Self {
            version: PdfVersionChoice::V1_7,
            archival_standard: PdfArchivalStandard::None,
            pdf_ua1: false,
            tagged: true,
        }
    }
}

impl PdfExportConfig {
    pub fn build_standards(&self) -> Result<PdfStandards, String> {
        let mut list = vec![self.version.to_standard()];
        if let Some(a_std) = self.archival_standard.to_standard() {
            list.push(a_std);
        }
        if self.pdf_ua1 {
            list.push(PdfStandard::Ua_1);
        }
        PdfStandards::new(&list).map_err(|e| e.message().to_string())
    }

    pub fn summary_badge(&self) -> String {
        let mut parts = vec![format!("PDF {}", self.version.short_label())];
        if self.archival_standard != PdfArchivalStandard::None {
            parts.push(self.archival_standard.code_label().to_string());
        }
        if self.pdf_ua1 {
            parts.push("PDF/UA-1".to_string());
        }
        parts.join(" + ")
    }
}
