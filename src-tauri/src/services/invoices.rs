use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::invoice::{
    Invoice, InvoiceDraftInput, InvoiceKind, InvoiceLine, InvoiceLineInput, InvoiceStatus,
};
use crate::money::{format_amount, line_amounts, round_eur};
use crate::services::audit::AuditService;
use crate::services::settings::SettingsService;

pub struct InvoicesService;

fn parse_date_opt(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn decimal_from_row(s: &str) -> Decimal {
    s.parse::<Decimal>().map(round_eur).unwrap_or(dec!(0))
}

fn map_line(row: &rusqlite::Row<'_>) -> rusqlite::Result<InvoiceLine> {
    Ok(InvoiceLine {
        id: row.get(0)?,
        invoice_id: row.get(1)?,
        position: row.get(2)?,
        description: row.get(3)?,
        quantity: decimal_from_row(&row.get::<_, String>(4)?),
        unit_price_ht: decimal_from_row(&row.get::<_, String>(5)?),
        tax_rate_percent: decimal_from_row(&row.get::<_, String>(6)?),
        line_ht: decimal_from_row(&row.get::<_, String>(7)?),
        line_tva: decimal_from_row(&row.get::<_, String>(8)?),
        line_ttc: decimal_from_row(&row.get::<_, String>(9)?),
    })
}

fn load_lines(conn: &Connection, invoice_id: &str) -> AppResult<Vec<InvoiceLine>> {
    let mut stmt = conn.prepare(
        "SELECT id, invoice_id, position, description, quantity, unit_price_ht, tax_rate_percent, line_ht, line_tva, line_ttc FROM invoice_lines WHERE invoice_id = ?1 ORDER BY position",
    )?;
    let rows = stmt.query_map(params![invoice_id], map_line)?;
    let mut lines = Vec::new();
    for r in rows {
        lines.push(r?);
    }
    Ok(lines)
}

fn map_invoice(conn: &Connection, invoice_id: &str) -> AppResult<Invoice> {
    let inv = conn.query_row(
        "SELECT id, number, contact_id, status, invoice_kind, related_invoice_id, issue_date, due_date, customer_notes, total_ht, total_tva, total_ttc, COALESCE(amount_paid, '0.00'), COALESCE(payment_status, 'impayee'), created_at, updated_at, issued_at FROM invoices WHERE id = ?1",
        params![invoice_id],
        |row| {
            let status_s: String = row.get(3)?;
            let kind_s: String = row.get(4)?;
            let issue_date: Option<String> = row.get(6)?;
            let due_date: Option<String> = row.get(7)?;
            let created_at: String = row.get(14)?;
            let updated_at: String = row.get(15)?;
            let issued_at: Option<String> = row.get(16)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                InvoiceStatus::from_db_str(&status_s).unwrap_or(InvoiceStatus::Brouillon),
                InvoiceKind::from_db_str(&kind_s).unwrap_or(InvoiceKind::Facture),
                row.get::<_, Option<String>>(5)?,
                issue_date.and_then(|s| parse_date_opt(&s)),
                due_date.and_then(|s| parse_date_opt(&s)),
                row.get::<_, Option<String>>(8)?,
                decimal_from_row(&row.get::<_, String>(9)?),
                decimal_from_row(&row.get::<_, String>(10)?),
                decimal_from_row(&row.get::<_, String>(11)?),
                decimal_from_row(&row.get::<_, String>(12)?),
                row.get::<_, String>(13)?,
                created_at,
                updated_at,
                issued_at,
            ))
        },
    ).map_err(|_| AppError::Introuvable)?;

    let lines = load_lines(conn, invoice_id)?;

    let (
        id,
        number,
        contact_id,
        status,
        invoice_kind,
        related_invoice_id,
        issue_date,
        due_date,
        customer_notes,
        total_ht,
        total_tva,
        total_ttc,
        amount_paid,
        payment_status,
        created_at,
        updated_at,
        issued_at,
    ) = inv;

    Ok(Invoice {
        id,
        number,
        contact_id,
        status,
        invoice_kind,
        related_invoice_id,
        issue_date,
        due_date,
        customer_notes,
        total_ht,
        total_tva,
        total_ttc,
        amount_paid,
        payment_status,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        issued_at: issued_at.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .ok()
                .map(|d| d.with_timezone(&Utc))
        }),
        lines,
    })
}

