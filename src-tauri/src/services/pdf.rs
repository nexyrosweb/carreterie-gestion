use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::PathBuf;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{Datelike, NaiveDate, Utc};
use printpdf::{
    image_crate::io::Reader as ImageReader,
    path::{PaintMode, WindingOrder},
    BuiltinFont, Color, Image, ImageTransform, Mm, PdfDocument, PdfDocumentReference,
    PdfLayerReference, Rect, Rgb,
};

use crate::db::DbPool;
use crate::error::{AppError, AppResult};
use crate::models::invoice::{InvoiceKind, InvoiceLineKind};
use crate::models::settings::AppSettings;
use crate::money::format_amount;
use crate::paths::attachments_dir;
use crate::services::contacts::ContactsService;
use crate::services::invoices::InvoicesService;
use crate::services::settings::SettingsService;
use crate::siret::{format_siren, format_siret};
use crate::DISCLAIMER_E_FACTURATION;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PdfTemplate {
    Classique,
    Compact,
    Billetterie,
}

impl PdfTemplate {
    fn from_settings(s: &str) -> Self {
        match s {
            "compact" => Self::Compact,
            "billetterie" => Self::Billetterie,
            _ => Self::Classique,
        }
    }
}

/// Logo officiel embarqué (même fichier que `src/assets/brand/logo-theatre-carreterie.jpg`).
const BRAND_LOGO_JPEG: &[u8] = include_bytes!("../../resources/logo-theatre-carreterie.jpg");

pub struct PdfService;

impl PdfService {
    pub fn generate_invoice_pdf(db: &DbPool, invoice_id: &str, demo: bool) -> AppResult<PathBuf> {
        let bytes = Self::render_pdf_bytes(db, invoice_id, demo)?;
        let invoice = InvoicesService::get(db, invoice_id)?;
        let out_dir = attachments_dir()?.join("pdf");
        std::fs::create_dir_all(&out_dir)?;
        let file_name = format!(
            "{}_{}.pdf",
            invoice
                .number
                .as_deref()
                .unwrap_or(invoice.invoice_kind.as_db_str()),
            Utc::now().format("%Y%m%d%H%M%S")
        );
        let path = out_dir.join(file_name);
        let mut file = File::create(&path)?;
        file.write_all(&bytes)?;
        let _ = crate::db::blobs::BlobStore::put_file(db, &path, "application/pdf");
        Ok(path)
    }

    pub fn generate_invoice_pdf_base64(
        db: &DbPool,
        invoice_id: &str,
        demo: bool,
    ) -> AppResult<String> {
        let bytes = Self::render_pdf_bytes(db, invoice_id, demo)?;
        Ok(STANDARD.encode(bytes))
    }

