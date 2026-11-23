use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

use std::io::Cursor;

use chrono::Utc;
use printpdf::{
    image_crate::io::Reader as ImageReader, BuiltinFont, Image, ImageTransform, Mm, PdfDocument,
    PdfDocumentReference, PdfLayerReference,
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
        let invoice = InvoicesService::get(db, invoice_id)?;
        if invoice.number.is_none() {
            return Err(AppError::interdit(
                "Seules les factures émises ou avoirs émis peuvent être exportées en PDF.",
            ));
        }
        let contact = ContactsService::get(db, &invoice.contact_id)?;
        let settings = SettingsService::load(db)?;

        let title = match invoice.invoice_kind {
            InvoiceKind::Avoir => "AVOIR",
            InvoiceKind::Facture => "FACTURE",
        };

        let out_dir = attachments_dir()?.join("pdf");
        std::fs::create_dir_all(&out_dir)?;
        let file_name = format!(
            "{}_{}.pdf",
            invoice.number.as_deref().unwrap_or("sans_numero"),
            Utc::now().format("%Y%m%d")
        );
        let path = out_dir.join(file_name);

        let (doc, page1, layer1) =
            PdfDocument::new(title, Mm(210.0), Mm(297.0), "Contenu");
        let current_layer = doc.get_page(page1).get_layer(layer1);
        let font = doc.add_builtin_font(BuiltinFont::Helvetica).map_err(|e| {
            AppError::interne(format!("Police PDF : {e}"))
        })?;
        let font_bold = doc
            .add_builtin_font(BuiltinFont::HelveticaBold)
            .map_err(|e| AppError::interne(format!("Police PDF : {e}")))?;

        if demo {
            write_line(
                &current_layer,
                &font_bold,
                14.0,
                28.0,
                160.0,
                "DEMONSTRATION - SANS VALEUR COMPTABLE",
            );
        }

        // Logo TLC en haut à droite (fichier brand officiel).
        let _ = place_brand_logo(&current_layer);

        let mut y = 280.0;
        write_line(&current_layer, &font_bold, 16.0, 20.0, y, &settings.theatre_name);
        y -= 8.0;
        for line in settings.theatre_address.lines().take(4) {
            write_line(&current_layer, &font, 10.0, 20.0, y, line);
            y -= 5.0;
        }
        if !settings.theatre_siret.is_empty() {
            write_line(
                &current_layer,
                &font,
                9.0,
                20.0,
                y,
                &format!("SIRET : {}", settings.theatre_siret),
            );
            y -= 5.0;
        }
        if !settings.theatre_vat_number.is_empty() {
            write_line(
                &current_layer,
                &font,
                9.0,
                20.0,
                y,
                &format!("N° TVA : {}", settings.theatre_vat_number),
            );
            let _ = y;
        }

        write_line(
            &current_layer,
            &font_bold,
            14.0,
            120.0,
            270.0,
            &format!("{title} N° {}", invoice.number.as_deref().unwrap_or("—")),
        );
        if let Some(d) = invoice.issue_date {
            write_line(
                &current_layer,
                &font,
                10.0,
                120.0,
                262.0,
                &format!("Date : {}", d.format("%d/%m/%Y")),
            );
        }
        if let Some(d) = invoice.due_date {
            write_line(
                &current_layer,
                &font,
                10.0,
                120.0,
                256.0,
                &format!("Échéance : {}", d.format("%d/%m/%Y")),
            );
        }

        y = 230.0;
        write_line(&current_layer, &font_bold, 11.0, 20.0, y, "Client");
        y -= 6.0;
        write_line(&current_layer, &font, 10.0, 20.0, y, &contact.name);
        y -= 5.0;
        if let Some(a) = &contact.address_line1 {
            write_line(&current_layer, &font, 10.0, 20.0, y, a);
            y -= 5.0;
        }
        if let Some(cp) = &contact.postal_code {
            let city = contact.city.as_deref().unwrap_or("");
            write_line(
                &current_layer,
                &font,
                10.0,
                20.0,
                y,
                &format!("{cp} {city}"),
            );
            y -= 8.0;
        }

        y -= 4.0;
        write_line(
            &current_layer,
            &font_bold,
            10.0,
            20.0,
            y,
            "Désignation — Qté — PU HT — TVA % — Total TTC",
        );
        y -= 6.0;
        for line in &invoice.lines {
            let row = format!(
                "{} — {} — {} € — {} % — {} €",
                line.description,
                format_amount(line.quantity),
                format_amount(line.unit_price_ht),
                format_amount(line.tax_rate_percent),
                format_amount(line.line_ttc),
            );
            write_line(&current_layer, &font, 9.0, 20.0, y, &row);
            y -= 5.0;
            if y < 80.0 {
                break;
            }
        }

        y = 70.0;
        write_line(
            &current_layer,
            &font_bold,
            11.0,
            20.0,
            y,
            &format!("Total HT : {} €", format_amount(invoice.total_ht)),
        );
        y -= 6.0;
        write_line(
            &current_layer,
            &font,
            10.0,
            20.0,
            y,
            &format!("Total TVA : {} €", format_amount(invoice.total_tva)),
        );
        y -= 6.0;
        write_line(
            &current_layer,
            &font_bold,
            11.0,
            20.0,
            y,
            &format!("Total TTC : {} €", format_amount(invoice.total_ttc)),
        );

        y = 45.0;
        write_line(
            &current_layer,
            &font,
            8.0,
            20.0,
            y,
            &settings.legal_mentions,
        );
        y -= 10.0;
        write_line(
            &current_layer,
            &font,
            7.5,
            20.0,
            y,
            DISCLAIMER_E_FACTURATION,
        );
        y -= 5.0;
        write_line(
            &current_layer,
            &font,
            7.0,
            20.0,
            y,
            "Pénalités de retard (taux BCE + 10 pts). Indemnité forfaitaire de recouvrement : 40 € (B2B).",
        );

        if demo {
            y -= 8.0;
            write_line(
                &current_layer,
                &font_bold,
                9.0,
                20.0,
                y,
                "DÉMONSTRATION — SANS VALEUR COMPTABLE — Nexyros",
            );
        }

        save_pdf(doc, &path)?;
        Ok(path)
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

fn place_brand_logo(layer: &PdfLayerReference) -> AppResult<()> {
    let reader = ImageReader::new(Cursor::new(BRAND_LOGO_JPEG))
        .with_guessed_format()
        .map_err(|e| AppError::interne(format!("Logo JPEG : {e}")))?;
    let dyn_img = reader
        .decode()
        .map_err(|e| AppError::interne(format!("Décodage logo : {e}")))?;
    let image = Image::from_dynamic_image(&dyn_img);
    // En-tête droite (~50 mm de large sur A4).
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

fn save_pdf(doc: PdfDocumentReference, path: &PathBuf) -> AppResult<()> {
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    doc.save(&mut writer)
        .map_err(|e| AppError::interne(format!("Écriture PDF : {e}")))?;
    Ok(())
}