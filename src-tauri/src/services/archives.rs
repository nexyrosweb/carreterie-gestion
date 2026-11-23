use std::fs::File;
use std::path::PathBuf;

use chrono::Utc;
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

pub struct ArchivesService;

impl ArchivesService {
    pub fn export_invoices_csv(db: &DbPool, user_id: &str) -> AppResult<PathBuf> {
        let settings: AppSettings = SettingsService::load(db)?;
        let export_dir = if settings.export_folder.trim().is_empty() {
            crate::paths::app_data_root()?
        } else {
            PathBuf::from(&settings.export_folder)
        };
        std::fs::create_dir_all(&export_dir)?;

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
}