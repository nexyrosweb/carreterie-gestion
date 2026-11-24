use lettre::message::{header::ContentType, Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{Message, SmtpTransport, Transport};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::invoice::SendDocumentEmailInput;
use crate::services::audit::AuditService;
use crate::services::contacts::ContactsService;
use crate::services::invoices::InvoicesService;
use crate::services::pdf::PdfService;
use crate::services::settings::SettingsService;

pub struct MailService;

impl MailService {
    pub fn send_document(
        db: &DbPool,
        user_id: &str,
        input: SendDocumentEmailInput,
        demo: bool,
    ) -> AppResult<()> {
        let settings = SettingsService::load(db)?;
        if !settings.smtp_enabled {
            return Err(AppError::interdit(
                "SMTP désactivé. Activez-le dans Paramètres.",
            ));
        }
        if settings.smtp_host.trim().is_empty() || settings.smtp_from_email.trim().is_empty() {
            return Err(AppError::validation(
                "Hôte SMTP et e-mail expéditeur obligatoires.",
            ));
        }

        let invoice = InvoicesService::get(db, &input.invoice_id)?;
        let contact = ContactsService::get(db, &invoice.contact_id)?;
        let to = input
            .to_email
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                contact
                    .email
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .ok_or_else(|| {
                AppError::validation("Aucun e-mail destinataire (saisie ou fiche contact).")
            })?;

        let title = invoice.invoice_kind.document_title();
        let number = invoice.number.as_deref().unwrap_or("BROUILLON");
        let subject = input
            .subject
            .unwrap_or_else(|| format!("{title} {number} — {}", settings.theatre_name));
        let body = input.body.unwrap_or_else(|| {
            format!(
                "Bonjour,\n\nVeuillez trouver ci-joint le document {title} {number}.\n\nCordialement,\n{}",
                settings.theatre_name
            )
        });

        let pdf_bytes = PdfService::generate_invoice_pdf_base64(db, &input.invoice_id, demo)
            .and_then(|b64| {
                use base64::{engine::general_purpose::STANDARD, Engine as _};
                STANDARD
                    .decode(b64)
                    .map_err(|e| AppError::interne(format!("Décodage PDF : {e}")))
            })?;

        let from_name = if settings.smtp_from_name.trim().is_empty() {
            settings.theatre_name.clone()
        } else {
            settings.smtp_from_name.clone()
        };

        let mut builder = Message::builder()
            .from(
                format!("{from_name} <{}>", settings.smtp_from_email)
                    .parse()
                    .map_err(|e| AppError::validation(format!("Expéditeur invalide : {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| AppError::validation(format!("Destinataire invalide : {e}")))?)
            .subject(subject);

        if !settings.smtp_reply_to.trim().is_empty() {
            builder = builder.reply_to(
                settings
                    .smtp_reply_to
                    .parse()
                    .map_err(|e| AppError::validation(format!("Reply-To invalide : {e}")))?,
            );
        }

        let filename = format!("{title}_{number}.pdf").replace(' ', "_");
        let attachment = Attachment::new(filename).body(
            pdf_bytes,
            ContentType::parse("application/pdf")
                .unwrap_or(ContentType::TEXT_PLAIN),
        );
        let multipart = MultiPart::mixed()
            .singlepart(SinglePart::plain(body))
            .singlepart(attachment);

        let email = builder
            .multipart(multipart)
            .map_err(|e| AppError::interne(format!("Construction e-mail : {e}")))?;

        let mailer = build_transport(&settings)?;
        mailer
            .send(&email)
            .map_err(|e| AppError::interne(format!("Envoi SMTP échoué : {e}")))?;

        AuditService::log(
            db,
            user_id,
            "document_envoye_email",
            Some("invoice"),
            Some(&input.invoice_id),
            Some(to),
        )?;
        Ok(())
    }

    pub fn test_connection(db: &DbPool) -> AppResult<String> {
        let settings = SettingsService::load(db)?;
        if settings.smtp_host.trim().is_empty() {
            return Err(AppError::validation("Hôte SMTP manquant."));
        }
        let mailer = build_transport(&settings)?;
        mailer
            .test_connection()
            .map_err(|e| AppError::interne(format!("Test SMTP échoué : {e}")))?;
        Ok("Connexion SMTP réussie.".into())
    }
}

fn build_transport(settings: &crate::models::settings::AppSettings) -> AppResult<SmtpTransport> {
    let creds = Credentials::new(
        settings.smtp_username.clone(),
        settings.smtp_password.clone(),
    );
    let host = settings.smtp_host.trim();
    let port = settings.smtp_port;

    let builder = match settings.smtp_encryption.as_str() {
        "ssl" | "tls" => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|e| AppError::interne(format!("TLS : {e}")))?;
            SmtpTransport::relay(host)
                .map_err(|e| AppError::interne(format!("SMTP : {e}")))?
                .port(port)
                .tls(Tls::Wrapper(tls))
        }
        "none" => SmtpTransport::builder_dangerous(host).port(port),
        _ => {
            let tls = TlsParameters::new(host.to_string())
                .map_err(|e| AppError::interne(format!("TLS : {e}")))?;
            SmtpTransport::relay(host)
                .map_err(|e| AppError::interne(format!("SMTP : {e}")))?
                .port(port)
                .tls(Tls::Required(tls))
        }
    };

    Ok(builder.credentials(creds).build())
}
