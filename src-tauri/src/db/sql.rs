//! API SQL portable SQLite (local/démo) + MySQL (o2switch).

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub enum SqlVal {
    Null,
    Text(String),
    Int(i64),
    Blob(Vec<u8>),
}

impl SqlVal {
    pub fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }
    pub fn int(v: i64) -> Self {
        Self::Int(v)
    }
    pub fn opt_text(s: Option<impl Into<String>>) -> Self {
        match s {
            Some(v) => Self::Text(v.into()),
            None => Self::Null,
        }
    }

    pub fn as_i64(&self) -> AppResult<i64> {
        match self {
            Self::Int(v) => Ok(*v),
            Self::Text(s) => s
                .parse()
                .map_err(|_| AppError::interne(format!("Entier attendu, reçu « {s} »"))),
            Self::Null => Ok(0),
            Self::Blob(_) => Err(AppError::interne("Entier attendu, blob reçu")),
        }
    }

    pub fn as_string(&self) -> AppResult<String> {
        match self {
            Self::Text(s) => Ok(s.clone()),
            Self::Int(v) => Ok(v.to_string()),
            Self::Null => Ok(String::new()),
            Self::Blob(_) => Err(AppError::interne("Texte attendu, blob reçu")),
        }
    }

    pub fn as_opt_string(&self) -> Option<String> {
        match self {
            Self::Null => None,
            Self::Text(s) if s.is_empty() => None,
            Self::Text(s) => Some(s.clone()),
            Self::Int(v) => Some(v.to_string()),
            Self::Blob(_) => None,
        }
    }

    pub fn as_blob(&self) -> AppResult<Vec<u8>> {
        match self {
            Self::Blob(b) => Ok(b.clone()),
            Self::Null => Ok(Vec::new()),
            _ => Err(AppError::interne("Blob attendu")),
        }
    }
}

impl From<&str> for SqlVal {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}
impl From<String> for SqlVal {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}
impl From<i64> for SqlVal {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<u32> for SqlVal {
    fn from(v: u32) -> Self {
        Self::Int(v as i64)
    }
}
impl From<Option<String>> for SqlVal {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(v) => Self::Text(v),
            None => Self::Null,
        }
    }
}
impl From<&Option<String>> for SqlVal {
    fn from(s: &Option<String>) -> Self {
        match s {
            Some(v) => Self::Text(v.clone()),
            None => Self::Null,
        }
    }
}

/// Remplace `?1`..`?n` et `?` SQLite par des `?` positionnels MySQL.
pub fn to_mysql_placeholders(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'?' {
            out.push('?');
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    // Dialecte
    out.replace("INSERT OR IGNORE", "INSERT IGNORE")
        .replace("insert or ignore", "INSERT IGNORE")
}

pub fn strip_sqlite_pragmas(sql: &str) -> String {
    sql.lines()
        .filter(|l| {
            let t = l.trim().to_ascii_uppercase();
            !t.starts_with("PRAGMA ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mysql_placeholders() {
        assert_eq!(
            to_mysql_placeholders("SELECT * FROM t WHERE a = ?1 AND b = ?2"),
            "SELECT * FROM t WHERE a = ? AND b = ?"
        );
        assert_eq!(
            to_mysql_placeholders("INSERT OR IGNORE INTO t (x) VALUES (?1)"),
            "INSERT IGNORE INTO t (x) VALUES (?)"
        );
    }
}
