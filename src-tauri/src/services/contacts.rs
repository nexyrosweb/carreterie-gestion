use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::contact::{Contact, ContactInput, ContactKind};
use crate::services::audit::AuditService;

pub struct ContactsService;

fn map_contact(row: &rusqlite::Row<'_>) -> rusqlite::Result<Contact> {
    let kind_s: String = row.get(2)?;
    let kind = ContactKind::from_db_str(&kind_s).unwrap_or(ContactKind::Client);
    let created_at: String = row.get(13)?;
    let updated_at: String = row.get(14)?;
    Ok(Contact {
        id: row.get(0)?,
        kind,
        name: row.get(1)?,
        email: row.get(3)?,
        phone: row.get(4)?,
        address_line1: row.get(5)?,
        address_line2: row.get(6)?,
        postal_code: row.get(7)?,
        city: row.get(8)?,
        country: row.get(9)?,
        siret: row.get(10)?,
        vat_number: row.get(11)?,
        notes: row.get(12)?,
        created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}

impl ContactsService {
    pub fn list(db: &DbPool, search: Option<&str>) -> AppResult<Vec<Contact>> {
        db.with_conn(|conn| {
            let mut out = Vec::new();
            if let Some(q) = search.filter(|s| !s.trim().is_empty()) {
                let pattern = format!("%{}%", q.trim());
                let mut stmt = conn.prepare(
                    "SELECT id, name, kind, email, phone, address_line1, address_line2, postal_code, city, country, siret, vat_number, notes, created_at, updated_at FROM contacts WHERE name LIKE ?1 OR email LIKE ?1 ORDER BY name",
                )?;
                let rows = stmt.query_map(params![pattern], map_contact)?;
                for r in rows {
                    out.push(r?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, name, kind, email, phone, address_line1, address_line2, postal_code, city, country, siret, vat_number, notes, created_at, updated_at FROM contacts ORDER BY name",
                )?;
                let rows = stmt.query_map([], map_contact)?;
                for r in rows {
                    out.push(r?);
                }
            }
            Ok(out)
        })
    }

    pub fn get(db: &DbPool, id: &str) -> AppResult<Contact> {
        db.with_conn(|conn| {
            conn.query_row(
                "SELECT id, name, kind, email, phone, address_line1, address_line2, postal_code, city, country, siret, vat_number, notes, created_at, updated_at FROM contacts WHERE id = ?1",
                params![id],
                map_contact,
            )
            .map_err(|_| AppError::Introuvable)
        })
    }

    pub fn create(db: &DbPool, user_id: &str, input: ContactInput) -> AppResult<Contact> {
        if input.name.trim().is_empty() {
            return Err(AppError::validation("Le nom du contact est obligatoire."));
        }
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let country = input.country.unwrap_or_else(|| "France".to_string());
        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO contacts (id, kind, name, email, phone, address_line1, address_line2, postal_code, city, country, siret, vat_number, notes, created_at, updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    id,
                    input.kind.as_db_str(),
                    input.name.trim(),
                    input.email,
                    input.phone,
                    input.address_line1,
                    input.address_line2,
                    input.postal_code,
                    input.city,
                    country,
                    input.siret,
                    input.vat_number,
                    input.notes,
                    now,
                    now
                ],
            )?;
            Ok(())
        })?;
        AuditService::log(
            db,
            user_id,
            "contact_cree",
            Some("contact"),
            Some(&id),
            Some(&input.name),
        )?;
        Self::get(db, &id)
    }

    pub fn update(
        db: &DbPool,
        user_id: &str,
        id: &str,
        input: ContactInput,
    ) -> AppResult<Contact> {
        if input.name.trim().is_empty() {
            return Err(AppError::validation("Le nom du contact est obligatoire."));
        }
        let now = Utc::now().to_rfc3339();
        let country = input.country.unwrap_or_else(|| "France".to_string());
        let updated = db.with_conn_mut(|conn| {
            conn.execute(
                "UPDATE contacts SET kind=?1, name=?2, email=?3, phone=?4, address_line1=?5, address_line2=?6, postal_code=?7, city=?8, country=?9, siret=?10, vat_number=?11, notes=?12, updated_at=?13 WHERE id=?14",
                params![
                    input.kind.as_db_str(),
                    input.name.trim(),
                    input.email,
                    input.phone,
                    input.address_line1,
                    input.address_line2,
                    input.postal_code,
                    input.city,
                    country,
                    input.siret,
                    input.vat_number,
                    input.notes,
                    now,
                    id
                ],
            )?;
            Ok(conn.changes())
        })?;
        if updated == 0 {
            return Err(AppError::Introuvable);
        }
        AuditService::log(
            db,
            user_id,
            "contact_modifie",
            Some("contact"),
            Some(id),
            None,
        )?;
        Self::get(db, id)
    }

    pub fn delete(db: &DbPool, user_id: &str, id: &str) -> AppResult<()> {
        let used: i64 = db.with_conn(|conn| {
            let c: i64 = conn.query_row(
                "SELECT COUNT(*) FROM invoices WHERE contact_id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            Ok(c)
        })?;
        if used > 0 {
            return Err(AppError::interdit(
                "Impossible de supprimer un contact lié à des factures.",
            ));
        }
        let n = db.with_conn_mut(|conn| {
            conn.execute("DELETE FROM contacts WHERE id = ?1", params![id])?;
            Ok(conn.changes())
        })?;
        if n == 0 {
            return Err(AppError::Introuvable);
        }
        AuditService::log(
            db,
            user_id,
            "contact_supprime",
            Some("contact"),
            Some(id),
            None,
        )?;
        Ok(())
    }
}