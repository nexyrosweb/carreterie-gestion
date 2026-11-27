use std::path::{Path, PathBuf};
use std::sync::Arc;

use mysql::prelude::{Queryable, ToValue};
use mysql::{Opts, OptsBuilder, Pool as MysqlPool, SslOpts, Value as MysqlValue};
use parking_lot::Mutex;
use rusqlite::{types::ValueRef, Connection, ToSql};

use crate::db::sql::{strip_sqlite_pragmas, to_mysql_placeholders, SqlVal};
use crate::error::{AppError, AppResult};
use crate::paths::{demo_database_path, production_database_path};
use crate::remote_config::RemoteDatabaseConfig;

#[derive(Clone)]
pub struct DbPool {
    backend: Arc<DbBackend>,
}

enum DbBackend {
    Sqlite {
        inner: Mutex<Connection>,
        path: PathBuf,
    },
    Mysql {
        pool: MysqlPool,
        label: String,
        /// Sérialise les miroirs pull/push (évite courses intra-processus).
        mirror_lock: Mutex<()>,
    },
}

impl DbPool {
    pub fn open_at(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path).map_err(AppError::BaseDeDonnees)?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .map_err(AppError::BaseDeDonnees)?;
        Ok(Self {
            backend: Arc::new(DbBackend::Sqlite {
                inner: Mutex::new(conn),
                path,
            }),
        })
    }

    pub fn open_production() -> AppResult<Self> {
        let remote = RemoteDatabaseConfig::load()?;
        if remote.enabled {
            return Self::open_mysql(&remote);
        }
        Self::open_at(production_database_path()?)
    }

    pub fn open_demo() -> AppResult<Self> {
        Self::open_at(demo_database_path()?)
    }

    pub fn open_mysql(cfg: &RemoteDatabaseConfig) -> AppResult<Self> {
        cfg.validate_for_connect()?;
        let mut builder = OptsBuilder::new()
            .ip_or_hostname(Some(cfg.host.trim()))
            .tcp_port(cfg.port)
            .user(Some(cfg.username.trim()))
            .pass(Some(cfg.password.as_str()))
            .db_name(Some(cfg.database.trim()))
            .prefer_socket(false);

        if cfg.use_tls {
            let ssl = if cfg.danger_accept_invalid_certs {
                SslOpts::default().with_danger_accept_invalid_certs(true)
            } else {
                SslOpts::default()
            };
            builder = builder.ssl_opts(Some(ssl));
        }

        let opts = Opts::from(builder);
        let pool = MysqlPool::new(opts).map_err(|e| {
            AppError::interne(format!(
                "Connexion MySQL o2switch impossible ({}) : {e}",
                cfg.mysql_opts_url_hint()
            ))
        })?;

        // Ping
        {
            let mut conn = pool
                .get_conn()
                .map_err(|e| AppError::interne(format!("Pool MySQL : {e}")))?;
            conn.query_drop("SELECT 1")
                .map_err(|e| AppError::interne(format!("Ping MySQL : {e}")))?;
        }

        Ok(Self {
            backend: Arc::new(DbBackend::Mysql {
                pool,
                label: cfg.mysql_opts_url_hint(),
                mirror_lock: Mutex::new(()),
            }),
        })
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.backend.as_ref(), DbBackend::Mysql { .. })
    }

    pub fn connection_path(&self) -> PathBuf {
        match self.backend.as_ref() {
            DbBackend::Sqlite { path, .. } => path.clone(),
            DbBackend::Mysql { label, .. } => PathBuf::from(label),
        }
    }

    pub fn with_conn<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&Connection) -> AppResult<R>,
    {
        match self.backend.as_ref() {
            DbBackend::Sqlite { inner, .. } => {
                let guard = inner.lock();
                f(&guard)
            }
            DbBackend::Mysql {
                pool,
                mirror_lock,
                ..
            } => {
                let _guard = mirror_lock.lock();
                crate::db::mysql_mirror::with_mysql_mirror_read(pool, f)
            }
        }
    }

    pub fn with_conn_mut<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&mut Connection) -> AppResult<R>,
    {
        match self.backend.as_ref() {
            DbBackend::Sqlite { inner, .. } => {
                let mut guard = inner.lock();
                f(&mut guard)
            }
            DbBackend::Mysql {
                pool,
                mirror_lock,
                ..
            } => {
                let _guard = mirror_lock.lock();
                crate::db::mysql_mirror::with_mysql_mirror_write(pool, f)
            }
        }
    }

    /// Exécute un SQL portable. Placeholders : `?` ou `?1`.
    pub fn execute(&self, sql: &str, params: &[SqlVal]) -> AppResult<u64> {
        match self.backend.as_ref() {
            DbBackend::Sqlite { inner, .. } => {
                let conn = inner.lock();
                let bind = to_rusqlite_params(params)?;
                let refs: Vec<&dyn ToSql> = bind.iter().map(|v| v as &dyn ToSql).collect();
                let n = conn
                    .execute(sql, refs.as_slice())
                    .map_err(AppError::BaseDeDonnees)?;
                Ok(n as u64)
            }
            DbBackend::Mysql { pool, .. } => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| AppError::interne(format!("MySQL : {e}")))?;
                let mysql_sql = to_mysql_placeholders(sql);
                let values = to_mysql_values(params);
                conn.exec_drop(mysql_sql.as_str(), values)
                    .map_err(|e| AppError::interne(format!("MySQL execute : {e}")))?;
                Ok(conn.affected_rows())
            }
        }
    }

    pub fn execute_batch_script(&self, script: &str) -> AppResult<()> {
        match self.backend.as_ref() {
            DbBackend::Sqlite { inner, .. } => {
                let conn = inner.lock();
                conn.execute_batch(script)
                    .map_err(AppError::BaseDeDonnees)?;
                Ok(())
            }
            DbBackend::Mysql { pool, .. } => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| AppError::interne(format!("MySQL : {e}")))?;
                let cleaned = strip_sqlite_pragmas(script);
                for stmt in split_sql_statements(&cleaned) {
                    if stmt.trim().is_empty() {
                        continue;
                    }
                    let mysql_sql = to_mysql_placeholders(stmt);
                    conn.query_drop(mysql_sql.as_str())
                        .map_err(|e| AppError::interne(format!("MySQL batch : {e} | SQL: {stmt}")))?;
                }
                Ok(())
            }
        }
    }

    pub fn query(&self, sql: &str, params: &[SqlVal]) -> AppResult<Vec<Vec<SqlVal>>> {
        match self.backend.as_ref() {
            DbBackend::Sqlite { inner, .. } => {
                let conn = inner.lock();
                let bind = to_rusqlite_params(params)?;
                let refs: Vec<&dyn ToSql> = bind.iter().map(|v| v as &dyn ToSql).collect();
                let mut stmt = conn.prepare(sql).map_err(AppError::BaseDeDonnees)?;
                let col_count = stmt.column_count();
                let mut rows = stmt.query(refs.as_slice()).map_err(AppError::BaseDeDonnees)?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().map_err(AppError::BaseDeDonnees)? {
                    let mut cells = Vec::with_capacity(col_count);
                    for i in 0..col_count {
                        cells.push(rusqlite_cell(row, i)?);
                    }
                    out.push(cells);
                }
                Ok(out)
            }
            DbBackend::Mysql { pool, .. } => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| AppError::interne(format!("MySQL : {e}")))?;
                let mysql_sql = to_mysql_placeholders(sql);
                let values = to_mysql_values(params);
                let result = conn
                    .exec_iter(mysql_sql.as_str(), values)
                    .map_err(|e| AppError::interne(format!("MySQL query : {e}")))?;
                let mut out = Vec::new();
                for row_res in result {
                    let row: mysql::Row =
                        row_res.map_err(|e| AppError::interne(format!("MySQL row : {e}")))?;
                    let mut cells = Vec::with_capacity(row.len());
                    for i in 0..row.len() {
                        let value: MysqlValue = row.get(i).unwrap_or(MysqlValue::NULL);
                        cells.push(mysql_cell(value));
                    }
                    out.push(cells);
                }
                Ok(out)
            }
        }
    }

    pub fn query_one(&self, sql: &str, params: &[SqlVal]) -> AppResult<Vec<SqlVal>> {
        let mut rows = self.query(sql, params)?;
        rows.pop()
            .ok_or(AppError::Introuvable)
            .or_else(|_| Err(AppError::Introuvable))
    }

    pub fn query_one_opt(&self, sql: &str, params: &[SqlVal]) -> AppResult<Option<Vec<SqlVal>>> {
        let mut rows = self.query(sql, params)?;
        Ok(rows.pop())
    }

    pub fn checkpoint_and_close(self) -> AppResult<()> {
        if let DbBackend::Sqlite { inner, .. } = self.backend.as_ref() {
            let conn = inner.lock();
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn open_in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory().map_err(AppError::BaseDeDonnees)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(AppError::BaseDeDonnees)?;
        Ok(Self {
            backend: Arc::new(DbBackend::Sqlite {
                inner: Mutex::new(conn),
                path: PathBuf::from(":memory:"),
            }),
        })
    }
}

