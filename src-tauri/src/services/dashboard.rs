use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;

use crate::db::DbPool;
use crate::error::AppResult;
use crate::money::round_eur;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardStats {
    pub contacts_count: i64,
    pub drafts_count: i64,
    pub issued_invoices_count: i64,
    pub credit_notes_count: i64,
    pub supplier_invoices_count: i64,
    pub unpaid_invoices_count: i64,
    pub total_issued_ttc: Decimal,
    pub total_supplier_ttc: Decimal,
    pub total_unpaid_ttc: Decimal,
}

pub struct DashboardService;

impl DashboardService {
    pub fn stats(db: &DbPool) -> AppResult<DashboardStats> {
        db.with_conn(|conn| {
            let contacts_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM contacts", [], |r| r.get(0))?;
            let drafts_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'brouillon'",
                [],
                |r| r.get(0),
            )?;
            let issued_invoices_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'emise'",
                [],
                |r| r.get(0),
            )?;
            let credit_notes_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'avoir' OR invoice_kind = 'avoir'",
                [],
                |r| r.get(0),
            )?;
            let supplier_invoices_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM supplier_invoices",
                [],
                |r| r.get(0),
            )?;
            let unpaid_invoices_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'emise' AND COALESCE(payment_status, 'impayee') != 'payee'",
                [],
                |r| r.get(0),
            ).unwrap_or(0);

            let total_issued_ttc = sum_decimal_column(
                conn,
                "SELECT total_ttc FROM invoices WHERE status IN ('emise', 'avoir')",
            )?;
            let total_supplier_ttc = sum_decimal_column(
                conn,
                "SELECT amount_ttc FROM supplier_invoices WHERE amount_ttc IS NOT NULL",
            )?;
            let total_unpaid_ttc = sum_unpaid(conn)?;

            Ok(DashboardStats {
                contacts_count,
                drafts_count,
                issued_invoices_count,
                credit_notes_count,
                supplier_invoices_count,
                unpaid_invoices_count,
                total_issued_ttc,
                total_supplier_ttc,
                total_unpaid_ttc,
            })
        })
    }
}

fn sum_unpaid(conn: &rusqlite::Connection) -> AppResult<Decimal> {
    let mut stmt = conn.prepare(
        "SELECT total_ttc, COALESCE(amount_paid, '0.00') FROM invoices WHERE status = 'emise' AND COALESCE(payment_status, 'impayee') != 'payee'",
    )?;
    let mut rows = stmt.query([])?;
    let mut total = dec!(0);
    while let Some(row) = rows.next()? {
        let ttc = decimal_or_zero(&row.get::<_, String>(0)?);
        let paid = decimal_or_zero(&row.get::<_, String>(1)?);
        total = round_eur(total + (ttc - paid));
    }
    Ok(total)
}

fn decimal_or_zero(s: &str) -> Decimal {
    s.parse::<Decimal>().map(round_eur).unwrap_or(dec!(0))
}

fn sum_decimal_column(conn: &rusqlite::Connection, sql: &str) -> AppResult<Decimal> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query([])?;
    let mut total = dec!(0);
    while let Some(row) = rows.next()? {
        let s: String = row.get(0)?;
        if let Ok(d) = s.parse::<Decimal>() {
            total = round_eur(total + d);
        }
    }
    Ok(total)
}