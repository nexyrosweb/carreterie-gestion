//! Miroir transactionnel MySQL ↔ SQLite mémoire pour réutiliser le code métier rusqlite.

use std::collections::HashSet;

use mysql::prelude::{Queryable, ToValue};
use mysql::{Pool as MysqlPool, Value as MysqlValue};
use rusqlite::{types::ValueRef, Connection};

use crate::db::migrations::run_migrations;
use crate::db::sql::to_mysql_placeholders;
use crate::error::{AppError, AppResult};

const MIRROR_TABLES: &[&str] = &[
    "schema_version",
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

pub fn with_mysql_mirror_read<R>(
    pool: &MysqlPool,
    f: impl FnOnce(&Connection) -> AppResult<R>,
) -> AppResult<R> {
    let mut mem = open_mirror_db()?;
    pull_all(pool, &mut mem)?;
    f(&mem)
}

pub fn with_mysql_mirror_write<R>(
    pool: &MysqlPool,
    f: impl FnOnce(&mut Connection) -> AppResult<R>,
) -> AppResult<R> {
    let mut mem = open_mirror_db()?;
    pull_all(pool, &mut mem)?;
    let before = snapshot_ids(&mem)?;
    let result = f(&mut mem)?;
    let after = snapshot_ids(&mem)?;
    push_all(pool, &mem, &before, &after)?;
    Ok(result)
}

fn open_mirror_db() -> AppResult<Connection> {
    let conn = Connection::open_in_memory().map_err(AppError::BaseDeDonnees)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(AppError::BaseDeDonnees)?;
    run_migrations(&conn)?;
    // file_blobs reste côté MySQL uniquement
    Ok(conn)
}

fn snapshot_ids(conn: &Connection) -> AppResult<Vec<(String, HashSet<String>)>> {
    let mut out = Vec::new();
    for table in MIRROR_TABLES {
        if table == &"schema_version" || table == &"login_attempts" || table == &"audit_log" {
            continue;
        }
        let pk = primary_key_column(table);
        let mut set = HashSet::new();
        let mut stmt = conn
            .prepare(&format!("SELECT \"{pk}\" FROM {table}"))
            .map_err(AppError::BaseDeDonnees)?;
        let rows = stmt
            .query_map([], |r| {
                let v: rusqlite::types::Value = r.get(0)?;
                Ok(match v {
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Integer(n) => n.to_string(),
                    other => format!("{other:?}"),
                })
            })
            .map_err(AppError::BaseDeDonnees)?;
        for id in rows {
            set.insert(id.map_err(AppError::BaseDeDonnees)?);
        }
        out.push(((*table).to_string(), set));
    }
    Ok(out)
}

fn primary_key_column(table: &str) -> &'static str {
    match table {
        "sessions_meta" | "settings" => "key",
        "login_attempts" | "audit_log" | "schema_version" => "id",
        _ => "id",
    }
}

fn pull_all(pool: &MysqlPool, mem: &mut Connection) -> AppResult<()> {
    let mut mysql = pool
        .get_conn()
        .map_err(|e| AppError::interne(format!("MySQL mirror : {e}")))?;

    for table in MIRROR_TABLES {
        mem.execute(&format!("DELETE FROM {table}"), [])
            .map_err(AppError::BaseDeDonnees)?;

        let sql = format!("SELECT * FROM `{table}`");
        let result = mysql
            .query_iter(sql)
            .map_err(|e| AppError::interne(format!("MySQL pull {table}: {e}")))?;

        let mut column_names: Option<Vec<String>> = None;
        for row_res in result {
            let row: mysql::Row =
                row_res.map_err(|e| AppError::interne(format!("MySQL pull row : {e}")))?;
            if column_names.is_none() {
                column_names = Some(
                    row.columns_ref()
                        .iter()
                        .map(|c| c.name_str().to_string())
                        .collect(),
                );
            }
            let cols = column_names.as_ref().unwrap();
            let placeholders = vec!["?"; cols.len()].join(",");
            let col_list = cols
                .iter()
                .map(|c| format!("\"{c}\""))
                .collect::<Vec<_>>()
                .join(",");
            let insert = format!("INSERT INTO {table} ({col_list}) VALUES ({placeholders})");
            let mut vals: Vec<rusqlite::types::Value> = Vec::with_capacity(cols.len());
            for i in 0..cols.len() {
                let v: MysqlValue = row.get(i).unwrap_or(MysqlValue::NULL);
                vals.push(mysql_to_sqlite(v));
            }
            let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|v| v as _).collect();
            mem.execute(&insert, refs.as_slice())
                .map_err(AppError::BaseDeDonnees)?;
        }
    }
    Ok(())
}

