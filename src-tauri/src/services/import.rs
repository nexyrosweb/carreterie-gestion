use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{NaiveDate, Utc};
use rusqlite::params;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::ZipArchive;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::import::{
    ContactMatchCandidate, ExtractionSource, ImportAnalyzeResult, ImportDirection,
    ImportDocumentType, ImportListFilters, ImportStatus, ImportValidateInput, ImportedDocument,
};
use crate::models::invoice::{InvoiceDraftInput, InvoiceLineInput};
use crate::models::supplier_invoice::SupplierInvoiceInput;
use crate::paths::{imports_dir, max_import_bytes};
use crate::services::audit::AuditService;
use crate::services::contacts::ContactsService;
use crate::services::invoices::InvoicesService;
use crate::services::pdf_parse::{check_amounts_coherence, parse_french_invoice_text};
use crate::services::pdf_text::extract_pdf_text;
use crate::services::settings::SettingsService;
use crate::services::suppliers::SuppliersService;

#[derive(Debug, Clone, Serialize)]
pub struct ImportLimits {
    pub max_bytes: u64,
    pub max_mb: u64,
    pub free_disk_bytes: u64,
    pub accepted_extensions: Vec<String>,
    pub accepted_label: String,
}

pub struct ImportService;

impl ImportService {
    pub fn limits() -> AppResult<ImportLimits> {
        let max_bytes = max_import_bytes()?;
        let free = crate::paths::free_disk_bytes(&imports_dir()?)?;
        Ok(ImportLimits {
            max_bytes,
            max_mb: (max_bytes / (1024 * 1024)).max(1),
            free_disk_bytes: free,
            accepted_extensions: vec!["pdf".into(), "zip".into()],
            accepted_label: "PDF, ZIP (plusieurs PDF)".into(),
        })
    }

    pub fn analyze_paths(
        db: &DbPool,
        user_id: &str,
        paths: Vec<String>,
        default_direction: &str,
    ) -> AppResult<Vec<ImportAnalyzeResult>> {
        let direction = ImportDirection::from_db_str(default_direction)
            .unwrap_or(ImportDirection::Fournisseur);
        let imports = imports_dir()?;
        fs::create_dir_all(&imports)?;
        let max_bytes = max_import_bytes()?;

        let mut results = Vec::new();
        for path_str in paths {
            let path = PathBuf::from(&path_str);
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();

            if ext == "zip" {
                match extract_pdfs_from_zip(&path, max_bytes) {
                    Ok(pdfs) => {
                        if pdfs.is_empty() {
                            results.push(ImportAnalyzeResult {
                                document: stub_error_document(
                                    &path_str,
                                    "Aucune archive PDF trouvée dans le ZIP.",
                                ),
                                matches: vec![],
                                warnings: vec![
                                    "Le ZIP ne contient aucun fichier .pdf.".to_string(),
                                ],
                            });
                            continue;
                        }
                        for pdf_path in pdfs {
                            let display = pdf_path.display().to_string();
                            match Self::analyze_one(
                                db,
                                user_id,
                                &display,
                                direction,
                                &imports,
                                max_bytes,
                            ) {
                                Ok(r) => results.push(r),
                                Err(e) => {
                                    results.push(ImportAnalyzeResult {
                                        document: stub_error_document(&display, &e.to_string()),
                                        matches: vec![],
                                        warnings: vec![e.to_string()],
                                    });
                                }
                            }
                            let _ = fs::remove_file(&pdf_path);
                        }
                    }
                    Err(e) => {
                        results.push(ImportAnalyzeResult {
                            document: stub_error_document(&path_str, &e.to_string()),
                            matches: vec![],
                            warnings: vec![e.to_string()],
                        });
                    }
                }
                continue;
            }

            match Self::analyze_one(db, user_id, &path_str, direction, &imports, max_bytes) {
                Ok(r) => results.push(r),
                Err(e) => {
                    results.push(ImportAnalyzeResult {
                        document: stub_error_document(&path_str, &e.to_string()),
                        matches: vec![],
                        warnings: vec![e.to_string()],
                    });
                }
            }
        }
        Ok(results)
    }

