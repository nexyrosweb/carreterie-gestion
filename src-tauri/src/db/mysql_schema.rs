use crate::db::{DbPool, SqlVal};
use crate::error::AppResult;

const SCHEMA_SQL: &str = include_str!("../../../deploy/o2switch/schema.sql");

/// Applique le schéma o2switch si la base est vide / version absente.
pub fn ensure_mysql_schema(db: &DbPool) -> AppResult<()> {
    if !db.is_remote() {
        return Ok(());
    }
    let existing = db.query(
        "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema = DATABASE() AND table_name = 'schema_version'",
        &[],
    )?;
    let has = existing
        .first()
        .and_then(|r| r.first())
        .map(|c| c.as_i64().unwrap_or(0) > 0)
        .unwrap_or(false);
    if !has {
        db.execute_batch_script(SCHEMA_SQL)?;
        return Ok(());
    }
    let ver = db.query("SELECT COALESCE(MAX(version), 0) FROM schema_version", &[])?;
    let version = ver
        .first()
        .and_then(|r| r.first())
        .map(|c| c.as_i64().unwrap_or(0))
        .unwrap_or(0);
    if version < 7 {
        // Ré-appliquer inserts de settings / file_blobs si besoin
        db.execute_batch_script(
            r#"
CREATE TABLE IF NOT EXISTS file_blobs (
    path VARCHAR(512) NOT NULL PRIMARY KEY,
    content LONGBLOB NOT NULL,
    size_bytes BIGINT NOT NULL,
    content_type VARCHAR(128) NOT NULL DEFAULT 'application/octet-stream',
    updated_at TEXT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
INSERT IGNORE INTO schema_version (version) VALUES (7);
"#,
        )?;
    }
    let _ = SqlVal::Int(version);
    Ok(())
}
