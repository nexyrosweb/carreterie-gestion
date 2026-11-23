use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::error::{AppError, AppResult};
use crate::paths::{demo_database_path, production_database_path};

#[derive(Clone)]
pub struct DbPool {
    inner: Arc<Mutex<Connection>>,
    path: PathBuf,
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
            inner: Arc::new(Mutex::new(conn)),
            path,
        })
    }

    pub fn open_production() -> AppResult<Self> {
        Self::open_at(production_database_path()?)
    }

    pub fn open_demo() -> AppResult<Self> {
        Self::open_at(demo_database_path()?)
    }

    pub fn with_conn<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&Connection) -> AppResult<R>,
    {
        let guard = self.inner.lock();
        f(&guard)
    }

    pub fn with_conn_mut<F, R>(&self, f: F) -> AppResult<R>
    where
        F: FnOnce(&mut Connection) -> AppResult<R>,
    {
        let mut guard = self.inner.lock();
        f(&mut guard)
    }

    pub fn connection_path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Ferme proprement les writers avant remplacement du fichier (réinit démo / restore).
    pub fn checkpoint_and_close(self) -> AppResult<()> {
        {
            let conn = self.inner.lock();
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
            inner: Arc::new(Mutex::new(conn)),
            path: PathBuf::from(":memory:"),
        })
    }
}
