use chrono::Utc;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::invoice::{InvoiceKind, InvoiceStatus};
use crate::services::audit::AuditService;
use crate::services::invoices::InvoicesService;
use crate::services::pdf::PdfService;

pub struct ContractsService;

impl ContractsService {
    pub fn generate_from_devis(
        db: &DbPool,
        user_id: &str,
        devis_id: &str,
        demo: bool,
    ) -> AppResult<String> {
        ensure_accepted_devis(db, devis_id)?;
        let path = PdfService::generate_cession_contract_pdf(db, devis_id, demo)?;
        crate::paths::assert_path_inside_active_root(&path)?;
        AuditService::log(
            db,
            user_id,
            "contrat_cession_genere",
            Some("invoice"),
            Some(devis_id),
            Some(path.to_string_lossy().as_ref()),
        )?;
        Ok(path.to_string_lossy().to_string())
    }

    pub fn preview_from_devis(db: &DbPool, devis_id: &str, demo: bool) -> AppResult<String> {
        ensure_accepted_devis(db, devis_id)?;
        PdfService::generate_cession_contract_pdf_base64(db, devis_id, demo)
    }
}

fn ensure_accepted_devis(db: &DbPool, devis_id: &str) -> AppResult<()> {
    let devis = InvoicesService::get(db, devis_id)?;
    if devis.invoice_kind != InvoiceKind::Devis {
        return Err(AppError::validation("Le document n'est pas un devis."));
    }
    if devis.status != InvoiceStatus::Emise {
        return Err(AppError::interdit(
            "Seuls les devis émis peuvent générer un contrat de cession.",
        ));
    }
    if devis.accepted_at.is_none() {
        return Err(AppError::interdit(
            "Marquez d'abord le devis comme accepté avant de générer le contrat.",
        ));
    }
    let _ = Utc::now();
    Ok(())
}