    fn analyze_one(
        db: &DbPool,
        user_id: &str,
        source_path: &str,
        direction: ImportDirection,
        imports: &Path,
        max_bytes: u64,
    ) -> AppResult<ImportAnalyzeResult> {
        let path = PathBuf::from(source_path);
        validate_pdf_file(&path, max_bytes)?;
        let file_hash = sha256_file(&path)?;

        if let Some(existing) = Self::get_by_hash(db, &file_hash)? {
            let mut warnings = vec!["Document déjà importé (empreinte SHA-256 identique).".to_string()];
            if existing.status != ImportStatus::Doublon {
                warnings.push("Statut précédent : enregistrement existant retourné.".to_string());
            }
            let matches = Self::match_contacts_for_doc(db, &existing)?;
            return Ok(ImportAnalyzeResult {
                document: existing,
                matches,
                warnings,
            });
        }

        let stored_name = format!("{file_hash}.pdf");
        let stored_path = imports.join(&stored_name);
        if !stored_path.exists() {
            fs::copy(&path, &stored_path)?;
        }

        let original_filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf")
            .to_string();

        let extraction = extract_pdf_text(&stored_path)?;
        let mut warnings = extraction.warnings;

        let excerpt = if extraction.text.len() > 4000 {
            Some(extraction.text[..4000].to_string())
        } else if extraction.text.is_empty() {
            None
        } else {
            Some(extraction.text.clone())
        };

        let parse = parse_french_invoice_text(&extraction.text);
        let amounts_coherent = parse.amounts_coherent;

        let mut status = ImportStatus::EnAttente;
        if Self::is_business_duplicate(
            db,
            parse.fields.invoice_number.as_deref(),
            parse.fields.siret.as_deref(),
            None,
        )? {
            status = ImportStatus::Doublon;
            warnings.push(
                "Doublon métier : même numéro de facture et SIRET déjà enregistrés.".to_string(),
            );
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let uncertain_json = serde_json::to_string(&parse.confidence.uncertain_fields)
            .unwrap_or_else(|_| "[]".to_string());
        let missing_json = serde_json::to_string(&parse.confidence.missing_fields)
            .unwrap_or_else(|_| "[]".to_string());
        let amounts_i = amounts_coherent.map(|b| if b { 1i32 } else { 0 });

        db.with_conn_mut(|conn| {
            conn.execute(
                r#"INSERT INTO imported_documents (
                    id, file_hash, original_filename, stored_path, direction, document_type, status,
                    extraction_source, issuer_name, party_name, party_address, siren, siret,
                    invoice_number, issue_date, payment_date, payment_method, payment_status,
                    description, total_ht, tax_rate, total_tva, total_ttc, related_invoice_ref,
                    contact_id, linked_draft_invoice_id, linked_supplier_invoice_id,
                    uncertain_fields, missing_fields, amounts_coherent, raw_text_excerpt,
                    ocr_available, notes, created_at, updated_at, validated_at, validated_by
                ) VALUES (
                    ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,
                    ?21,?22,?23,?24,NULL,NULL,NULL,?25,?26,?27,?28,?29,NULL,?30,?31,NULL,NULL
                )"#,
                params![
                    id,
                    file_hash,
                    original_filename,
                    stored_path.to_string_lossy().to_string(),
                    direction.as_db_str(),
                    parse.document_type.as_db_str(),
                    status.as_db_str(),
                    extraction.source.as_db_str(),
                    parse.fields.issuer_name,
                    parse.fields.party_name,
                    parse.fields.party_address,
                    parse.fields.siren,
                    parse.fields.siret,
                    parse.fields.invoice_number,
                    parse.fields.issue_date,
                    parse.fields.payment_date,
                    parse.fields.payment_method,
                    parse.fields.payment_status,
                    parse.fields.description,
                    parse.fields.total_ht,
                    parse.fields.tax_rate,
                    parse.fields.total_tva,
                    parse.fields.total_ttc,
                    parse.fields.related_invoice_ref,
                    uncertain_json,
                    missing_json,
                    amounts_i,
                    excerpt,
                    if extraction.ocr_available { 1i32 } else { 0i32 },
                    now,
                    now,
                ],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "import_pdf_analyse",
            Some("imported_document"),
            Some(&id),
            Some(&original_filename),
        )?;

        let document = Self::get(db, &id)?;
        let matches = Self::match_contacts(
            db,
            document.siret.as_deref(),
            document.siren.as_deref(),
            document.party_name.as_deref(),
        )?;

        Ok(ImportAnalyzeResult {
            document,
            matches,
            warnings,
        })
    }

    pub fn list(db: &DbPool, filters: ImportListFilters) -> AppResult<Vec<ImportedDocument>> {
        const SELECT: &str = "SELECT id, file_hash, original_filename, stored_path, direction, document_type, status,
                extraction_source, issuer_name, party_name, party_address, siren, siret, invoice_number,
                issue_date, payment_date, payment_method, payment_status, description, total_ht, tax_rate,
                total_tva, total_ttc, related_invoice_ref, contact_id, linked_draft_invoice_id,
                linked_supplier_invoice_id, uncertain_fields, missing_fields, amounts_coherent,
                raw_text_excerpt, ocr_available, notes, created_at, updated_at, validated_at, validated_by
                FROM imported_documents";

        db.with_conn(|conn| {
            let mut all = conn.prepare(&format!("{SELECT} ORDER BY created_at DESC"))?;
            let rows = all.query_map([], map_imported)?;
            let mut out = Vec::new();
            for r in rows {
                let doc = r?;
                if let Some(s) = filters.status.as_deref().filter(|s| !s.is_empty()) {
                    if doc.status.as_db_str() != s {
                        continue;
                    }
                }
                if let Some(d) = filters.direction.as_deref().filter(|s| !s.is_empty()) {
                    if doc.direction.as_db_str() != d {
                        continue;
                    }
                }
                if let Some(dt) = filters.document_type.as_deref().filter(|s| !s.is_empty()) {
                    if doc.document_type.as_db_str() != dt {
                        continue;
                    }
                }
                if let Some(cid) = filters.contact_id.as_deref().filter(|s| !s.is_empty()) {
                    if doc.contact_id.as_deref() != Some(cid) {
                        continue;
                    }
                }
                if let Some(year) = filters.year {
                    let prefix = format!("{year}-");
                    if !doc
                        .issue_date
                        .as_deref()
                        .is_some_and(|d| d.starts_with(&prefix))
                    {
                        continue;
                    }
                }
                out.push(doc);
            }
            Ok(out)
        })
    }

    pub fn get(db: &DbPool, id: &str) -> AppResult<ImportedDocument> {
        db.with_conn(|conn| {
            conn.query_row(
                "SELECT id, file_hash, original_filename, stored_path, direction, document_type, status,
                extraction_source, issuer_name, party_name, party_address, siren, siret, invoice_number,
                issue_date, payment_date, payment_method, payment_status, description, total_ht, tax_rate,
                total_tva, total_ttc, related_invoice_ref, contact_id, linked_draft_invoice_id,
                linked_supplier_invoice_id, uncertain_fields, missing_fields, amounts_coherent,
                raw_text_excerpt, ocr_available, notes, created_at, updated_at, validated_at, validated_by
                FROM imported_documents WHERE id = ?1",
                params![id],
                map_imported,
            )
            .map_err(|_| AppError::Introuvable)
        })
    }

    pub fn get_pdf_base64(db: &DbPool, id: &str) -> AppResult<String> {
        let doc = Self::get(db, id)?;
        let path = PathBuf::from(&doc.stored_path);
        if !path.is_file() {
            return Err(AppError::Introuvable);
        }
        let mut file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(STANDARD.encode(bytes))
    }

    pub fn match_contacts(
        db: &DbPool,
        siret: Option<&str>,
        siren: Option<&str>,
        name: Option<&str>,
    ) -> AppResult<Vec<ContactMatchCandidate>> {
        let contacts = ContactsService::list(db, name)?;
        let mut out = Vec::new();
        let name_q = name.map(|n| n.trim().to_lowercase());

        for c in contacts {
            let mut score = 0.0f32;
            let mut reason = String::new();

            if let Some(st) = siret.filter(|s| !s.is_empty()) {
                if c.siret.as_deref() == Some(st) {
                    score = 1.0;
                    reason = "SIRET identique".to_string();
                }
            }
            if score < 1.0 {
                if let Some(sn) = siren.filter(|s| !s.is_empty()) {
                    if let Some(cs) = c.siret.as_deref() {
                        if cs.starts_with(sn) {
                            score = score.max(0.85);
                            reason = "SIREN correspond (9 premiers chiffres du SIRET)".to_string();
                        }
                    }
                }
            }
            if score < 0.5 {
                if let Some(ref q) = name_q {
                    let cn = c.name.to_lowercase();
                    if cn == *q {
                        score = score.max(0.75);
                        reason = "Nom identique".to_string();
                    } else if cn.contains(q) || q.contains(&cn) {
                        score = score.max(0.5);
                        reason = "Nom partiellement similaire".to_string();
                    }
                }
            }

            if score >= 0.5 {
                out.push(ContactMatchCandidate {
                    id: c.id,
                    name: c.name,
                    siret: c.siret,
                    score,
                    match_reason: reason,
                });
            }
        }

        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        Ok(out)
    }

    pub fn validate(db: &DbPool, user_id: &str, input: ImportValidateInput) -> AppResult<ImportedDocument> {
        let doc = Self::get(db, &input.id)?;
        if doc.status != ImportStatus::EnAttente && doc.status != ImportStatus::Doublon {
            return Err(AppError::interdit(
                "Seuls les imports en attente ou doublons peuvent être validés.",
            ));
        }

        let amounts_coherent = check_amounts_coherence(
            input.total_ht.as_deref(),
            input.total_tva.as_deref(),
            input.total_ttc.as_deref(),
        );
        let amounts_i = amounts_coherent.map(|b| if b { 1i32 } else { 0 });

        let contact_id = resolve_contact(db, user_id, &input)?;

        let now = Utc::now().to_rfc3339();
        let validated_at = now.clone();

        let mut linked_draft: Option<String> = doc.linked_draft_invoice_id.clone();
        let mut linked_supplier: Option<String> = doc.linked_supplier_invoice_id.clone();

        if input.create_linked_record {
            let cid = contact_id.as_deref().ok_or_else(|| {
                AppError::validation(
                    "Un contact est obligatoire pour créer une facture liée (brouillon ou fournisseur).",
                )
            })?;
            match input.direction {
                ImportDirection::Client => {
                    linked_draft = Some(create_linked_client_draft(
                        db,
                        user_id,
                        cid,
                        &input,
                    )?);
                }
                ImportDirection::Fournisseur => {
                    linked_supplier = Some(create_linked_supplier(
                        db,
                        user_id,
                        cid,
                        &doc.stored_path,
                        &input,
                    )?);
                }
            }
        }

        db.with_conn_mut(|conn| {
            conn.execute(
                r#"UPDATE imported_documents SET
                    direction = ?2, document_type = ?3, status = 'valide', extraction_source = 'manuel',
                    issuer_name = ?4, party_name = ?5, party_address = ?6, siren = ?7, siret = ?8,
                    invoice_number = ?9, issue_date = ?10, payment_date = ?11, payment_method = ?12,
                    payment_status = ?13, description = ?14, total_ht = ?15, tax_rate = ?16, total_tva = ?17,
                    total_ttc = ?18, related_invoice_ref = ?19, contact_id = ?20,
                    linked_draft_invoice_id = ?21, linked_supplier_invoice_id = ?22,
                    amounts_coherent = ?23, notes = ?24, updated_at = ?25, validated_at = ?26, validated_by = ?27
                WHERE id = ?1"#,
                params![
                    input.id,
                    input.direction.as_db_str(),
                    input.document_type.as_db_str(),
                    input.issuer_name,
                    input.party_name,
                    input.party_address,
                    input.siren,
                    input.siret,
                    input.invoice_number,
                    input.issue_date,
                    input.payment_date,
                    input.payment_method,
                    input.payment_status,
                    input.description,
                    input.total_ht,
                    input.tax_rate,
                    input.total_tva,
                    input.total_ttc,
                    input.related_invoice_ref,
                    contact_id,
                    linked_draft,
                    linked_supplier,
                    amounts_i,
                    input.notes,
                    now,
                    validated_at,
                    user_id,
                ],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "import_pdf_valide",
            Some("imported_document"),
            Some(&input.id),
            input.invoice_number.as_deref(),
        )?;

        Self::get(db, &input.id)
    }

    pub fn reject(db: &DbPool, user_id: &str, id: &str, reason: Option<String>) -> AppResult<ImportedDocument> {
        let doc = Self::get(db, id)?;
        if doc.status == ImportStatus::Valide {
            return Err(AppError::interdit("Un import validé ne peut pas être rejeté."));
        }
        let now = Utc::now().to_rfc3339();
        let note = reason.or(doc.notes);

        db.with_conn_mut(|conn| {
            conn.execute(
                "UPDATE imported_documents SET status = 'rejete', notes = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, note, now],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "import_pdf_rejete",
            Some("imported_document"),
            Some(id),
            None,
        )?;

        Self::get(db, id)
    }

    fn get_by_hash(db: &DbPool, hash: &str) -> AppResult<Option<ImportedDocument>> {
        db.with_conn(|conn| {
            let r = conn.query_row(
                "SELECT id, file_hash, original_filename, stored_path, direction, document_type, status,
                extraction_source, issuer_name, party_name, party_address, siren, siret, invoice_number,
                issue_date, payment_date, payment_method, payment_status, description, total_ht, tax_rate,
                total_tva, total_ttc, related_invoice_ref, contact_id, linked_draft_invoice_id,
                linked_supplier_invoice_id, uncertain_fields, missing_fields, amounts_coherent,
                raw_text_excerpt, ocr_available, notes, created_at, updated_at, validated_at, validated_by
                FROM imported_documents WHERE file_hash = ?1",
                params![hash],
                map_imported,
            );
            match r {
                Ok(doc) => Ok(Some(doc)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::BaseDeDonnees(e)),
            }
        })
    }

    fn is_business_duplicate(
        db: &DbPool,
        invoice_number: Option<&str>,
        siret: Option<&str>,
        contact_id: Option<&str>,
    ) -> AppResult<bool> {
        let Some(num) = invoice_number.filter(|s| !s.trim().is_empty()) else {
            return Ok(false);
        };
        let has_siret = siret.filter(|s| !s.trim().is_empty());
        let has_contact = contact_id.filter(|s| !s.trim().is_empty());
        if has_siret.is_none() && has_contact.is_none() {
            return Ok(false);
        }

        db.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT COUNT(*) FROM imported_documents WHERE invoice_number = ?1 AND status IN ('en_attente','valide')",
            );
            let count: i64 = if let Some(st) = has_siret {
                sql.push_str(" AND siret = ?2");
                conn.query_row(&sql, params![num, st], |row| row.get(0))?
            } else if let Some(cid) = has_contact {
                sql.push_str(" AND contact_id = ?2");
                conn.query_row(&sql, params![num, cid], |row| row.get(0))?
            } else {
                0
            };
            Ok(count > 0)
        })
    }

    fn match_contacts_for_doc(db: &DbPool, doc: &ImportedDocument) -> AppResult<Vec<ContactMatchCandidate>> {
        Self::match_contacts(
            db,
            doc.siret.as_deref(),
            doc.siren.as_deref(),
            doc.party_name.as_deref(),
        )
    }
}

fn resolve_contact(
    db: &DbPool,
    user_id: &str,
    input: &ImportValidateInput,
) -> AppResult<Option<String>> {
    if let Some(cid) = input.contact_id.as_ref().filter(|s| !s.is_empty()) {
        let _ = ContactsService::get(db, cid)?;
        return Ok(Some(cid.clone()));
    }
    if let Some(ref contact_in) = input.create_new_contact {
        let contact = ContactsService::create(db, user_id, contact_in.clone())?;
        return Ok(Some(contact.id));
    }
    Ok(None)
}

fn create_linked_client_draft(
    db: &DbPool,
    user_id: &str,
    contact_id: &str,
    input: &ImportValidateInput,
) -> AppResult<String> {
    let issue_date = input
        .issue_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let ht = input
        .total_ht
        .as_deref()
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or(dec!(0));
    let tax = input
        .tax_rate
        .as_deref()
        .and_then(|s| s.parse::<Decimal>().ok())
        .unwrap_or(SettingsService::get_default_tax(db).unwrap_or(dec!(20)));
    let desc = input
        .description
        .clone()
        .or_else(|| input.invoice_number.clone())
        .unwrap_or_else(|| "Import PDF".to_string());

    let draft = InvoiceDraftInput {
        contact_id: contact_id.to_string(),
        issue_date,
        due_date: input
            .payment_date
            .as_deref()
            .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()),
        customer_notes: input.notes.clone(),
        lines: vec![InvoiceLineInput {
            description: desc,
            quantity: dec!(1),
            unit_price_ht: ht,
            tax_rate_percent: tax,
        }],
    };
    let inv = InvoicesService::create_draft(db, user_id, draft)?;
    Ok(inv.id)
}

fn create_linked_supplier(
    db: &DbPool,
    user_id: &str,
    contact_id: &str,
    attachment_path: &str,
    input: &ImportValidateInput,
) -> AppResult<String> {
    let invoice_date = input
        .issue_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let sup = SupplierInvoiceInput {
        supplier_contact_id: Some(contact_id.to_string()),
        reference: input.invoice_number.clone(),
        invoice_date,
        amount_ht: input.total_ht.as_deref().and_then(|s| s.parse().ok()),
        amount_ttc: input.total_ttc.as_deref().and_then(|s| s.parse().ok()),
        tax_rate_percent: input.tax_rate.as_deref().and_then(|s| s.parse().ok()),
        notes: input.notes.clone(),
        attachment_path: Some(attachment_path.to_string()),
    };
    let rec = SuppliersService::create(db, user_id, sup)?;
    Ok(rec.id)
}

fn validate_pdf_file(path: &Path, max_bytes: u64) -> AppResult<()> {
    if !path.is_file() {
        return Err(AppError::validation("Fichier introuvable."));
    }
    let meta = fs::metadata(path)?;
    if meta.len() > max_bytes {
        let max_mb = (max_bytes / (1024 * 1024)).max(1);
        return Err(AppError::validation(format!(
            "PDF trop volumineux (maximum {max_mb} Mo selon l'espace disque disponible)."
        )));
    }
    if meta.len() < 5 {
        return Err(AppError::validation("Fichier trop petit pour être un PDF."));
    }
    let mut buf = [0u8; 5];
    let mut f = File::open(path)?;
    f.read_exact(&mut buf)?;
    if &buf != b"%PDF-" {
        return Err(AppError::validation(
            "Le fichier n'est pas un PDF valide (en-tête %PDF- manquant).",
        ));
    }
    Ok(())
}

fn extract_pdfs_from_zip(zip_path: &Path, max_bytes: u64) -> AppResult<Vec<PathBuf>> {
    if !zip_path.is_file() {
        return Err(AppError::validation("Archive ZIP introuvable."));
    }
    let meta = fs::metadata(zip_path)?;
    if meta.len() > max_bytes {
        let max_mb = (max_bytes / (1024 * 1024)).max(1);
        return Err(AppError::validation(format!(
            "ZIP trop volumineux (maximum {max_mb} Mo selon l'espace disque disponible)."
        )));
    }

    let file = File::open(zip_path)?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| AppError::validation(format!("ZIP invalide : {e}")))?;

    let staging = imports_dir()?.join(format!("zip-{}", Uuid::new_v4()));
    fs::create_dir_all(&staging)?;

    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::validation(format!("Lecture ZIP impossible : {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .enclosed_name()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(entry.name()));
        let ext = name
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "pdf" {
            continue;
        }
        let file_name = name
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf");
        let dest = staging.join(format!("{i}_{file_name}"));
        {
            let mut out_file = File::create(&dest)?;
            copy(&mut entry, &mut out_file)?;
            out_file.flush()?;
        }
        if let Err(e) = validate_pdf_file(&dest, max_bytes) {
            let _ = fs::remove_file(&dest);
            return Err(e);
        }
        out.push(dest);
    }
    Ok(out)
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn parse_json_array(s: Option<String>) -> Vec<String> {
    s.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default()
}

fn map_imported(row: &rusqlite::Row<'_>) -> rusqlite::Result<ImportedDocument> {
    let direction_s: String = row.get(4)?;
    let doc_type_s: String = row.get(5)?;
    let status_s: String = row.get(6)?;
    let source_s: String = row.get(7)?;
    let amounts_i: Option<i64> = row.get(29)?;
    let ocr_i: i64 = row.get(31)?;
    let created_at: String = row.get(33)?;
    let updated_at: String = row.get(34)?;
    let validated_at: Option<String> = row.get(35)?;

    Ok(ImportedDocument {
        id: row.get(0)?,
        file_hash: row.get(1)?,
        original_filename: row.get(2)?,
        stored_path: row.get(3)?,
        direction: ImportDirection::from_db_str(&direction_s).unwrap_or(ImportDirection::Fournisseur),
        document_type: ImportDocumentType::from_db_str(&doc_type_s).unwrap_or(ImportDocumentType::Inconnu),
        status: ImportStatus::from_db_str(&status_s).unwrap_or(ImportStatus::EnAttente),
        extraction_source: ExtractionSource::from_db_str(&source_s).unwrap_or(ExtractionSource::Aucun),
        issuer_name: row.get(8)?,
        party_name: row.get(9)?,
        party_address: row.get(10)?,
        siren: row.get(11)?,
        siret: row.get(12)?,
        invoice_number: row.get(13)?,
        issue_date: row.get(14)?,
        payment_date: row.get(15)?,
        payment_method: row.get(16)?,
        payment_status: row.get(17)?,
        description: row.get(18)?,
        total_ht: row.get(19)?,
        tax_rate: row.get(20)?,
        total_tva: row.get(21)?,
        total_ttc: row.get(22)?,
        related_invoice_ref: row.get(23)?,
        contact_id: row.get(24)?,
        linked_draft_invoice_id: row.get(25)?,
        linked_supplier_invoice_id: row.get(26)?,
        uncertain_fields: parse_json_array(row.get(27)?),
        missing_fields: parse_json_array(row.get(28)?),
        amounts_coherent: amounts_i.map(|v| v != 0),
        raw_text_excerpt: row.get(30)?,
        ocr_available: ocr_i != 0,
        notes: row.get(32)?,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        validated_at: validated_at.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        validated_by: row.get(36)?,
    })
}

fn stub_error_document(path: &str, err: &str) -> ImportedDocument {
    let now = Utc::now();
    ImportedDocument {
        id: String::new(),
        file_hash: String::new(),
        original_filename: PathBuf::from(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string(),
        stored_path: String::new(),
        direction: ImportDirection::Fournisseur,
        document_type: ImportDocumentType::Inconnu,
        status: ImportStatus::Rejete,
        extraction_source: ExtractionSource::Aucun,
        issuer_name: None,
        party_name: None,
        party_address: None,
        siren: None,
        siret: None,
        invoice_number: None,
        issue_date: None,
        payment_date: None,
        payment_method: None,
        payment_status: None,
        description: None,
        total_ht: None,
        tax_rate: None,
        total_tva: None,
        total_ttc: None,
        related_invoice_ref: None,
        contact_id: None,
        linked_draft_invoice_id: None,
        linked_supplier_invoice_id: None,
        uncertain_fields: vec![],
        missing_fields: vec![],
        amounts_coherent: None,
        raw_text_excerpt: None,
        ocr_available: false,
        notes: Some(err.to_string()),
        created_at: now,
        updated_at: now,
        validated_at: None,
        validated_by: None,
    }
}
