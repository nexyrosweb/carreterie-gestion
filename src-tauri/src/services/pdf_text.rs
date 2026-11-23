use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};
use crate::models::import::ExtractionSource;

const MIN_TEXT_CHARS: usize = 40;

#[derive(Debug, Clone)]
pub struct PdfTextExtraction {
    pub text: String,
    pub source: ExtractionSource,
    pub ocr_available: bool,
    pub warnings: Vec<String>,
}

pub fn tesseract_on_path() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Extrait le texte d'un PDF : `pdf-extract`, puis OCR Tesseract (fra) si le texte est quasi vide.
pub fn extract_pdf_text(path: &Path) -> AppResult<PdfTextExtraction> {
    let ocr_available = tesseract_on_path();
    let mut warnings = Vec::new();

    let mut text = match pdf_extract::extract_text(path) {
        Ok(t) => t,
        Err(e) => {
            warnings.push(format!(
                "Lecture PDF native impossible : {e}. Essai OCR si disponible."
            ));
            String::new()
        }
    };

    let trimmed_len = text.trim().chars().count();
    let mut source = if trimmed_len >= MIN_TEXT_CHARS {
        ExtractionSource::Texte
    } else {
        ExtractionSource::Aucun
    };

    if trimmed_len < MIN_TEXT_CHARS {
        if ocr_available {
            match run_tesseract_on_pdf(path) {
                Ok(ocr_text) if ocr_text.trim().chars().count() >= MIN_TEXT_CHARS => {
                    text = ocr_text;
                    source = ExtractionSource::Ocr;
                }
                Ok(_) => {
                    warnings.push(
                        "OCR Tesseract n'a pas produit assez de texte. Saisie manuelle requise."
                            .to_string(),
                    );
                }
                Err(e) => {
                    warnings.push(format!("Échec OCR Tesseract : {e}"));
                }
            }
        } else if trimmed_len == 0 {
            warnings.push(
                "PDF sans couche texte. Installez Tesseract OCR (fra) pour les scans, ou saisissez les champs manuellement.".to_string(),
            );
        }
    }

    Ok(PdfTextExtraction {
        text,
        source,
        ocr_available,
        warnings,
    })
}

fn run_tesseract_on_pdf(path: &Path) -> AppResult<String> {
    let output = Command::new("tesseract")
        .arg(path)
        .arg("-l")
        .arg("fra")
        .arg("stdout")
        .output()
        .map_err(|e| AppError::interne(format!("Impossible d'exécuter tesseract : {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::interne(format!(
            "Tesseract a échoué : {stderr}"
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}