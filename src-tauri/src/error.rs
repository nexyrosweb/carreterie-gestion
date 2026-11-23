use serde::Serialize;

use crate::DISCLAIMER_E_FACTURATION;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Erreur interne : {0}")]
    Interne(String),

    #[error("Session invalide ou expirée. Veuillez vous reconnecter.")]
    SessionInvalide,

    #[error("Identifiants incorrects.")]
    IdentifiantsIncorrects,

    #[error(
        "Compte temporairement verrouillé après plusieurs tentatives. Réessayez dans {minutes} minute(s)."
    )]
    CompteVerrouille { minutes: u64 },

    #[error("Un administrateur existe déjà. Utilisez la connexion.")]
    AdminDejaConfigure,

    #[error("Aucun utilisateur configuré. Créez le compte administrateur.")]
    PremiereConfigurationRequise,

    #[error("Mot de passe actuel incorrect.")]
    MotDePasseActuelIncorrect,

    #[error("Ressource introuvable.")]
    Introuvable,

    #[error("Opération interdite : {0}")]
    Interdit(String),

    #[error("Données invalides : {0}")]
    Validation(String),

    #[error("Erreur base de données : {0}")]
    BaseDeDonnees(#[from] rusqlite::Error),

    #[error("Erreur système de fichiers : {0}")]
    Fichier(#[from] std::io::Error),

    #[error("Erreur CSV : {0}")]
    Csv(#[from] csv::Error),

    #[error("Erreur archive ZIP : {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("{0}")]
    Autre(#[from] anyhow::Error),
}

impl AppError {
    pub fn interne(msg: impl Into<String>) -> Self {
        Self::Interne(msg.into())
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn interdit(msg: impl Into<String>) -> Self {
        Self::Interdit(msg.into())
    }
}

#[derive(Serialize)]
pub struct ErrorPayload {
    pub message: String,
    pub code: &'static str,
    pub disclaimer: &'static str,
}

impl From<AppError> for ErrorPayload {
    fn from(err: AppError) -> Self {
        let code = match &err {
            AppError::SessionInvalide => "SESSION_INVALIDE",
            AppError::IdentifiantsIncorrects => "IDENTIFIANTS_INCORRECTS",
            AppError::CompteVerrouille { .. } => "COMPTE_VERROUILLE",
            AppError::AdminDejaConfigure => "ADMIN_DEJA_CONFIGURE",
            AppError::PremiereConfigurationRequise => "PREMIERE_CONFIG",
            AppError::MotDePasseActuelIncorrect => "MOT_DE_PASSE_INCORRECT",
            AppError::Introuvable => "INTROUVABLE",
            AppError::Interdit(_) => "INTERDIT",
            AppError::Validation(_) => "VALIDATION",
            _ => "ERREUR",
        };
        ErrorPayload {
            message: err.to_string(),
            code,
            disclaimer: DISCLAIMER_E_FACTURATION,
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;