    pub fn generate_cession_contract_pdf(
        db: &DbPool,
        devis_id: &str,
        demo: bool,
    ) -> AppResult<PathBuf> {
        let bytes = Self::render_cession_bytes(db, devis_id, demo)?;
        let out_dir = attachments_dir()?.join("pdf");
        std::fs::create_dir_all(&out_dir)?;
        let path = out_dir.join(format!(
            "cession_{}_{}.pdf",
            &devis_id[..8.min(devis_id.len())],
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        let mut file = File::create(&path)?;
        file.write_all(&bytes)?;
        Ok(path)
    }

    pub fn generate_cession_contract_pdf_base64(
        db: &DbPool,
        devis_id: &str,
        demo: bool,
    ) -> AppResult<String> {
        let bytes = Self::render_cession_bytes(db, devis_id, demo)?;
        Ok(STANDARD.encode(bytes))
    }

    fn render_pdf_bytes(db: &DbPool, invoice_id: &str, demo: bool) -> AppResult<Vec<u8>> {
        let invoice = InvoicesService::get(db, invoice_id)?;
        let contact = ContactsService::get(db, &invoice.contact_id)?;
        let settings = SettingsService::load(db)?;
        let primary = parse_hex_rgb(&settings.pdf_primary_color);
        let template = PdfTemplate::from_settings(&settings.pdf_template);
        let compact = template == PdfTemplate::Compact;
        let billetterie = template == PdfTemplate::Billetterie;

        let doc_label = invoice.invoice_kind.document_title();
        let number_display = invoice
            .number
            .clone()
            .unwrap_or_else(|| "BROUILLON".to_string());
        let date_display = invoice
            .issue_date
            .map(format_date_fr)
            .unwrap_or_else(|| "—".to_string());

        let (doc, page1, layer1) =
            PdfDocument::new(doc_label, Mm(210.0), Mm(297.0), "Contenu");
        let layer = doc.get_page(page1).get_layer(layer1);
        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;
        let font_italic = doc
            .add_builtin_font(BuiltinFont::HelveticaOblique)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;

        if demo {
            write_line(
                &layer,
                &font_bold,
                11.0,
                55.0,
                288.0,
                "DEMONSTRATION - SANS VALEUR COMPTABLE",
            );
        }

        let _ = place_logo(&layer, &settings);

        let name_size = if compact { 10.0 } else { 12.0 };
        let body_size = if compact { 8.0 } else { 9.0 };
        let step = if compact { 3.8 } else { 4.5 };

        let mut y = if compact { 282.0 } else { 278.0 };
        write_line(
            &layer,
            &font_bold,
            name_size,
            18.0,
            y,
            &settings.theatre_name.to_uppercase(),
        );
        y -= if compact { 4.5 } else { 5.5 };
        if !settings.pdf_header_subtitle.trim().is_empty() {
            write_line(
                &layer,
                &font_italic,
                if compact { 7.5 } else { 8.5 },
                18.0,
                y,
                settings.pdf_header_subtitle.trim(),
            );
            y -= step;
        }
        for line in settings.theatre_address.lines().take(if compact { 2 } else { 3 }) {
            write_line(&layer, &font, body_size, 18.0, y, &line.to_uppercase());
            y -= step;
        }
        if !settings.theatre_siret.is_empty() {
            write_line(
                &layer,
                &font,
                if compact { 7.5 } else { 8.5 },
                18.0,
                y,
                &format!("Siret : {}", format_siret(&settings.theatre_siret)),
            );
            y -= step;
        }
        // Coordonnées bancaires en-tête (modèle classique / billetterie ; compact = IBAN seul)
        if !compact {
            let _ = write_bank_header_lines(&layer, &font, &settings, y, step);
        } else if !settings.bank_iban.is_empty() {
            write_line(
                &layer,
                &font,
                7.5,
                18.0,
                y,
                &format!("IBAN : {}", settings.bank_iban),
            );
        }

        let client_box_y = if compact { 235.0 } else { 228.0 };
        let client_box_h = if compact { 26.0 } else { 32.0 };
        draw_filled_rect_rgb(&layer, 118.0, client_box_y, 74.0, client_box_h, primary, 0.12);
        let mut cy = if compact { 255.0 } else { 252.0 };
        write_line(
            &layer,
            &font_bold,
            if compact { 9.0 } else { 10.0 },
            122.0,
            cy,
            &contact.name,
        );
        cy -= if compact { 4.2 } else { 5.0 };
        if let Some(a) = &contact.address_line1 {
            write_line(&layer, &font, body_size, 122.0, cy, a);
            cy -= step;
        }
        if let Some(cp) = &contact.postal_code {
            let city = contact.city.as_deref().unwrap_or("");
            write_line(
                &layer,
                &font,
                body_size,
                122.0,
                cy,
                &format!("{cp} {city}"),
            );
            cy -= step;
        }
        if let Some(siret) = &contact.siret {
            write_line(
                &layer,
                &font,
                body_size,
                122.0,
                cy,
                &format!("SIREN : {}", format_siren(siret)),
            );
        }

        let title_y = if compact { 212.0 } else { 205.0 };
        if billetterie {
            draw_filled_rect_rgb(&layer, 18.0, title_y, 174.0, 16.0, primary, 0.18);
            let title = format!("{doc_label}  ·  N° {number_display}  ·  {date_display}");
            write_line(&layer, &font_bold, 12.0, 22.0, title_y + 5.5, &title);
            write_line(
                &layer,
                &font,
                8.0,
                22.0,
                title_y + 1.0,
                "Modèle billetterie",
            );
        } else {
            draw_rect_stroke_rgb(&layer, 45.0, title_y, 120.0, if compact { 11.0 } else { 14.0 }, primary);
            draw_filled_rect_rgb(
                &layer,
                45.5,
                title_y + 0.5,
                119.0,
                if compact { 10.0 } else { 13.0 },
                primary,
                0.08,
            );
            let title = format!("{doc_label} N° {number_display} en date du {date_display}");
            write_line(
                &layer,
                &font_bold,
                if compact { 9.5 } else { 11.0 },
                52.0,
                title_y + 5.0,
                &title,
            );
        }

        let box_bottom = if compact { 155.0 } else { 145.0 };
        let box_h = if compact { 38.0 } else { 48.0 };
        draw_filled_rect_rgb(&layer, 18.0, box_bottom, 174.0, box_h, primary, 0.10);
        let has_location = invoice
            .lines
            .iter()
            .any(|l| l.line_kind == InvoiceLineKind::LocationSalle);
        let prestation = if has_location {
            let mut parts = vec!["Location de salle".to_string()];
            if let (Some(s), Some(e)) = (&invoice.rental_start, &invoice.rental_end) {
                parts.push(format!("du {s} au {e}"));
            }
            if let Some(n) = invoice.customer_notes.as_deref().filter(|s| !s.trim().is_empty()) {
                parts.push(n.to_string());
            }
            format!("Prestation : {}", parts.join(" — "))
        } else {
            invoice
                .customer_notes
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .map(|s| format!("Prestation : {s}"))
                .or_else(|| {
                    invoice
                        .lines
                        .first()
                        .map(|l| format!("Prestation : {}", l.description))
                })
                .unwrap_or_else(|| "Prestation :".to_string())
        };
        let prest_y = box_bottom + box_h - 10.0;
        write_line(
            &layer,
            &font_italic,
            if compact { 9.0 } else { 10.0 },
            22.0,
            prest_y,
            &prestation,
        );

        let tax_rate = invoice
            .lines
            .first()
            .map(|l| format_amount(l.tax_rate_percent))
            .unwrap_or_else(|| "20.00".into());

        let totals_x = if billetterie { 22.0 } else { 130.0 };
        let ttc_y = box_bottom + 30.0;
        let ttc_size = if billetterie {
            14.0
        } else if compact {
            9.5
        } else {
            10.0
        };
        write_line(
            &layer,
            &font_bold,
            ttc_size,
            totals_x,
            ttc_y,
            &format!("TTC : {} €", format_amount(invoice.total_ttc)),
        );
        write_line(
            &layer,
            &font,
            if compact { 8.5 } else { 9.5 },
            totals_x,
            ttc_y - 7.0,
            &format!(
                "Dont TVA {tax_rate} % : {} €",
                format_amount(invoice.total_tva)
            ),
        );
        write_line(
            &layer,
            &font,
            if compact { 8.5 } else { 9.5 },
            totals_x,
            ttc_y - 14.0,
            &format!("HT : {} €", format_amount(invoice.total_ht)),
        );

        let mut ly = box_bottom - 8.0;
        let detail_floor = if invoice.invoice_kind == InvoiceKind::Facture
            && invoice.payment_status != "payee"
        {
            88.0
        } else {
            70.0
        };
        if !compact && (invoice.lines.len() > 1 || has_location) {
            write_line(&layer, &font_bold, 9.0, 18.0, ly, "Détail");
            ly -= 5.0;
            for line in &invoice.lines {
                let kind_tag = match line.line_kind {
                    InvoiceLineKind::LocationSalle => "[Location] ",
                    InvoiceLineKind::Standard => "",
                };
                write_line(
                    &layer,
                    &font,
                    8.5,
                    18.0,
                    ly,
                    &format!(
                        "{kind_tag}{} — qté {} — {} € HT — TVA {} % — {} € TTC",
                        truncate(&line.description, 38),
                        format_amount(line.quantity),
                        format_amount(line.unit_price_ht),
                        format_amount(line.tax_rate_percent),
                        format_amount(line.line_ttc),
                    ),
                );
                ly -= 4.5;
                if ly < detail_floor {
                    break;
                }
            }
        } else if compact && !invoice.lines.is_empty() {
            for line in invoice.lines.iter().take(4) {
                write_line(
                    &layer,
                    &font,
                    7.5,
                    18.0,
                    ly,
                    &format!(
                        "{} · {} × {} € HT",
                        truncate(&line.description, 42),
                        format_amount(line.quantity),
                        format_amount(line.unit_price_ht),
                    ),
                );
                ly -= 3.8;
                if ly < detail_floor {
                    break;
                }
            }
        }

        if has_location && !compact {
            write_line(
                &layer,
                &font,
                7.5,
                18.0,
                58.0,
                "Clause location : la salle est mise à disposition selon les horaires indiqués ; toute prolongation fait l'objet d'un avenant.",
            );
        }

        let unpaid = invoice.invoice_kind == InvoiceKind::Facture
            && invoice.payment_status != "payee";
        let reste = (invoice.total_ttc - invoice.amount_paid).max(rust_decimal_macros::dec!(0));

        match invoice.invoice_kind {
            InvoiceKind::Devis => {
                write_line(&layer, &font_bold, 9.0, 18.0, 48.0, "Devis");
                write_line(
                    &layer,
                    &font,
                    8.5,
                    18.0,
                    42.0,
                    "Document sans valeur comptable. Valable 30 jours sauf mention contraire.",
                );
                if let Some(d) = invoice.due_date {
                    write_line(
                        &layer,
                        &font,
                        8.5,
                        18.0,
                        36.0,
                        &format!("Validité jusqu'au {}.", format_date_fr(d)),
                    );
                }
            }
            InvoiceKind::Avoir => {
                write_line(&layer, &font_bold, 9.0, 18.0, 48.0, "Avoir");
                write_line(
                    &layer,
                    &font,
                    8.5,
                    18.0,
                    42.0,
                    &format!(
                        "Avoir de {} € au profit du client.",
                        format_amount(invoice.total_ttc.abs())
                    ),
                );
            }
            InvoiceKind::Facture => {
                if unpaid {
                    write_unpaid_virement_block(
                        &layer,
                        &font,
                        &font_bold,
                        &settings,
                        reste,
                        primary,
                        compact,
                    );
                } else {
                    write_line(&layer, &font_bold, 9.0, 18.0, 52.0, "Montant réglé");
                    let method = "virement bancaire";
                    let paid_date = invoice
                        .issue_date
                        .map(|d| d.format("%d/%m/%y").to_string())
                        .unwrap_or_default();
                    write_line(
                        &layer,
                        &font,
                        8.5,
                        18.0,
                        46.0,
                        &format!(
                            "{} € par {method} au {paid_date}",
                            format_amount(invoice.amount_paid)
                        ),
                    );
                    write_line(
                        &layer,
                        &font,
                        8.5,
                        18.0,
                        40.0,
                        &format!("Au profit de {}.", settings.theatre_name),
                    );
                }
            }
        }

        write_line(&layer, &font, 7.0, 18.0, 22.0, &settings.legal_mentions);
        write_line(&layer, &font, 6.5, 18.0, 16.0, DISCLAIMER_E_FACTURATION);
        if demo {
            write_line(
                &layer,
                &font_bold,
                8.0,
                18.0,
                10.0,
                "DÉMONSTRATION — SANS VALEUR COMPTABLE — Nexyros",
            );
        }

        let mut buf = Vec::new();
        {
            let mut writer = BufWriter::new(Cursor::new(&mut buf));
            doc.save(&mut writer)
                .map_err(|e| AppError::interne(format!("Écriture PDF : {e}")))?;
        }
        Ok(buf)
    }

    fn render_cession_bytes(db: &DbPool, devis_id: &str, demo: bool) -> AppResult<Vec<u8>> {
        let devis = InvoicesService::get(db, devis_id)?;
        let contact = ContactsService::get(db, &devis.contact_id)?;
        let settings = SettingsService::load(db)?;
        let primary = parse_hex_rgb(&settings.pdf_primary_color);
        let number = devis.number.as_deref().unwrap_or("BROUILLON");
        let today = format_date_fr(Utc::now().date_naive());

        let (doc, page1, layer1) =
            PdfDocument::new("Contrat de cession", Mm(210.0), Mm(297.0), "Contenu");
        let layer = doc.get_page(page1).get_layer(layer1);
        let font = doc
            .add_builtin_font(BuiltinFont::Helvetica)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;

        if demo {
            write_line(
                &layer,
                &font_bold,
                10.0,
                40.0,
                288.0,
                "DÉMONSTRATION — SANS VALEUR JURIDIQUE",
            );
        }

        let _ = place_logo(&layer, &settings);
        draw_filled_rect_rgb(&layer, 18.0, 268.0, 174.0, 12.0, primary, 0.15);
        write_line(
            &layer,
            &font_bold,
            14.0,
            22.0,
            272.0,
            "CONTRAT DE CESSION",
        );

        let mut y = 255.0;
        write_line(
            &layer,
            &font,
            9.0,
            18.0,
            y,
            &format!("Établi le {today} à partir du devis {number}"),
        );
        y -= 10.0;

        write_line(&layer, &font_bold, 10.0, 18.0, y, "Entre les soussignés :");
        y -= 6.0;
        write_line(
            &layer,
            &font,
            9.0,
            18.0,
            y,
            &format!(
                "Le cédant : {} — SIRET {}",
                settings.theatre_name, settings.theatre_siret
            ),
        );
        y -= 4.5;
        for line in settings.theatre_address.lines().take(2) {
            write_line(&layer, &font, 8.5, 22.0, y, line);
            y -= 4.0;
        }
        y -= 4.0;
        write_line(
            &layer,
            &font,
            9.0,
            18.0,
            y,
            &format!("Le cessionnaire : {}", contact.name),
        );
        y -= 4.5;
        if let Some(a) = &contact.address_line1 {
            write_line(&layer, &font, 8.5, 22.0, y, a);
            y -= 4.0;
        }
        if let Some(cp) = &contact.postal_code {
            write_line(
                &layer,
                &font,
                8.5,
                22.0,
                y,
                &format!("{cp} {}", contact.city.as_deref().unwrap_or("")),
            );
            y -= 4.0;
        }
        y -= 8.0;

        write_line(&layer, &font_bold, 10.0, 18.0, y, "Article 1 — Objet");
        y -= 5.0;
        let objet = devis
            .customer_notes
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("Prestation artistique / cession de droits selon devis accepté.");
        for chunk in wrap_text(objet, 95) {
            write_line(&layer, &font, 8.5, 18.0, y, &chunk);
            y -= 4.2;
        }
        y -= 4.0;

        write_line(&layer, &font_bold, 10.0, 18.0, y, "Article 2 — Conditions financières");
        y -= 5.0;
        write_line(
            &layer,
            &font,
            8.5,
            18.0,
            y,
            &format!(
                "Montant HT : {} € — TVA : {} € — TTC : {} €",
                format_amount(devis.total_ht),
                format_amount(devis.total_tva),
                format_amount(devis.total_ttc)
            ),
        );
        y -= 6.0;

        write_line(&layer, &font_bold, 10.0, 18.0, y, "Article 3 — Détail des prestations");
        y -= 5.0;
        for line in &devis.lines {
            write_line(
                &layer,
                &font,
                8.0,
                18.0,
                y,
                &format!(
                    "• {} — {} € HT (TVA {} %)",
                    truncate(&line.description, 55),
                    format_amount(line.line_ht),
                    format_amount(line.tax_rate_percent)
                ),
            );
            y -= 4.2;
            if y < 60.0 {
                break;
            }
        }
        y -= 6.0;

        write_line(&layer, &font_bold, 10.0, 18.0, y, "Article 4 — Acceptation");
        y -= 5.0;
        write_line(
            &layer,
            &font,
            8.5,
            18.0,
            y,
            "Le présent contrat est établi suite à l'acceptation du devis par le cessionnaire.",
        );
        y -= 10.0;

        write_line(&layer, &font, 8.5, 18.0, y, "Fait en deux exemplaires.");
        y -= 16.0;
        write_line(&layer, &font_bold, 9.0, 30.0, y, "Le cédant");
        write_line(&layer, &font_bold, 9.0, 120.0, y, "Le cessionnaire");
        y -= 20.0;
        write_line(&layer, &font, 8.0, 30.0, y, "________________");
        write_line(&layer, &font, 8.0, 120.0, y, "________________");

        write_line(&layer, &font, 6.5, 18.0, 18.0, DISCLAIMER_E_FACTURATION);
        if demo {
            write_line(
                &layer,
                &font_bold,
                8.0,
                18.0,
                12.0,
                "DÉMONSTRATION — SANS VALEUR JURIDIQUE — Nexyros",
            );
        }

        let mut buf = Vec::new();
        {
            let mut writer = BufWriter::new(Cursor::new(&mut buf));
            doc.save(&mut writer)
                .map_err(|e| AppError::interne(format!("Écriture PDF : {e}")))?;
        }
        Ok(buf)
    }
}

fn wrap_text(s: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.len() + 1 + word.len() <= max {
            cur.push(' ');
            cur.push_str(word);
        } else {
            out.push(cur);
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn format_date_fr(d: NaiveDate) -> String {
    const MONTHS: [&str; 12] = [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ];
    format!(
        "{} {} {}",
        d.day(),
        MONTHS[d.month0() as usize],
        d.year()
    )
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{t}…")
    }
}

fn write_line(
    layer: &PdfLayerReference,
    font: &printpdf::IndirectFontRef,
    size: f32,
    x: f32,
    y: f32,
    text: &str,
) {
    layer.use_text(text, size, Mm(x), Mm(y), font);
}

/// IBAN / BIC / banque / titulaire sous l'en-tête émetteur. Renvoie le y suivant.
fn write_bank_header_lines(
    layer: &PdfLayerReference,
    font: &printpdf::IndirectFontRef,
    settings: &AppSettings,
    mut y: f32,
    step: f32,
) -> f32 {
    if !settings.bank_iban.is_empty() {
        write_line(
            layer,
            font,
            8.5,
            18.0,
            y,
            &format!("IBAN : {}", settings.bank_iban),
        );
        y -= step;
    }
    if !settings.bank_bic.is_empty() {
        write_line(
            layer,
            font,
            8.5,
            18.0,
            y,
            &format!("BIC : {}", settings.bank_bic),
        );
        y -= step;
    }
    if !settings.bank_name.is_empty() {
        write_line(
            layer,
            font,
            8.5,
            18.0,
            y,
            &format!("Banque : {}", settings.bank_name),
        );
        y -= step;
    }
    if !settings.bank_holder.is_empty() {
        write_line(
            layer,
            font,
            8.5,
            18.0,
            y,
            &format!("Titulaire : {}", settings.bank_holder),
        );
        y -= step;
    } else if !settings.bank_account.is_empty() {
        write_line(
            layer,
            font,
            8.5,
            18.0,
            y,
            &format!("Compte bancaire N° : {}", settings.bank_account),
        );
        y -= step;
    }
    y
}

fn write_unpaid_virement_block(
    layer: &PdfLayerReference,
    font: &printpdf::IndirectFontRef,
    font_bold: &printpdf::IndirectFontRef,
    settings: &AppSettings,
    reste: rust_decimal::Decimal,
    primary: (f32, f32, f32),
    compact: bool,
) {
    let box_y = if compact { 34.0 } else { 32.0 };
    let box_h = if compact { 22.0 } else { 28.0 };
    draw_filled_rect_rgb(layer, 18.0, box_y, 174.0, box_h, primary, 0.14);
    draw_rect_stroke_rgb(layer, 18.0, box_y, 174.0, box_h, primary);

    let mut y = box_y + box_h - 5.0;
    write_line(
        layer,
        font_bold,
        if compact { 8.5 } else { 9.5 },
        22.0,
        y,
        &format!(
            "À régler par virement — reste dû : {} €",
            format_amount(reste)
        ),
    );
    y -= if compact { 4.0 } else { 5.0 };

    let holder = if !settings.bank_holder.is_empty() {
        settings.bank_holder.as_str()
    } else {
        settings.theatre_name.as_str()
    };
    let mut parts: Vec<String> = Vec::new();
    if !settings.bank_iban.is_empty() {
        parts.push(format!("IBAN {}", settings.bank_iban));
    }
    if !settings.bank_bic.is_empty() {
        parts.push(format!("BIC {}", settings.bank_bic));
    }
    if !settings.bank_name.is_empty() {
        parts.push(settings.bank_name.clone());
    }
    parts.push(format!("Titulaire : {holder}"));

    write_line(
        layer,
        font,
        if compact { 7.0 } else { 7.5 },
        22.0,
        y,
        &parts.join("  ·  "),
    );
    if !compact {
        y -= 4.5;
        write_line(
            layer,
            font,
            7.0,
            22.0,
            y,
            "Merci d'indiquer le numéro de facture en libellé du virement.",
        );
    }
}

fn parse_hex_rgb(hex: &str) -> (f32, f32, f32) {
    let h = hex.trim().trim_start_matches('#');
    if h.len() >= 6 {
        let r = u8::from_str_radix(&h[0..2], 16).unwrap_or(0x8B) as f32 / 255.0;
        let g = u8::from_str_radix(&h[2..4], 16).unwrap_or(0x1E) as f32 / 255.0;
        let b = u8::from_str_radix(&h[4..6], 16).unwrap_or(0x1E) as f32 / 255.0;
        (r, g, b)
    } else {
        (0.545, 0.118, 0.118)
    }
}

fn draw_filled_rect_rgb(
    layer: &PdfLayerReference,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    primary: (f32, f32, f32),
    mix: f32,
) {
    let grey = 0.92;
    let r = primary.0 * mix + grey * (1.0 - mix);
    let g = primary.1 * mix + grey * (1.0 - mix);
    let b = primary.2 * mix + grey * (1.0 - mix);
    layer.set_fill_color(Color::Rgb(Rgb::new(r, g, b, None)));
    layer.set_outline_color(Color::Rgb(Rgb::new(
        primary.0 * 0.5 + 0.4,
        primary.1 * 0.5 + 0.4,
        primary.2 * 0.5 + 0.4,
        None,
    )));
    layer.set_outline_thickness(0.4);
    let rect = Rect::new(Mm(x), Mm(y), Mm(x + w), Mm(y + h))
        .with_mode(PaintMode::FillStroke)
        .with_winding(WindingOrder::NonZero);
    layer.add_rect(rect);
}

fn draw_rect_stroke_rgb(
    layer: &PdfLayerReference,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    primary: (f32, f32, f32),
) {
    layer.set_outline_color(Color::Rgb(Rgb::new(primary.0, primary.1, primary.2, None)));
    layer.set_outline_thickness(0.9);
    let rect = Rect::new(Mm(x), Mm(y), Mm(x + w), Mm(y + h))
        .with_mode(PaintMode::Stroke)
        .with_winding(WindingOrder::NonZero);
    layer.add_rect(rect);
}

fn place_logo(layer: &PdfLayerReference, settings: &AppSettings) -> AppResult<()> {
    let bytes = resolve_logo_bytes(settings)?;
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| AppError::interne(format!("Logo : {e}")))?;
    let dyn_img = reader
        .decode()
        .map_err(|e| AppError::interne(format!("Décodage logo : {e}")))?;
    let image = Image::from_dynamic_image(&dyn_img);
    image.add_to_layer(
        layer.clone(),
        ImageTransform {
            translate_x: Some(Mm(148.0)),
            translate_y: Some(Mm(262.0)),
            dpi: Some(420.0),
            ..Default::default()
        },
    );
    Ok(())
}

fn resolve_logo_bytes(settings: &AppSettings) -> AppResult<Vec<u8>> {
    if settings.pdf_use_custom_logo
        && !settings.logo_path.is_empty()
        && !settings.logo_path.starts_with("brand:")
    {
        let path = PathBuf::from(&settings.logo_path);
        crate::paths::assert_path_inside_active_root(&path)?;
        return std::fs::read(&path)
            .map_err(|e| AppError::interne(format!("Lecture logo personnalisé : {e}")));
    }
    Ok(BRAND_LOGO_JPEG.to_vec())
}

#[allow(dead_code)]
fn save_pdf(doc: PdfDocumentReference, path: &PathBuf) -> AppResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)
        .map_err(|e| AppError::interne(format!("Écriture PDF : {e}")))?;
    Ok(())
}
