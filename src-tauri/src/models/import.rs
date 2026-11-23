use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::models::contact::ContactInput;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportDirection {
    Client,
    Fournisseur,
}

impl ImportDirection {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ImportDirection::Client => "client",
            ImportDirection::Fournisseur => "fournisseur",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "client" => Some(ImportDirection::Client),
            "fournisseur" => Some(ImportDirection::Fournisseur),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportDocumentType {
    Facture,
    Acompte,
    Avoir,
    Inconnu,
}

impl ImportDocumentType {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ImportDocumentType::Facture => "facture",
            ImportDocumentType::Acompte => "acompte",
            ImportDocumentType::Avoir => "avoir",
            ImportDocumentType::Inconnu => "inconnu",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "facture" => Some(ImportDocumentType::Facture),
            "acompte" => Some(ImportDocumentType::Acompte),
            "avoir" => Some(ImportDocumentType::Avoir),
            "inconnu" => Some(ImportDocumentType::Inconnu),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    EnAttente,
    Valide,
    Rejete,
    Doublon,
}

impl ImportStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ImportStatus::EnAttente => "en_attente",
            ImportStatus::Valide => "valide",
            ImportStatus::Rejete => "rejete",
            ImportStatus::Doublon => "doublon",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "en_attente" => Some(ImportStatus::EnAttente),
            "valide" => Some(ImportStatus::Valide),
            "rejete" => Some(ImportStatus::Rejete),
            "doublon" => Some(ImportStatus::Doublon),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionSource {
    Texte,
    Ocr,
    Manuel,
    Aucun,
}

impl ExtractionSource {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            ExtractionSource::Texte => "texte",
            ExtractionSource::Ocr => "ocr",
            ExtractionSource::Manuel => "manuel",
            ExtractionSource::Aucun => "aucun",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "texte" => Some(ExtractionSource::Texte),
            "ocr" => Some(ExtractionSource::Ocr),
            "manuel" => Some(ExtractionSource::Manuel),
            "aucun" => Some(ExtractionSource::Aucun),
            _ => None,
        }
    }
}

/// Champs extraits d'un PDF (tous optionnels tant que non confirmés).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedFields {
    pub issuer_name: Option<String>,
    pub party_name: Option<String>,
    pub party_address: Option<String>,
    pub siren: Option<String>,
    pub siret: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub payment_date: Option<String>,
    pub payment_method: Option<String>,
    pub payment_status: Option<String>,
    pub description: Option<String>,
    pub total_ht: Option<String>,
    pub tax_rate: Option<String>,
    pub total_tva: Option<String>,
    pub total_ttc: Option<String>,
    pub related_invoice_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldConfidence {
    pub uncertain_fields: Vec<String>,
    pub missing_fields: Vec<String>,
}

impl FieldConfidence {
    pub fn mark_uncertain(fields: &mut Vec<String>, name: &str) {
        if !fields.iter().any(|f| f == name) {
            fields.push(name.to_string());
        }
    }

    pub fn required_missing(fields: &mut Vec<String>, name: &str, value: &Option<String>) {
        if value.as_ref().map(|s| s.trim().is_empty()).unwrap_or(true) {
            if !fields.iter().any(|f| f == name) {
                fields.push(name.to_string());
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedDocument {
    pub id: String,
    pub file_hash: String,
    pub original_filename: String,
    pub stored_path: String,
    pub direction: ImportDirection,
    pub document_type: ImportDocumentType,
    pub status: ImportStatus,
    pub extraction_source: ExtractionSource,
    pub issuer_name: Option<String>,
    pub party_name: Option<String>,
    pub party_address: Option<String>,
    pub siren: Option<String>,
    pub siret: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub payment_date: Option<String>,
    pub payment_method: Option<String>,
    pub payment_status: Option<String>,
    pub description: Option<String>,
    pub total_ht: Option<String>,
    pub tax_rate: Option<String>,
    pub total_tva: Option<String>,
    pub total_ttc: Option<String>,
    pub related_invoice_ref: Option<String>,
    pub contact_id: Option<String>,
    pub linked_draft_invoice_id: Option<String>,
    pub linked_supplier_invoice_id: Option<String>,
    pub uncertain_fields: Vec<String>,
    pub missing_fields: Vec<String>,
    pub amounts_coherent: Option<bool>,
    pub raw_text_excerpt: Option<String>,
    pub ocr_available: bool,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub validated_at: Option<DateTime<Utc>>,
    pub validated_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactMatchCandidate {
    pub id: String,
    pub name: String,
    pub siret: Option<String>,
    pub score: f32,
    pub match_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAnalyzeResult {
    pub document: ImportedDocument,
    pub matches: Vec<ContactMatchCandidate>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportListFilters {
    pub status: Option<String>,
    pub direction: Option<String>,
    pub year: Option<i32>,
    pub contact_id: Option<String>,
    pub document_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportValidateInput {
    pub id: String,
    pub direction: ImportDirection,
    pub document_type: ImportDocumentType,
    pub issuer_name: Option<String>,
    pub party_name: Option<String>,
    pub party_address: Option<String>,
    pub siren: Option<String>,
    pub siret: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub payment_date: Option<String>,
    pub payment_method: Option<String>,
    pub payment_status: Option<String>,
    pub description: Option<String>,
    pub total_ht: Option<String>,
    pub tax_rate: Option<String>,
    pub total_tva: Option<String>,
    pub total_ttc: Option<String>,
    pub related_invoice_ref: Option<String>,
    pub contact_id: Option<String>,
    pub create_new_contact: Option<ContactInput>,
    pub create_linked_record: bool,
    pub notes: Option<String>,
}