use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::Serialize;

use crate::db::DbPool;
use crate::error::AppResult;
use crate::money::round_eur;

#[derive(Debug, Clone, Serialize)]
pub struct MonthlyActivityRow {
    pub month: String,
    pub revenue_ht: Decimal,
    pub supplier_ttc: Decimal,
}

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
    pub total_issued_ht: Decimal,
    pub total_unpaid_ht: Decimal,
    pub pending_quotes_count: i64,
    pub quotes_issued_count: i64,
    pub monthly_activity: Vec<MonthlyActivityRow>,
}

pub struct DashboardService;

impl DashboardService {
    pub fn stats(db: &DbPool) -> AppResult<DashboardStats> {
        db.with_conn(|conn| {
            let contacts_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM contacts", [], |r| r.get(0))?;
            let drafts_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'brouillon' AND invoice_kind = 'facture'",
                [],
                |r| r.get(0),
            )?;
            let issued_invoices_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE status = 'emise' AND invoice_kind = 'facture'",
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
                "SELECT COUNT(*) FROM invoices WHERE status = 'emise' AND invoice_kind = 'facture' AND COALESCE(payment_status, 'impayee') != 'payee'",
                [],
                |r| r.get(0),
            ).unwrap_or(0);

            let quotes_issued_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE invoice_kind = 'devis' AND status = 'emise'",
                [],
                |r| r.get(0),
            )?;
            let pending_quotes_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices d
                 WHERE d.invoice_kind = 'devis'
                   AND (
                     d.status = 'brouillon'
                     OR (
                       d.status = 'emise'
                       AND NOT EXISTS (
                         SELECT 1 FROM invoices f
                         WHERE f.related_invoice_id = d.id AND f.invoice_kind = 'facture'
                       )
                     )
                   )",
                [],
                |r| r.get(0),
            )?;

            let total_issued_ttc = sum_decimal_column(
                conn,
                "SELECT total_ttc FROM invoices WHERE status = 'emise' AND invoice_kind = 'facture'",
            )?;
            let total_issued_ht = sum_decimal_column(
                conn,
                "SELECT total_ht FROM invoices WHERE status = 'emise' AND invoice_kind = 'facture'",
            )?;
            let total_supplier_ttc = sum_decimal_column(
                conn,
                "SELECT amount_ttc FROM supplier_invoices WHERE amount_ttc IS NOT NULL",
            )?;
            let (total_unpaid_ttc, total_unpaid_ht) = sum_unpaid(conn)?;
            let monthly_activity = monthly_rows(conn)?;

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
                total_issued_ht,
                total_unpaid_ht,
                pending_quotes_count,
                quotes_issued_count,
                monthly_activity,
            })
        })
    }
}

fn sum_unpaid(conn: &rusqlite::Connection) -> AppResult<(Decimal, Decimal)> {
    let mut stmt = conn.prepare(
        "SELECT total_ttc, total_ht, COALESCE(amount_paid, '0.00') FROM invoices WHERE status = 'emise' AND invoice_kind = 'facture' AND COALESCE(payment_status, 'impayee') != 'payee'",
    )?;
    let mut rows = stmt.query([])?;
    let mut total_ttc = dec!(0);
    let mut total_ht = dec!(0);
    while let Some(row) = rows.next()? {
        let ttc = decimal_or_zero(&row.get::<_, String>(0)?);
        let ht = decimal_or_zero(&row.get::<_, String>(1)?);
        let paid = decimal_or_zero(&row.get::<_, String>(2)?);
        let ratio = if ttc > dec!(0) { (ttc - paid) / ttc } else { dec!(0) };
        total_ttc = round_eur(total_ttc + (ttc - paid));
        total_ht = round_eur(total_ht + round_eur(ht * ratio));
    }
    Ok((total_ttc, total_ht))
}

fn monthly_rows(conn: &rusqlite::Connection) -> AppResult<Vec<MonthlyActivityRow>> {
    let mut map: std::collections::BTreeMap<String, (Decimal, Decimal)> =
        std::collections::BTreeMap::new();

    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', issue_date), total_ht FROM invoices
         WHERE status = 'emise' AND invoice_kind = 'facture' AND issue_date IS NOT NULL",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let month: String = row.get(0)?;
        let ht = decimal_or_zero(&row.get::<_, String>(1)?);
        let entry = map.entry(month).or_insert((dec!(0), dec!(0)));
        entry.0 = round_eur(entry.0 + ht);
    }

    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m', invoice_date), amount_ttc FROM supplier_invoices
         WHERE invoice_date IS NOT NULL AND amount_ttc IS NOT NULL",
    )?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let month: String = row.get(0)?;
        let ttc = decimal_or_zero(&row.get::<_, String>(1)?);
        let entry = map.entry(month).or_insert((dec!(0), dec!(0)));
        entry.1 = round_eur(entry.1 + ttc);
    }

    Ok(map
        .into_iter()
        .map(|(month, (revenue_ht, supplier_ttc))| MonthlyActivityRow {
            month,
            revenue_ht,
            supplier_ttc,
        })
        .collect())
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
