use std::fs::File;
use std::path::PathBuf;

use chrono::{NaiveDate, Utc};
use csv::Writer;
use serde::Serialize;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::settings::AppSettings;
use crate::services::audit::AuditService;
use crate::services::settings::SettingsService;

#[derive(Serialize)]
struct InvoiceCsvRow {
    numero: String,
    statut: String,
    type_document: String,
    client_id: String,
    date_emission: String,
    total_ht: String,
    total_tva: String,
    total_ttc: String,
    facture_liee: String,
}

#[derive(Serialize)]
struct FecLiteRow {
    #[serde(rename = "JournalCode")]
    journal_code: String,
    #[serde(rename = "EcritureDate")]
    ecriture_date: String,
    #[serde(rename = "CompteNum")]
    compte_num: String,
    #[serde(rename = "CompteLib")]
    compte_lib: String,
    #[serde(rename = "PieceRef")]
    piece_ref: String,
    #[serde(rename = "EcritureLib")]
    ecriture_lib: String,
    #[serde(rename = "Debit")]
    debit: String,
    #[serde(rename = "Credit")]
    credit: String,
}

#[derive(Clone)]
pub struct AccountingExportInput {
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
}

pub struct ArchivesService;

impl ArchivesService {
    pub fn export_invoices_csv(db: &DbPool, user_id: &str) -> AppResult<PathBuf> {
        let settings: AppSettings = SettingsService::load(db)?;
        let export_dir = export_dir(&settings)?;
        let file_name = format!(
            "export_factures_{}.csv",
            Utc::now().format("%Y%m%d_%H%M%S")
        );
        let path = export_dir.join(file_name);

        let rows: Vec<InvoiceCsvRow> = db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT number, status, invoice_kind, contact_id, issue_date, total_ht, total_tva, total_ttc, related_invoice_id FROM invoices WHERE status IN ('emise', 'avoir') ORDER BY issued_at, number",
            )?;
            let mapped = stmt.query_map([], |row| {
                Ok(InvoiceCsvRow {
                    numero: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    statut: row.get(1)?,
                    type_document: row.get(2)?,
                    client_id: row.get(3)?,
                    date_emission: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                    total_ht: row.get(5)?,
                    total_tva: row.get(6)?,
                    total_ttc: row.get(7)?,
                    facture_liee: row
                        .get::<_, Option<String>>(8)?
                        .unwrap_or_default(),
                })
            })?;
            let mut out = Vec::new();
            for r in mapped {
                out.push(r?);
            }
            Ok(out)
        })?;

        if rows.is_empty() {
            return Err(AppError::validation(
                "Aucune facture émise à exporter.",
            ));
        }

        let file = File::create(&path)?;
        let mut wtr = Writer::from_writer(file);
        for row in rows {
            wtr.serialize(row)?;
        }
        wtr.flush()?;

        AuditService::log(
            db,
            user_id,
            "export_csv_factures",
            Some("archive"),
            None,
            Some(path.to_string_lossy().as_ref()),
        )?;
        Ok(path)
    }

    pub fn export_fec_lite(
        db: &DbPool,
        user_id: &str,
        input: AccountingExportInput,
    ) -> AppResult<PathBuf> {
        let settings = SettingsService::load(db)?;
        let export_dir = export_dir(&settings)?;
        let path = export_dir.join(format!(
            "export_fec_lite_{}.csv",
            Utc::now().format("%Y%m%d_%H%M%S")
        ));

        let from = input.date_from.map(|d| d.format("%Y-%m-%d").to_string());
        let to = input.date_to.map(|d| d.format("%Y-%m-%d").to_string());

        let rows: Vec<FecLiteRow> = db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT i.number, i.issue_date, i.invoice_kind, i.total_ht, i.total_tva, i.total_ttc, c.name
                 FROM invoices i
                 JOIN contacts c ON c.id = i.contact_id
                 WHERE i.status IN ('emise', 'avoir')
                   AND i.invoice_kind IN ('facture', 'avoir')
                   AND (?1 IS NULL OR i.issue_date >= ?1)
                   AND (?2 IS NULL OR i.issue_date <= ?2)
                 ORDER BY i.issue_date, i.number",
            )?;
            let mapped = stmt.query_map(rusqlite::params![from, to], |row| {
                let number: String = row.get::<_, Option<String>>(0)?.unwrap_or_default();
                let date: String = row
                    .get::<_, Option<String>>(1)?
                    .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string())
                    .replace('-', "");
                let kind: String = row.get(2)?;
                let ht: String = row.get(3)?;
                let tva: String = row.get(4)?;
                let ttc: String = row.get(5)?;
                let client: String = row.get(6)?;
                let is_avoir = kind == "avoir";
                let sign = if is_avoir { -1.0_f64 } else { 1.0 };
                let ht_f: f64 = ht.parse().unwrap_or(0.0) * sign;
                let tva_f: f64 = tva.parse().unwrap_or(0.0) * sign;
                let ttc_f: f64 = ttc.parse().unwrap_or(0.0) * sign;
                let lib = format!(
                    "{} {} — {}",
                    if is_avoir { "Avoir" } else { "Facture" },
                    number,
                    client
                );

                Ok(vec![
                    FecLiteRow {
                        journal_code: "VE".into(),
                        ecriture_date: date.clone(),
                        compte_num: "411000".into(),
                        compte_lib: "Clients".into(),
                        piece_ref: number.clone(),
                        ecriture_lib: lib.clone(),
                        debit: format_amount_f(ttc_f.max(0.0)),
                        credit: format_amount_f((-ttc_f).max(0.0)),
                    },
                    FecLiteRow {
                        journal_code: "VE".into(),
                        ecriture_date: date.clone(),
                        compte_num: "706000".into(),
                        compte_lib: "Prestations de services".into(),
                        piece_ref: number.clone(),
                        ecriture_lib: lib.clone(),
                        debit: format_amount_f((-ht_f).max(0.0)),
                        credit: format_amount_f(ht_f.max(0.0)),
                    },
                    FecLiteRow {
                        journal_code: "VE".into(),
                        ecriture_date: date,
                        compte_num: "445710".into(),
                        compte_lib: "TVA collectée".into(),
                        piece_ref: number,
                        ecriture_lib: lib,
                        debit: format_amount_f((-tva_f).max(0.0)),
                        credit: format_amount_f(tva_f.max(0.0)),
                    },
                ])
            })?;
            let mut out = Vec::new();
            for r in mapped {
                out.extend(r?);
            }
            Ok(out)
        })?;

        if rows.is_empty() {
            return Err(AppError::validation(
                "Aucune écriture à exporter pour la période.",
            ));
        }

        let file = File::create(&path)?;
        let mut wtr = Writer::from_writer(file);
        for row in &rows {
            wtr.serialize(row)?;
        }
        wtr.flush()?;

        AuditService::log(
            db,
            user_id,
            "export_fec_lite",
            Some("archive"),
            None,
            Some(path.to_string_lossy().as_ref()),
        )?;
        Ok(path)
    }
}

fn export_dir(settings: &AppSettings) -> AppResult<PathBuf> {
    let export_dir = if settings.export_folder.trim().is_empty() {
        crate::paths::app_data_root()?
    } else {
        PathBuf::from(&settings.export_folder)
    };
    std::fs::create_dir_all(&export_dir)?;
    Ok(export_dir)
}

fn format_amount_f(v: f64) -> String {
    format!("{v:.2}")
}