fn to_rusqlite_params(params: &[SqlVal]) -> AppResult<Vec<rusqlite::types::Value>> {
    Ok(params
        .iter()
        .map(|p| match p {
            SqlVal::Null => rusqlite::types::Value::Null,
            SqlVal::Text(s) => rusqlite::types::Value::Text(s.clone()),
            SqlVal::Int(v) => rusqlite::types::Value::Integer(*v),
            SqlVal::Blob(b) => rusqlite::types::Value::Blob(b.clone()),
        })
        .collect())
}

fn to_mysql_values(params: &[SqlVal]) -> Vec<MysqlValue> {
    params
        .iter()
        .map(|p| match p {
            SqlVal::Null => MysqlValue::NULL,
            SqlVal::Text(s) => s.to_value(),
            SqlVal::Int(v) => v.to_value(),
            SqlVal::Blob(b) => b.to_value(),
        })
        .collect()
}

fn rusqlite_cell(row: &rusqlite::Row<'_>, i: usize) -> AppResult<SqlVal> {
    let v = row.get_ref(i).map_err(AppError::BaseDeDonnees)?;
    Ok(match v {
        ValueRef::Null => SqlVal::Null,
        ValueRef::Integer(n) => SqlVal::Int(n),
        ValueRef::Real(f) => SqlVal::Text(f.to_string()),
        ValueRef::Text(t) => SqlVal::Text(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(b) => SqlVal::Blob(b.to_vec()),
    })
}

fn mysql_cell(value: MysqlValue) -> SqlVal {
    match value {
        MysqlValue::NULL => SqlVal::Null,
        MysqlValue::Bytes(b) => match String::from_utf8(b.clone()) {
            Ok(s) => SqlVal::Text(s),
            Err(_) => SqlVal::Blob(b),
        },
        MysqlValue::Int(n) => SqlVal::Int(n),
        MysqlValue::UInt(n) => SqlVal::Int(n as i64),
        MysqlValue::Float(f) => SqlVal::Text(f.to_string()),
        MysqlValue::Double(f) => SqlVal::Text(f.to_string()),
        other => SqlVal::Text(format!("{other:?}")),
    }
}

fn split_sql_statements(script: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_single = false;
    let mut in_double = false;
    let bytes = script.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'\'' if !in_double => in_single = !in_single,
            b'"' if !in_single => in_double = !in_double,
            b';' if !in_single && !in_double => {
                out.push(&script[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < script.len() {
        out.push(&script[start..]);
    }
    out
}