fn push_all(
    pool: &MysqlPool,
    mem: &Connection,
    before: &[(String, HashSet<String>)],
    after: &[(String, HashSet<String>)],
) -> AppResult<()> {
    let mut mysql = pool
        .get_conn()
        .map_err(|e| AppError::interne(format!("MySQL mirror : {e}")))?;

    // Suppress FK during bulk
    let _ = mysql.query_drop("SET FOREIGN_KEY_CHECKS=0");

    for table in MIRROR_TABLES {
        if *table == "schema_version" {
            continue;
        }

        // Deletes
        if let (Some((_, b)), Some((_, a))) = (
            before.iter().find(|(t, _)| t == table),
            after.iter().find(|(t, _)| t == table),
        ) {
            let pk = primary_key_column(table);
            for id in b.difference(a) {
                let del = to_mysql_placeholders(&format!("DELETE FROM `{table}` WHERE `{pk}` = ?"));
                mysql
                    .exec_drop(del, (id,))
                    .map_err(|e| AppError::interne(format!("MySQL delete {table}: {e}")))?;
            }
        }

        // Upserts from mirror
        let mut stmt = match mem.prepare(&format!("SELECT * FROM {table}")) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let col_names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
        if col_names.is_empty() {
            continue;
        }
        let col_count = col_names.len();
        let cols_sql = col_names
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(",");
        let placeholders = vec!["?"; col_count].join(",");
        let updates = col_names
            .iter()
            .filter(|c| c.as_str() != primary_key_column(table))
            .map(|c| format!("`{c}`=VALUES(`{c}`)"))
            .collect::<Vec<_>>()
            .join(",");
        let upsert = if updates.is_empty() {
            format!("INSERT IGNORE INTO `{table}` ({cols_sql}) VALUES ({placeholders})")
        } else {
            format!(
                "INSERT INTO `{table}` ({cols_sql}) VALUES ({placeholders}) ON DUPLICATE KEY UPDATE {updates}"
            )
        };

        let mut rows = stmt.query([]).map_err(AppError::BaseDeDonnees)?;
        while let Some(row) = rows.next().map_err(AppError::BaseDeDonnees)? {
            let mut vals: Vec<MysqlValue> = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let v = row.get_ref(i).map_err(AppError::BaseDeDonnees)?;
                vals.push(sqlite_ref_to_mysql(v));
            }
            mysql
                .exec_drop(upsert.as_str(), vals)
                .map_err(|e| AppError::interne(format!("MySQL upsert {table}: {e}")))?;
        }
    }

    let _ = mysql.query_drop("SET FOREIGN_KEY_CHECKS=1");
    Ok(())
}

fn mysql_to_sqlite(v: MysqlValue) -> rusqlite::types::Value {
    match v {
        MysqlValue::NULL => rusqlite::types::Value::Null,
        MysqlValue::Bytes(b) => match String::from_utf8(b.clone()) {
            Ok(s) => rusqlite::types::Value::Text(s),
            Err(_) => rusqlite::types::Value::Blob(b),
        },
        MysqlValue::Int(n) => rusqlite::types::Value::Integer(n),
        MysqlValue::UInt(n) => rusqlite::types::Value::Integer(n as i64),
        MysqlValue::Float(f) => rusqlite::types::Value::Real(f as f64),
        MysqlValue::Double(f) => rusqlite::types::Value::Real(f),
        other => rusqlite::types::Value::Text(format!("{other:?}")),
    }
}

fn sqlite_ref_to_mysql(v: ValueRef<'_>) -> MysqlValue {
    match v {
        ValueRef::Null => MysqlValue::NULL,
        ValueRef::Integer(n) => n.to_value(),
        ValueRef::Real(f) => f.to_value(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().to_value(),
        ValueRef::Blob(b) => b.to_value(),
    }
}
