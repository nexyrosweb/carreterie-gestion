//! Configuration base partagée o2switch (MySQL distant) — fichier local hors SQLite.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paths::app_data_root;

pub const REMOTE_CONFIG_FILE: &str = "remote_database.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteDatabaseConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    /// Stocké localement chiffré n’est pas encore en place : fichier ACL Windows AppData.
    pub password: String,
    pub use_tls: bool,
    /// Accepter le certificat serveur o2switch (souvent nécessaire).
    pub danger_accept_invalid_certs: bool,
}

impl Default for RemoteDatabaseConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 3306,
            database: String::new(),
            username: String::new(),
            password: String::new(),
            use_tls: true,
            danger_accept_invalid_certs: true,
        }
    }
}

impl RemoteDatabaseConfig {
    pub fn config_path() -> AppResult<PathBuf> {
        Ok(app_data_root()?.join(REMOTE_CONFIG_FILE))
    }

    pub fn load() -> AppResult<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&path)?;
        let cfg: Self = serde_json::from_str(&raw)
            .map_err(|e| AppError::interne(format!("Config distante invalide : {e}")))?;
        Ok(cfg)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::interne(format!("Sérialisation config distante : {e}")))?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn mysql_opts_url_hint(&self) -> String {
        format!(
            "mysql://{}@{}:{}/{}",
            self.username, self.host, self.port, self.database
        )
    }

    pub fn validate_for_connect(&self) -> AppResult<()> {
        if self.host.trim().is_empty() {
            return Err(AppError::validation("Hôte MySQL o2switch obligatoire."));
        }
        if self.database.trim().is_empty() {
            return Err(AppError::validation("Nom de base MySQL obligatoire."));
        }
        if self.username.trim().is_empty() {
            return Err(AppError::validation("Utilisateur MySQL obligatoire."));
        }
        Ok(())
    }
}