fn compute_totals(lines: &[InvoiceLineInput]) -> AppResult<(Decimal, Decimal, Decimal, Vec<(InvoiceLineInput, Decimal, Decimal, Decimal)>)> {
    let mut total_ht = dec!(0);
    let mut total_tva = dec!(0);
    let mut total_ttc = dec!(0);
    let mut computed = Vec::new();
    for line in lines {
        if line.description.trim().is_empty() {
            return Err(AppError::validation(
                "Chaque ligne doit avoir une description.",
            ));
        }
        let (ht, tva, ttc) = line_amounts(line.quantity, line.unit_price_ht, line.tax_rate_percent)?;
        total_ht = round_eur(total_ht + ht);
        total_tva = round_eur(total_tva + tva);
        total_ttc = round_eur(total_ttc + ttc);
        computed.push((line.clone(), ht, tva, ttc));
    }
    Ok((total_ht, total_tva, total_ttc, computed))
}

fn insert_lines(
    conn: &Connection,
    invoice_id: &str,
    computed: &[(InvoiceLineInput, Decimal, Decimal, Decimal)],
) -> AppResult<()> {
    for (pos, (line, ht, tva, ttc)) in computed.iter().enumerate() {
        let line_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO invoice_lines (id, invoice_id, position, description, quantity, unit_price_ht, tax_rate_percent, line_ht, line_tva, line_ttc) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                line_id,
                invoice_id,
                pos as i32,
                line.description.trim(),
                format_amount(line.quantity),
                format_amount(line.unit_price_ht),
                format_amount(line.tax_rate_percent),
                format_amount(*ht),
                format_amount(*tva),
                format_amount(*ttc),
            ],
        )?;
    }
    Ok(())
}

