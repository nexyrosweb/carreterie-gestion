use chrono::Utc;
use rusqlite::params;

use crate::db::DbPool;
use crate::error::AppResult;
use crate::models::audit::AuditEntry;

pub struct AuditService;

impl AuditService {
    pub fn log(
        db: &DbPool,
        user_id: &str,
        action: &str,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        details: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO audit_log (created_at, user_id, action, entity_type, entity_id, details) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![now, user_id, action, entity_type, entity_id, details],
            )?;
            Ok(())
        })
    }

    pub fn log_system(
        db: &DbPool,
        action: &str,
        entity_type: Option<&str>,
        entity_id: Option<&str>,
        details: Option<&str>,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        db.with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO audit_log (created_at, user_id, action, entity_type, entity_id, details) VALUES (?1, NULL, ?2, ?3, ?4, ?5)",
                params![now, action, entity_type, entity_id, details],
            )?;
            Ok(())
        })
    }

    pub fn list_recent(db: &DbPool, limit: i64) -> AppResult<Vec<AuditEntry>> {
        db.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, user_id, action, entity_type, entity_id, details FROM audit_log ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                let created_at: String = row.get(1)?;
                Ok(AuditEntry {
                    id: row.get(0)?,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    user_id: row.get(2)?,
                    action: row.get(3)?,
                    entity_type: row.get(4)?,
                    entity_id: row.get(5)?,
                    details: row.get(6)?,
                })
            })?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            Ok(out)
        })
    }
}