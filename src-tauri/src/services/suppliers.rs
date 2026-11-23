use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::supplier_invoice::{SupplierInvoice, SupplierInvoiceInput};
use crate::money::format_amount;
use crate::services::audit::AuditService;

fn map_supplier(row: &rusqlite::Row<'_>) -> rusqlite::Result<SupplierInvoice> {
    let invoice_date: Option<String> = row.get(3)?;
    let created_at: String = row.get(9)?;
    let updated_at: String = row.get(10)?;
    Ok(SupplierInvoice {
        id: row.get(0)?,
        supplier_contact_id: row.get(1)?,
        reference: row.get(2)?,
        invoice_date: invoice_date.and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        amount_ht: row
            .get::<_, Option<String>>(4)?
            .and_then(|s| s.parse().ok()),
        amount_ttc: row
            .get::<_, Option<String>>(5)?
            .and_then(|s| s.parse().ok()),
        tax_rate_percent: row
            .get::<_, Option<String>>(6)?
            .and_then(|s| s.parse().ok()),
        notes: row.get(7)?,
        attachment_path: row.get(8)?,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

pub struct SuppliersService;

impl SuppliersService {
    pub fn list(db: &DbPool) -> AppResult<Vec<SupplierInvoice>> {
        db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, supplier_contact_id, reference, invoice_date, amount_ht, amount_ttc, tax_rate_percent, notes, attachment_path, created_at, updated_at FROM supplier_invoices ORDER BY invoice_date DESC, created_at DESC",
            )?;
            let rows = stmt.query_map([], map_supplier)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }

    pub fn get(db: &DbPool, id: &str) -> AppResult<SupplierInvoice> {
        db.with_conn(|conn| {
            conn.query_row(
                "SELECT id, supplier_contact_id, reference, invoice_date, amount_ht, amount_ttc, tax_rate_percent, notes, attachment_path, created_at, updated_at FROM supplier_invoices WHERE id = ?1",
                params![id],
                map_supplier,
            )
            .map_err(|_| AppError::Introuvable)
        })
    }

    pub fn create(db: &DbPool, user_id: &str, input: SupplierInvoiceInput) -> AppResult<SupplierInvoice> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let date_s = input.invoice_date.map(|d| d.format("%Y-%m-%d").to_string());
        let ht = input.amount_ht.map(format_amount);
        let ttc = input.amount_ttc.map(format_amount);
        let tax = input.tax_rate_percent.map(format_amount);

        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO supplier_invoices (id, supplier_contact_id, reference, invoice_date, amount_ht, amount_ttc, tax_rate_percent, notes, attachment_path, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    id,
                    input.supplier_contact_id,
                    input.reference,
                    date_s,
                    ht,
                    ttc,
                    tax,
                    input.notes,
                    input.attachment_path,
                    now,
                    now
                ],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "facture_fournisseur_creee",
            Some("supplier_invoice"),
            Some(&id),
            input.reference.as_deref(),
        )?;
        Self::get(db, &id)
    }

    pub fn update(
        db: &DbPool,
        user_id: &str,
        id: &str,
        input: SupplierInvoiceInput,
    ) -> AppResult<SupplierInvoice> {
        let now = Utc::now().to_rfc3339();
        let date_s = input.invoice_date.map(|d| d.format("%Y-%m-%d").to_string());
        let ht = input.amount_ht.map(format_amount);
        let ttc = input.amount_ttc.map(format_amount);
        let tax = input.tax_rate_percent.map(format_amount);

        let n = db.with_conn_mut(|conn| {
            conn.execute(
                "UPDATE supplier_invoices SET supplier_contact_id=?1, reference=?2, invoice_date=?3, amount_ht=?4, amount_ttc=?5, tax_rate_percent=?6, notes=?7, attachment_path=?8, updated_at=?9 WHERE id=?10",
                params![
                    input.supplier_contact_id,
                    input.reference,
                    date_s,
                    ht,
                    ttc,
                    tax,
                    input.notes,
                    input.attachment_path,
                    now,
                    id
                ],
            )?;
            Ok(conn.changes())
        })?;
        if n == 0 {
            return Err(AppError::Introuvable);
        }
        AuditService::log(
            db,
            user_id,
            "facture_fournisseur_modifiee",
            Some("supplier_invoice"),
            Some(id),
            None,
        )?;
        Self::get(db, id)
    }

    pub fn delete(db: &DbPool, user_id: &str, id: &str) -> AppResult<()> {
        let n = db.with_conn_mut(|conn| {
            conn.execute("DELETE FROM supplier_invoices WHERE id = ?1", params![id])?;
            Ok(conn.changes())
        })?;
        if n == 0 {
            return Err(AppError::Introuvable);
        }
        AuditService::log(
            db,
            user_id,
            "facture_fournisseur_supprimee",
            Some("supplier_invoice"),
            Some(id),
            None,
        )?;
        Ok(())
    }
}