impl InvoicesService {
    pub fn list(
        db: &DbPool,
        status: Option<InvoiceStatus>,
        kind: Option<InvoiceKind>,
    ) -> AppResult<Vec<Invoice>> {
        db.with_conn(|conn| {
            let mut out = Vec::new();
            let mut sql = String::from("SELECT id FROM invoices WHERE 1=1");
            let mut binds: Vec<String> = Vec::new();
            if let Some(st) = status {
                sql.push_str(" AND status = ?");
                binds.push(st.as_db_str().to_string());
            }
            match kind {
                Some(InvoiceKind::Devis) => {
                    sql.push_str(" AND invoice_kind = 'devis'");
                }
                Some(InvoiceKind::Facture) => {
                    sql.push_str(" AND invoice_kind IN ('facture', 'avoir')");
                }
                Some(InvoiceKind::Avoir) => {
                    sql.push_str(" AND invoice_kind = 'avoir'");
                }
                None => {}
            }
            sql.push_str(" ORDER BY created_at DESC");
            let mut stmt = conn.prepare(&sql)?;
            let params_refs: Vec<&dyn rusqlite::types::ToSql> = binds
                .iter()
                .map(|s| s as &dyn rusqlite::types::ToSql)
                .collect();
            let ids: Vec<String> = stmt
                .query_map(params_refs.as_slice(), |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for id in ids {
                out.push(map_invoice(conn, &id)?);
            }
            Ok(out)
        })
    }

    pub fn get(db: &DbPool, id: &str) -> AppResult<Invoice> {
        db.with_conn(|conn| map_invoice(conn, id))
    }

    pub fn create_draft(db: &DbPool, user_id: &str, input: InvoiceDraftInput) -> AppResult<Invoice> {
        if input.lines.is_empty() {
            return Err(AppError::validation(
                "Ajoutez au moins une ligne au document.",
            ));
        }
        let kind = match input.invoice_kind.clone().unwrap_or(InvoiceKind::Facture) {
            InvoiceKind::Avoir => {
                return Err(AppError::validation(
                    "Utilisez la création d'avoir dédiée pour un avoir.",
                ));
            }
            other => other,
        };
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let (total_ht, total_tva, total_ttc, computed) = compute_totals(&input.lines)?;
        let issue_s = input
            .issue_date
            .map(|d| d.format("%Y-%m-%d").to_string());
        let due_s = input.due_date.map(|d| d.format("%Y-%m-%d").to_string());

        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO invoices (id, number, contact_id, status, invoice_kind, related_invoice_id, issue_date, due_date, customer_notes, total_ht, total_tva, total_ttc, created_at, updated_at, issued_at) VALUES (?1,NULL,?2,'brouillon',?3,NULL,?4,?5,?6,?7,?8,?9,?10,?11,NULL)",
                params![
                    id,
                    input.contact_id,
                    kind.as_db_str(),
                    issue_s,
                    due_s,
                    input.customer_notes,
                    format_amount(total_ht),
                    format_amount(total_tva),
                    format_amount(total_ttc),
                    now,
                    now
                ],
            )?;
            insert_lines(conn, &id, &computed)?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            if kind == InvoiceKind::Devis {
                "devis_brouillon_cree"
            } else {
                "facture_brouillon_cree"
            },
            Some("invoice"),
            Some(&id),
            None,
        )?;
        Self::get(db, &id)
    }

    pub fn update_draft(
        db: &DbPool,
        user_id: &str,
        id: &str,
        input: InvoiceDraftInput,
    ) -> AppResult<Invoice> {
        if input.lines.is_empty() {
            return Err(AppError::validation(
                "Ajoutez au moins une ligne à la facture.",
            ));
        }
        let (total_ht, total_tva, total_ttc, computed) = compute_totals(&input.lines)?;
        let now = Utc::now().to_rfc3339();
        let issue_s = input
            .issue_date
            .map(|d| d.format("%Y-%m-%d").to_string());
        let due_s = input.due_date.map(|d| d.format("%Y-%m-%d").to_string());

        db.with_conn_mut(|conn| {
            let status: String = conn.query_row(
                "SELECT status FROM invoices WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            if status != InvoiceStatus::Brouillon.as_db_str() {
                return Err(AppError::interdit(
                    "Seules les factures en brouillon peuvent être modifiées.",
                ));
            }
            conn.execute(
                "UPDATE invoices SET contact_id=?1, issue_date=?2, due_date=?3, customer_notes=?4, total_ht=?5, total_tva=?6, total_ttc=?7, updated_at=?8 WHERE id=?9",
                params![
                    input.contact_id,
                    issue_s,
                    due_s,
                    input.customer_notes,
                    format_amount(total_ht),
                    format_amount(total_tva),
                    format_amount(total_ttc),
                    now,
                    id
                ],
            )?;
            conn.execute(
                "DELETE FROM invoice_lines WHERE invoice_id = ?1",
                params![id],
            )?;
            insert_lines(conn, id, &computed)?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "facture_brouillon_modifiee",
            Some("invoice"),
            Some(id),
            None,
        )?;
        Self::get(db, id)
    }

    pub fn delete_draft(db: &DbPool, user_id: &str, id: &str) -> AppResult<()> {
        let n = db.with_conn_mut(|conn| {
            let status: String = conn.query_row(
                "SELECT status FROM invoices WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            if status != InvoiceStatus::Brouillon.as_db_str() {
                return Err(AppError::interdit(
                    "Impossible de supprimer une facture émise ou un avoir.",
                ));
            }
            conn.execute("DELETE FROM invoices WHERE id = ?1", params![id])?;
            Ok(conn.changes())
        })?;
        if n == 0 {
            return Err(AppError::Introuvable);
        }
        AuditService::log(
            db,
            user_id,
            "facture_brouillon_supprimee",
            Some("invoice"),
            Some(id),
            None,
        )?;
        Ok(())
    }

    pub fn issue(db: &DbPool, user_id: &str, id: &str) -> AppResult<Invoice> {
        db.with_conn_mut(|conn| {
            let (status, kind): (String, String) = conn.query_row(
                "SELECT status, invoice_kind FROM invoices WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if status != InvoiceStatus::Brouillon.as_db_str() {
                return Err(AppError::interdit(
                    "Cette facture a déjà été émise ou n'est plus un brouillon.",
                ));
            }
            let number = SettingsService::next_document_number(conn, &kind)?;
            let now = Utc::now().to_rfc3339();
            let new_status = if kind == InvoiceKind::Avoir.as_db_str() {
                InvoiceStatus::Avoir.as_db_str()
            } else {
                InvoiceStatus::Emise.as_db_str()
            };
            conn.execute(
                "UPDATE invoices SET status = ?1, number = ?2, issued_at = ?3, updated_at = ?3 WHERE id = ?4",
                params![new_status, number, now, id],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "document_emis",
            Some("invoice"),
            Some(id),
            None,
        )?;
        Self::get(db, id)
    }

    pub fn create_credit_note_draft(
        db: &DbPool,
        user_id: &str,
        related_invoice_id: &str,
        lines: Vec<InvoiceLineInput>,
        customer_notes: Option<String>,
    ) -> AppResult<Invoice> {
        db.with_conn_mut(|conn| {
            let (status, contact_id): (String, String) = conn.query_row(
                "SELECT status, contact_id FROM invoices WHERE id = ?1",
                params![related_invoice_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if status != InvoiceStatus::Emise.as_db_str() {
                return Err(AppError::interdit(
                    "Un avoir ne peut être créé qu'à partir d'une facture émise.",
                ));
            }
            Ok(contact_id)
        })?;

        let negated: Vec<InvoiceLineInput> = lines
            .into_iter()
            .map(|mut l| {
                l.quantity = round_eur(-l.quantity.abs());
                l
            })
            .collect();

        if negated.is_empty() {
            return Err(AppError::validation(
                "Indiquez les lignes à créditer pour l'avoir.",
            ));
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let (total_ht, total_tva, total_ttc, computed) = compute_totals(&negated)?;

        db.with_conn_mut(|conn| {
            let contact_id: String = conn.query_row(
                "SELECT contact_id FROM invoices WHERE id = ?1",
                params![related_invoice_id],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO invoices (id, number, contact_id, status, invoice_kind, related_invoice_id, issue_date, due_date, customer_notes, total_ht, total_tva, total_ttc, created_at, updated_at, issued_at) VALUES (?1,NULL,?2,'brouillon','avoir',?3,NULL,NULL,?4,?5,?6,?7,?8,?9,NULL)",
                params![
                    id,
                    contact_id,
                    related_invoice_id,
                    customer_notes,
                    format_amount(total_ht),
                    format_amount(total_tva),
                    format_amount(total_ttc),
                    now,
                    now
                ],
            )?;
            insert_lines(conn, &id, &computed)?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "avoir_brouillon_cree",
            Some("invoice"),
            Some(&id),
            Some(related_invoice_id),
        )?;
        Self::get(db, &id)
    }

    pub fn record_payment(
        db: &DbPool,
        user_id: &str,
        input: crate::models::invoice::RecordPaymentInput,
    ) -> AppResult<Invoice> {
        if input.amount <= dec!(0) {
            return Err(AppError::validation(
                "Le montant du règlement doit être positif.",
            ));
        }
        let invoice = Self::get(db, &input.invoice_id)?;
        if invoice.invoice_kind == InvoiceKind::Devis {
            return Err(AppError::interdit(
                "Un devis ne peut pas recevoir de règlement. Convertissez-le en facture.",
            ));
        }
        if invoice.status != InvoiceStatus::Emise {
            return Err(AppError::interdit(
                "Seules les factures émises peuvent recevoir un règlement.",
            ));
        }
        let new_paid = round_eur(invoice.amount_paid + input.amount);
        if new_paid > invoice.total_ttc + dec!(0.01) {
            return Err(AppError::validation(
                "Le cumul des règlements dépasse le total TTC.",
            ));
        }
        let payment_status = if new_paid + dec!(0.005) >= invoice.total_ttc {
            "payee"
        } else if new_paid > dec!(0) {
            "partielle"
        } else {
            "impayee"
        };
        let payment_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let paid_at = input.paid_at.format("%Y-%m-%d").to_string();

        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO invoice_payments (id, invoice_id, amount, paid_at, method, reference, notes, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    payment_id,
                    input.invoice_id,
                    format_amount(input.amount),
                    paid_at,
                    input.method,
                    input.reference,
                    input.notes,
                    now
                ],
            )?;
            conn.execute(
                "UPDATE invoices SET amount_paid = ?1, payment_status = ?2, updated_at = ?3 WHERE id = ?4",
                params![
                    format_amount(new_paid),
                    payment_status,
                    now,
                    input.invoice_id
                ],
            )?;
            Ok(())
        })?;

        AuditService::log(
            db,
            user_id,
            "reglement_enregistre",
            Some("invoice"),
            Some(&input.invoice_id),
            Some(&format_amount(input.amount)),
        )?;
        Self::get(db, &input.invoice_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::run_migrations;
    use crate::models::contact::{ContactInput, ContactKind};
    use crate::services::contacts::ContactsService;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_pool() -> DbPool {
        let pool = DbPool::open_in_memory().unwrap();
        pool.with_conn_mut(|conn| run_migrations(conn)).unwrap();
        let now = Utc::now().to_rfc3339();
        let user_id = Uuid::new_v4().to_string();
        pool.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO users (id, username, password_hash, created_at, updated_at) VALUES (?1,'test','hash',?2,?2)",
                params![user_id, now],
            )?;
            Ok(())
        })
        .unwrap();
        pool
    }

    #[test]
    fn numbering_increments() {
        let pool = test_pool();
        let n1 = pool
            .with_conn_mut(|conn| SettingsService::next_invoice_number(conn, false))
            .unwrap();
        let n2 = pool
            .with_conn_mut(|conn| SettingsService::next_invoice_number(conn, false))
            .unwrap();
        assert_ne!(n1, n2);
        let year = Utc::now().format("%Y");
        assert!(n1.contains(&year.to_string()));
        assert!(n2.contains(&year.to_string()));
    }

    #[test]
    fn issued_invoice_not_editable() {
        let pool = test_pool();
        let user = "uid";
        let contact = ContactsService::create(
            &pool,
            user,
            ContactInput {
                kind: ContactKind::Client,
                name: "Client Test".into(),
                email: None,
                phone: None,
                address_line1: None,
                address_line2: None,
                postal_code: None,
                city: None,
                country: None,
                siret: None,
                vat_number: None,
                notes: None,
            },
        )
        .unwrap();
        let draft = InvoicesService::create_draft(
            &pool,
            user,
            InvoiceDraftInput {
                contact_id: contact.id.clone(),
                issue_date: None,
                due_date: None,
                customer_notes: None,
                lines: vec![InvoiceLineInput {
                    description: "Prestation".into(),
                    quantity: dec!(1),
                    unit_price_ht: dec!(100),
                    tax_rate_percent: dec!(20),
                }],
            },
        )
        .unwrap();
        InvoicesService::issue(&pool, user, &draft.id).unwrap();
        let err = InvoicesService::update_draft(
            &pool,
            user,
            &draft.id,
            InvoiceDraftInput {
                contact_id: contact.id,
                issue_date: None,
                due_date: None,
                customer_notes: None,
                lines: vec![InvoiceLineInput {
                    description: "Autre".into(),
                    quantity: dec!(1),
                    unit_price_ht: dec!(50),
                    tax_rate_percent: dec!(20),
                }],
            },
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Interdit(_)));
    }

    #[test]
    fn credit_note_requires_emise() {
        let pool = test_pool();
        let user = "uid";
        let contact = ContactsService::create(
            &pool,
            user,
            ContactInput {
                kind: ContactKind::Client,
                name: "Client".into(),
                email: None,
                phone: None,
                address_line1: None,
                address_line2: None,
                postal_code: None,
                city: None,
                country: None,
                siret: None,
                vat_number: None,
                notes: None,
            },
        )
        .unwrap();
        let draft = InvoicesService::create_draft(
            &pool,
            user,
            InvoiceDraftInput {
                contact_id: contact.id.clone(),
                issue_date: None,
                due_date: None,
                customer_notes: None,
                lines: vec![InvoiceLineInput {
                    description: "Ligne".into(),
                    quantity: dec!(1),
                    unit_price_ht: dec!(10),
                    tax_rate_percent: dec!(20),
                }],
            },
        )
        .unwrap();
        let err = InvoicesService::create_credit_note_draft(
            &pool,
            user,
            &draft.id,
            vec![InvoiceLineInput {
                description: "Retour".into(),
                quantity: dec!(1),
                unit_price_ht: dec!(10),
                tax_rate_percent: dec!(20),
            }],
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::Interdit(_)));
    }
}