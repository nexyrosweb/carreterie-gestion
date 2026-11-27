//! Import d'un fichier carreterie.db local vers MySQL o2switch.

use rusqlite::Connection;

use crate::db::mysql_schema::ensure_mysql_schema;
use crate::db::{DbPool, SqlVal};
use crate::error::{AppError, AppResult};
use crate::paths::production_database_path;
use crate::remote_config::RemoteDatabaseConfig;

const TABLES: &[&str] = &[
    "users",
    "sessions_meta",
    "login_attempts",
    "contacts",
    "invoices",
    "invoice_lines",
    "invoice_payments",
    "supplier_invoices",
    "settings",
    "audit_log",
    "imported_documents",
];

#[derive(serde::Serialize)]
pub struct MigrateResult {
    pub tables: Vec<(String, u64)>,
    pub files_uploaded: usize,
}

pub fn migrate_local_sqlite_to_remote() -> AppResult<MigrateResult> {
    let cfg = RemoteDatabaseConfig::load()?;
    if !cfg.enabled {
        return Err(AppError::validation(
            "Active d'abord la base partagée o2switch avant de migrer.",
        ));
    }
    let sqlite_path = production_database_path()?;
    if !sqlite_path.exists() {
        return Err(AppError::validation(
            "Aucune base SQLite locale à migrer (carreterie.db introuvable).",
        ));
    }

    let remote = DbPool::open_mysql(&cfg)?;
    ensure_mysql_schema(&remote)?;

    let src = Connection::open(&sqlite_path).map_err(AppError::BaseDeDonnees)?;
    let mut tables = Vec::new();

    for table in TABLES {
        let count = copy_table(&src, &remote, table)?;
        tables.push(((*table).to_string(), count));
    }

    // schema_version
    let _ = remote.execute("INSERT IGNORE INTO schema_version (version) VALUES (7)", &[]);

    let files_uploaded = crate::db::blobs::BlobStore::sync_attachments_dir(&remote).unwrap_or(0);

    Ok(MigrateResult {
        tables,
        files_uploaded,
    })
}

fn copy_table(src: &Connection, dest: &DbPool, table: &str) -> AppResult<u64> {
    let exists: i64 = src
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return Ok(0);
    }

    let mut stmt = src
        .prepare(&format!("SELECT * FROM {table}"))
        .map_err(AppError::BaseDeDonnees)?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    if col_names.is_empty() {
        return Ok(0);
    }

    // Colonnes présentes côté MySQL uniquement
    let placeholders = vec!["?"; col_count].join(", ");
    let cols_sql = col_names
        .iter()
        .map(|c| format!("`{c}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let insert_sql = format!("INSERT IGNORE INTO `{table}` ({cols_sql}) VALUES ({placeholders})");

    let mut rows = stmt.query([]).map_err(AppError::BaseDeDonnees)?;
    let mut n = 0u64;
    while let Some(row) = rows.next().map_err(AppError::BaseDeDonnees)? {
        let mut vals = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let v = row.get_ref(i).map_err(AppError::BaseDeDonnees)?;
            vals.push(match v {
                rusqlite::types::ValueRef::Null => SqlVal::Null,
                rusqlite::types::ValueRef::Integer(x) => SqlVal::Int(x),
                rusqlite::types::ValueRef::Real(f) => SqlVal::Text(f.to_string()),
                rusqlite::types::ValueRef::Text(t) => {
                    SqlVal::Text(String::from_utf8_lossy(t).into_owned())
                }
                rusqlite::types::ValueRef::Blob(b) => SqlVal::Blob(b.to_vec()),
            });
        }
        dest.execute(&insert_sql, &vals)?;
        n += 1;
    }
    Ok(n)
}
