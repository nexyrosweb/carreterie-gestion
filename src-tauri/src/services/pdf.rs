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
use crate::models::invoice::InvoiceKind;
use crate::money::format_amount;
use crate::paths::attachments_dir;
use crate::services::contacts::ContactsService;
use crate::services::invoices::InvoicesService;
use crate::services::settings::SettingsService;
use crate::DISCLAIMER_E_FACTURATION;

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

    fn render_pdf_bytes(db: &DbPool, invoice_id: &str, demo: bool) -> AppResult<Vec<u8>> {
        let invoice = InvoicesService::get(db, invoice_id)?;
        let contact = ContactsService::get(db, &invoice.contact_id)?;
        let settings = SettingsService::load(db)?;

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

        let _ = place_brand_logo(&layer);

        // Émetteur (haut gauche)
        let mut y = 278.0;
        write_line(&layer, &font_bold, 12.0, 18.0, y, &settings.theatre_name.to_uppercase());
        y -= 5.5;
        for line in settings.theatre_address.lines().take(3) {
            write_line(&layer, &font, 9.0, 18.0, y, &line.to_uppercase());
            y -= 4.5;
        }
        if !settings.theatre_siret.is_empty() {
            write_line(
                &layer,
                &font,
                8.5,
                18.0,
                y,
                &format!("Siret : {}", settings.theatre_siret),
            );
            y -= 4.5;
        }
        if !settings.bank_iban.is_empty() {
            write_line(
                &layer,
                &font,
                8.5,
                18.0,
                y,
                &format!("IBAN : {}", settings.bank_iban),
            );
            y -= 4.5;
        }
        if !settings.bank_account.is_empty() {
            write_line(
                &layer,
                &font,
                8.5,
                18.0,
                y,
                &format!("Compte bancaire N° : {}", settings.bank_account),
            );
        }

        // Bloc client (droite)
        draw_filled_rect(&layer, 118.0, 228.0, 74.0, 32.0, 0.90);
        let mut cy = 252.0;
        write_line(&layer, &font_bold, 10.0, 122.0, cy, &contact.name);
        cy -= 5.0;
        if let Some(a) = &contact.address_line1 {
            write_line(&layer, &font, 9.0, 122.0, cy, a);
            cy -= 4.5;
        }
        if let Some(cp) = &contact.postal_code {
            let city = contact.city.as_deref().unwrap_or("");
            write_line(&layer, &font, 9.0, 122.0, cy, &format!("{cp} {city}"));
            cy -= 4.5;
        }
        if let Some(siret) = &contact.siret {
            let siren = if siret.len() >= 9 { &siret[..9] } else { siret };
            write_line(&layer, &font, 9.0, 122.0, cy, &format!("SIREN : {siren}"));
        }

        // Titre centré dans un cadre
        draw_rect_stroke(&layer, 45.0, 205.0, 120.0, 14.0);
        draw_filled_rect(&layer, 45.5, 205.5, 119.0, 13.0, 0.96);
        let title = format!("{doc_label} N° {number_display} en date du {date_display}");
        write_line(&layer, &font_bold, 11.0, 52.0, 210.0, &title);

        // Bloc prestation + montants
        draw_filled_rect(&layer, 18.0, 145.0, 174.0, 48.0, 0.90);
        let prestation = invoice
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
            .unwrap_or_else(|| "Prestation :".to_string());
        write_line(&layer, &font_italic, 10.0, 22.0, 182.0, &prestation);

        let tax_rate = invoice
            .lines
            .first()
            .map(|l| format_amount(l.tax_rate_percent))
            .unwrap_or_else(|| "20.00".into());

        write_line(
            &layer,
            &font_bold,
            10.0,
            130.0,
            175.0,
            &format!("TTC : {}", format_amount(invoice.total_ttc)),
        );
        write_line(
            &layer,
            &font,
            9.5,
            130.0,
            168.0,
            &format!("Dont TVA {tax_rate} % : {}", format_amount(invoice.total_tva)),
        );
        write_line(
            &layer,
            &font,
            9.5,
            130.0,
            161.0,
            &format!("HT : {}", format_amount(invoice.total_ht)),
        );

        // Détail lignes sous le bloc
        let mut ly = 135.0;
        if invoice.lines.len() > 1 {
            write_line(&layer, &font_bold, 9.0, 18.0, ly, "Détail");
            ly -= 5.0;
            for line in &invoice.lines {
                write_line(
                    &layer,
                    &font,
                    8.5,
                    18.0,
                    ly,
                    &format!(
                        "{} — qté {} — {} € HT — TVA {} % — {} € TTC",
                        truncate(&line.description, 42),
                        format_amount(line.quantity),
                        format_amount(line.unit_price_ht),
                        format_amount(line.tax_rate_percent),
                        format_amount(line.line_ttc),
                    ),
                );
                ly -= 4.5;
                if ly < 70.0 {
                    break;
                }
            }
        }

        // Pied de page paiement / devis
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
                write_line(&layer, &font_bold, 9.0, 18.0, 52.0, "Montant réglé");
                if invoice.amount_paid > rust_decimal_macros::dec!(0) {
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
                } else {
                    write_line(
                        &layer,
                        &font,
                        8.5,
                        18.0,
                        46.0,
                        "En attente de règlement.",
                    );
                    if !settings.bank_iban.is_empty() {
                        write_line(
                            &layer,
                            &font,
                            8.0,
                            18.0,
                            40.0,
                            &format!("IBAN : {}", settings.bank_iban),
                        );
                    }
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

fn draw_filled_rect(layer: &PdfLayerReference, x: f32, y: f32, w: f32, h: f32, grey: f32) {
    layer.set_fill_color(Color::Rgb(Rgb::new(grey, grey, grey, None)));
    layer.set_outline_color(Color::Rgb(Rgb::new(0.75, 0.75, 0.75, None)));
    layer.set_outline_thickness(0.4);
    let rect = Rect::new(Mm(x), Mm(y), Mm(x + w), Mm(y + h))
        .with_mode(PaintMode::FillStroke)
        .with_winding(WindingOrder::NonZero);
    layer.add_rect(rect);
}

fn draw_rect_stroke(layer: &PdfLayerReference, x: f32, y: f32, w: f32, h: f32) {
    layer.set_outline_color(Color::Rgb(Rgb::new(0.1, 0.1, 0.1, None)));
    layer.set_outline_thickness(0.8);
    let rect = Rect::new(Mm(x), Mm(y), Mm(x + w), Mm(y + h))
        .with_mode(PaintMode::Stroke)
        .with_winding(WindingOrder::NonZero);
    layer.add_rect(rect);
}

fn place_brand_logo(layer: &PdfLayerReference) -> AppResult<()> {
    let reader = ImageReader::new(Cursor::new(BRAND_LOGO_JPEG))
        .with_guessed_format()
        .map_err(|e| AppError::interne(format!("Logo JPEG : {e}")))?;
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

#[allow(dead_code)]
fn save_pdf(doc: PdfDocumentReference, path: &PathBuf) -> AppResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)
        .map_err(|e| AppError::interne(format!("Écriture PDF : {e}")))?;
    Ok(())
}
