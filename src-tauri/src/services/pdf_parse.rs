use regex::Regex;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use unicode_normalization::UnicodeNormalization;

use crate::models::import::{ExtractedFields, FieldConfidence, ImportDocumentType};
use crate::money::round_eur;

#[derive(Debug, Clone)]
pub struct ParseResult {
    pub fields: ExtractedFields,
    pub document_type: ImportDocumentType,
    pub confidence: FieldConfidence,
    pub amounts_coherent: Option<bool>,
}

/// Découpe un PDF multi-documents en segments (1 facture / acompte / avoir chacun).
pub fn split_invoice_documents(text: &str) -> Vec<String> {
    let normalized = normalize_text(text);
    if normalized.trim().is_empty() {
        return vec![String::new()];
    }

    let mut pages = split_by_page_markers(&normalized);
    if pages.len() <= 1 {
        pages = split_by_form_feed(&normalized);
    }
    if pages.len() <= 1 {
        pages = split_by_document_headers(&normalized);
    }

    let mut segments = Vec::new();
    for page in pages {
        let trimmed = page.trim();
        if trimmed.is_empty() {
            continue;
        }
        let sub = split_by_document_headers(trimmed);
        if sub.len() > 1 {
            for s in sub {
                let t = s.trim();
                if !t.is_empty() && looks_like_document(t) {
                    segments.push(t.to_string());
                }
            }
        } else if looks_like_document(trimmed) || segments.is_empty() {
            segments.push(trimmed.to_string());
        } else {
            // Page sans en-tête : rattacher à la précédente si possible
            if let Some(last) = segments.last_mut() {
                last.push('\n');
                last.push_str(trimmed);
            } else {
                segments.push(trimmed.to_string());
            }
        }
    }

    if segments.is_empty() {
        vec![normalized]
    } else {
        segments
    }
}

fn looks_like_document(text: &str) -> bool {
    let upper = text.to_uppercase();
    upper.contains("FACTURE") || upper.contains("AVOIR") || upper.contains("ACOMPTE")
}

fn split_by_form_feed(text: &str) -> Vec<String> {
    if !text.contains('\u{000c}') {
        return vec![text.to_string()];
    }
    text.split('\u{000c}')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_by_page_markers(text: &str) -> Vec<String> {
    let re = Regex::new(r"(?m)^\s*--\s*\d+\s*(?:of|sur|/)\s*\d+\s*--\s*$").ok();
    let Some(re) = re else {
        return vec![text.to_string()];
    };
    if !re.is_match(text) {
        return vec![text.to_string()];
    }
    re.split(text)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_by_document_headers(text: &str) -> Vec<String> {
    let re = Regex::new(r"(?i)(?:Facture|Avoir)\s*N[°ºo]").ok();
    let Some(re) = re else {
        return vec![text.to_string()];
    };
    let header_positions: Vec<usize> = re.find_iter(text).map(|m| m.start()).collect();
    if header_positions.len() <= 1 {
        return vec![text.to_string()];
    }

    // Chaque document commence souvent à l'en-tête théâtre / bloc client juste avant « Facture|Avoir N° ».
    let theatre_re = Regex::new(r"(?i)101\s+rue\s+de\s+la\s+carreterie").ok();
    let mut starts: Vec<usize> = Vec::with_capacity(header_positions.len());
    for (i, &pos) in header_positions.iter().enumerate() {
        let floor = if i == 0 {
            0
        } else {
            starts[i - 1].saturating_add(1).max(header_positions[i - 1])
        };
        let start = if let Some(re) = &theatre_re {
            let window = &text[floor..pos];
            re.find_iter(window)
                .last()
                .map(|m| floor + m.start())
                .unwrap_or(pos)
        } else {
            pos
        };
        starts.push(start.min(pos));
    }

    let mut out = Vec::new();
    for i in 0..starts.len() {
        let end = if i + 1 < starts.len() {
            starts[i + 1]
        } else {
            text.len()
        };
        if end <= starts[i] {
            continue;
        }
        let seg = text[starts[i]..end].trim();
        if !seg.is_empty() {
            out.push(seg.to_string());
        }
    }
    if out.len() <= 1 {
        vec![text.to_string()]
    } else {
        out
    }
}

pub fn parse_french_invoice_text(text: &str) -> ParseResult {
    let normalized = normalize_text(text);
    let upper = normalized.to_uppercase();

    let document_type = detect_document_type(&upper);
    let mut fields = ExtractedFields::default();
    let mut uncertain = Vec::new();

    fields.party_name = find_party_name(&normalized);
    fields.party_address = find_party_address(&normalized);
    fields.issuer_name = find_issuer_name(&normalized);
    fields.description = find_description(&normalized);

    if let Some((siren, siret)) = find_party_identifiers(&normalized) {
        fields.siren = siren;
        fields.siret = siret;
    } else {
        if let Some(siret) = find_siret_before_invoice(&normalized) {
            fields.siret = Some(siret.clone());
            if siret.len() >= 9 {
                fields.siren = Some(siret[..9].to_string());
            }
        }
        if fields.siren.is_none() {
            if let Some(siren) = find_siren_before_invoice(&normalized) {
                fields.siren = Some(siren);
            }
        }
    }

    match find_invoice_numbers(&normalized) {
        NumberMatch::Single(n) => fields.invoice_number = Some(n),
        NumberMatch::Multiple(candidates) => {
            // Sur un segment déjà découpé, prendre le 1er n° document
            fields.invoice_number = candidates.first().cloned();
            if candidates.len() > 1 {
                FieldConfidence::mark_uncertain(&mut uncertain, "invoice_number");
            }
        }
        NumberMatch::None => {}
    }

    fields.issue_date = find_document_issue_date(&normalized);
    if fields.issue_date.is_none() {
        fields.issue_date =
            find_labeled_date(&normalized, &["DATE", "DATE DE FACTURE", "FACTURE DU"]);
    }
    if fields.issue_date.is_none() {
        fields.issue_date = find_first_date(&normalized);
    }

    fields.payment_date = find_payment_date(&normalized).or_else(|| {
        find_labeled_date(
            &normalized,
            &["DATE D'ÉCHÉANCE", "DATE ECHEANCE", "ÉCHÉANCE", "ECHEANCE"],
        )
    });
    fields.payment_method = find_payment_method(&normalized);
    if upper.contains("MONTANT RÉGLÉ") || upper.contains("MONTANT REGLE") {
        fields.payment_status = Some("payee".into());
    }

    fields.total_ht = find_amount_label(
        &normalized,
        &["TOTAL HT", "MONTANT HT", "NET HT", "HT"],
    );
    fields.total_tva = find_amount_label(&normalized, &["TOTAL TVA", "MONTANT TVA"]);
    fields.total_ttc = find_amount_label(
        &normalized,
        &["TOTAL TTC", "NET À PAYER", "NET A PAYER", "MONTANT TTC", "TTC"],
    );

    // Ligne isolée de TVA (souvent juste sous le TTC dans le modèle théâtre)
    if fields.total_tva.is_none() {
        fields.total_tva = find_orphan_tva_amount(&normalized);
    }

    if let Some(rate) = find_tax_rate(&normalized) {
        fields.tax_rate = Some(rate);
    }

    fields.related_invoice_ref = find_related_ref(&normalized);

    let amounts_coherent = check_amounts_coherence(
        fields.total_ht.as_deref(),
        fields.total_tva.as_deref(),
        fields.total_ttc.as_deref(),
    );

    let mut missing = Vec::new();
    FieldConfidence::required_missing(&mut missing, "invoice_number", &fields.invoice_number);
    FieldConfidence::required_missing(&mut missing, "issue_date", &fields.issue_date);
    FieldConfidence::required_missing(&mut missing, "total_ttc", &fields.total_ttc);

    if amounts_coherent == Some(false) {
        FieldConfidence::mark_uncertain(&mut uncertain, "total_ht");
        FieldConfidence::mark_uncertain(&mut uncertain, "total_tva");
        FieldConfidence::mark_uncertain(&mut uncertain, "total_ttc");
    }

    ParseResult {
        fields,
        document_type,
        confidence: FieldConfidence {
            uncertain_fields: uncertain,
            missing_fields: missing,
        },
        amounts_coherent,
    }
}

enum NumberMatch {
    None,
    Single(String),
    Multiple(Vec<String>),
}

fn normalize_text(text: &str) -> String {
    text.nfc().collect::<String>().replace('\u{00a0}', " ")
}

fn detect_document_type(upper: &str) -> ImportDocumentType {
    // Priorité : en-tête Avoir, puis acompte dans la prestation, puis Facture
    if Regex::new(r"(?i)\bAVOIR\s*N")
        .ok()
        .map(|re| re.is_match(upper))
        .unwrap_or(false)
        || (upper.contains("AVOIR")
            && !upper.contains("FACTURE N")
            && !upper.contains("FACTURE N°"))
    {
        ImportDocumentType::Avoir
    } else if upper.contains("ACOMPTE") {
        ImportDocumentType::Acompte
    } else if upper.contains("FACTURE") {
        ImportDocumentType::Facture
    } else {
        ImportDocumentType::Inconnu
    }
}

fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

fn find_party_name(text: &str) -> Option<String> {
    let patterns = [
        r"(?i)(?:•\s*)?Client\s*:\s*(.+)",
        r"(?i)^Client\s*:\s*(.+)$",
    ];
    for pat in patterns {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(text) {
                let name = cap[1].trim().trim_matches('•').trim();
                if name.len() >= 2 {
                    return Some(name.lines().next().unwrap_or(name).trim().to_string());
                }
            }
        }
    }
    // Nom société (bloc client) — y compris « Theatre Ronin »
    if let Ok(re) = Regex::new(
        r"(?im)^\s*((?:Compagnie|Association|Theatre|Théâtre|SARL|SAS|EURL|SCI)[^\n]{2,80})\s*$",
    ) {
        for cap in re.captures_iter(text) {
            let name = cap[1].trim().to_string();
            let start = cap.get(0).map(|m| m.start()).unwrap_or(0);
            let before = text[..start].to_uppercase();
            // Préférer un nom qui apparaît après le bloc paiement / avant Prestation
            if before.contains("AU PROFIT") || before.contains("FACTURE N") || before.contains("AVOIR N")
            {
                return Some(name);
            }
        }
        // Fallback : premier nom société trouvé hors en-tête théâtre
        for cap in re.captures_iter(text) {
            let name = cap[1].trim().to_string();
            if !name.to_uppercase().contains("CARRETERIE") {
                return Some(name);
            }
        }
    }
    None
}

fn find_party_address(text: &str) -> Option<String> {
    if let Ok(re) = Regex::new(r"(?i)(?:•\s*)?Adresse\s*:\s*(.+)") {
        if let Some(cap) = re.captures(text) {
            let a = cap[1].trim();
            if !a.is_empty() {
                return Some(a.lines().next().unwrap_or(a).trim().to_string());
            }
        }
    }
    None
}

fn find_issuer_name(text: &str) -> Option<String> {
    if let Ok(re) = Regex::new(r"(?i)Au profit de\s+([^\n.]+)") {
        if let Some(cap) = re.captures(text) {
            return Some(cap[1].trim().trim_end_matches('.').to_string());
        }
    }
    None
}

fn find_description(text: &str) -> Option<String> {
    for pat in [
        r"(?i)Prestation\s*:\s*(.+)",
        r"(?i)Spectacle\s*:\s*(.+)",
        r"(?i)Remboursement client\s*\(([^)]+)\)",
    ] {
        if let Ok(re) = Regex::new(pat) {
            if let Some(cap) = re.captures(text) {
                let d = cap[1].trim();
                if !d.is_empty() {
                    return Some(d.lines().next().unwrap_or(d).trim().to_string());
                }
            }
        }
    }
    None
}

fn find_party_identifiers(text: &str) -> Option<(Option<String>, Option<String>)> {
    // Identifiants proches du libellé Client
    if let Ok(re) = Regex::new(
        r"(?is)Client\s*:[^\n]{0,80}(?:.*?)(?:SIRET\s*[:N°\s]*([\d\s.]{14,20})|SIREN\s*[:\s]*([\d\s.]{9,14}))",
    ) {
        if let Some(cap) = re.captures(text) {
            let siret = cap.get(1).map(|m| digits_only(m.as_str())).filter(|d| d.len() == 14);
            let siren = cap
                .get(2)
                .map(|m| digits_only(m.as_str()))
                .filter(|d| d.len() == 9)
                .or_else(|| siret.as_ref().map(|s| s[..9].to_string()));
            return Some((siren, siret));
        }
    }
    None
}

fn text_before_invoice_header(text: &str) -> &str {
    let upper = text.to_uppercase();
    let markers = ["FACTURE N", "AVOIR N", "FACTURE N°", "AVOIR N°"];
    let mut cut = text.len();
    for m in markers {
        if let Some(pos) = upper.find(m) {
            cut = cut.min(pos);
        }
    }
    &text[..cut]
}

fn find_siret_before_invoice(text: &str) -> Option<String> {
    find_siret(text_before_invoice_header(text)).or_else(|| {
        // Ne pas prendre le SIRET « Au profit de » (émetteur)
        let without_issuer = if let Some(pos) = text.to_uppercase().find("AU PROFIT") {
            &text[..pos]
        } else {
            text
        };
        find_siret(without_issuer)
    })
}

fn find_siren_before_invoice(text: &str) -> Option<String> {
    find_siren(text_before_invoice_header(text))
}

fn find_siret(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)SIRET\s*(?:N[°ºo])?\s*[:\s]?\s*(\d[\d\s\.]{12,20}\d)").ok()?;
    if let Some(cap) = re.captures(text) {
        let d = digits_only(&cap[1]);
        if d.len() == 14 {
            return Some(d);
        }
    }
    let re2 = Regex::new(r"\b(\d{3}\s?\d{3}\s?\d{3}\s?\d{5})\b").ok()?;
    for cap in re2.captures_iter(text) {
        let d = digits_only(&cap[1]);
        if d.len() == 14 {
            return Some(d);
        }
    }
    None
}

fn find_siren(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)SIREN\s*[:\s]?\s*(\d[\d\s\.]{7,14})").ok()?;
    if let Some(cap) = re.captures(text) {
        let d = digits_only(&cap[1]);
        if d.len() == 9 {
            return Some(d);
        }
    }
    None
}

fn find_invoice_numbers(text: &str) -> NumberMatch {
    let patterns = [
        r"(?i)(?:FACTURE|AVOIR)\s*N[°ºo]?\s*[:.]?\s*([A-Za-z0-9][A-Za-z0-9\-/\.]{0,30})",
        r"(?i)\b(FA|AV|DV)-[\w\-/]+",
        r"(?i)\bFA[\-/]?\d{4}[\-/]?\d+",
        r"(?i)\bAV[\-/]?\d{4}[\-/]?\d+",
    ];
    let mut found = Vec::new();
    for pat in patterns {
        if let Ok(re) = Regex::new(pat) {
            for cap in re.captures_iter(text) {
                let val = cap
                    .get(1)
                    .map(|m| m.as_str().trim().to_string())
                    .or_else(|| cap.get(0).map(|m| m.as_str().trim().to_string()));
                if let Some(v) = val {
                    // Éviter de capturer « date » après N°
                    let clean = v
                        .split_whitespace()
                        .next()
                        .unwrap_or(&v)
                        .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '/')
                        .to_string();
                    if clean.len() >= 1
                        && !clean.eq_ignore_ascii_case("date")
                        && !found.contains(&clean)
                    {
                        found.push(clean);
                    }
                }
            }
        }
    }
    match found.len() {
        0 => NumberMatch::None,
        1 => NumberMatch::Single(found.into_iter().next().unwrap()),
        _ => NumberMatch::Multiple(found),
    }
}

fn find_document_issue_date(text: &str) -> Option<String> {
    // « Facture N° 1 en date du 15 novembre 2025 » / « Avoir N°A1 date du 13 novembre 2026 »
    let re = Regex::new(
        r"(?i)(?:FACTURE|AVOIR)\s*N[°ºo]?\s*[A-Za-z0-9\-/.]*\s*(?:en\s+)?date\s+du\s+(\d{1,2}\s+[a-zéûôù]+\s+\d{4})",
    )
    .ok()?;
    if let Some(cap) = re.captures(text) {
        return parse_french_long_date(cap[1].trim());
    }
    None
}

fn parse_french_long_date(s: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(\d{1,2})\s+([a-zéûôù]+)\s+(\d{4})").ok()?;
    let cap = re.captures(s)?;
    let day: u32 = cap[1].parse().ok()?;
    let year: i32 = cap[3].parse().ok()?;
    let month = match cap[2].to_lowercase().as_str() {
        "janvier" => 1,
        "février" | "fevrier" => 2,
        "mars" => 3,
        "avril" => 4,
        "mai" => 5,
        "juin" => 6,
        "juillet" => 7,
        "août" | "aout" => 8,
        "septembre" => 9,
        "octobre" => 10,
        "novembre" => 11,
        "décembre" | "decembre" => 12,
        _ => return None,
    };
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn find_payment_date(text: &str) -> Option<String> {
    let re = Regex::new(
        r"(?i)par\s+virement(?:\s+bancaire)?\s+au\s+(\d{1,2}[/.]\d{1,2}[/.]\d{2,4})",
    )
    .ok()?;
    if let Some(cap) = re.captures(text) {
        return normalize_short_date(&cap[1]);
    }
    None
}

fn normalize_short_date(s: &str) -> Option<String> {
    let re = Regex::new(r"(\d{1,2})[/.](\d{1,2})[/.](\d{2,4})").ok()?;
    let cap = re.captures(s)?;
    let day: u32 = cap[1].parse().ok()?;
    let month: u32 = cap[2].parse().ok()?;
    let mut year: i32 = cap[3].parse().ok()?;
    if year < 100 {
        year += 2000;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn find_payment_method(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    if upper.contains("VIREMENT") {
        Some("virement".into())
    } else if upper.contains("CHÈQUE") || upper.contains("CHEQUE") {
        Some("cheque".into())
    } else if upper.contains("ESPÈCE") || upper.contains("ESPECE") {
        Some("especes".into())
    } else if upper.contains("CB") || upper.contains("CARTE") {
        Some("carte".into())
    } else {
        None
    }
}

fn find_labeled_date(text: &str, labels: &[&str]) -> Option<String> {
    let upper = text.to_uppercase();
    for label in labels {
        if let Some(pos) = upper.find(label) {
            let slice = &text[pos..text.len().min(pos + 80)];
            if let Some(d) = find_first_date(slice) {
                return Some(d);
            }
            if let Some(d) = parse_french_long_date(slice) {
                return Some(d);
            }
        }
    }
    None
}

fn find_first_date(text: &str) -> Option<String> {
    let re_slash = Regex::new(r"\b(\d{1,2})[/.](\d{1,2})[/.](\d{4})\b").ok()?;
    if let Some(cap) = re_slash.captures(text) {
        return Some(format!(
            "{:04}-{:02}-{:02}",
            &cap[3],
            cap[2].parse::<u32>().unwrap_or(1),
            cap[1].parse::<u32>().unwrap_or(1)
        ));
    }
    let re_iso = Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").ok()?;
    if let Some(cap) = re_iso.captures(text) {
        return Some(cap[0].to_string());
    }
    parse_french_long_date(text)
}

fn parse_french_amount(s: &str) -> Option<Decimal> {
    let cleaned = s
        .trim()
        .replace('\u{00a0}', "")
        .replace(' ', "")
        .replace(',', ".");
    let cleaned = cleaned.trim_end_matches(|c: char| !c.is_ascii_digit() && c != '.');
    cleaned.parse::<Decimal>().ok().map(round_eur)
}

fn find_amount_label(text: &str, labels: &[&str]) -> Option<String> {
    let amount_re = Regex::new(r"(?i)(\d[\d\s]*(?:[.,]\d{2})?)\s*€?").ok()?;
    for label in labels {
        // Préférer label en début de ligne / mot entier pour éviter « Dont TVA » sur « TVA »
        let pat = format!(r"(?im)(?:^|\b){}\s*[:\s]*(\d[\d\s]*(?:[.,]\d{{2}})?)", regex::escape(label));
        if let Ok(re) = Regex::new(&pat) {
            if let Some(cap) = re.captures(text) {
                if let Some(d) = parse_french_amount(&cap[1]) {
                    return Some(format!("{d:.2}"));
                }
            }
        }
        // Fallback : recherche simple
        let upper = text.to_uppercase();
        let lab = label.to_uppercase();
        if let Some(pos) = upper.find(&lab) {
            let slice = &text[pos..text.len().min(pos + 40)];
            if let Some(cap) = amount_re.captures(slice) {
                if let Some(d) = parse_french_amount(&cap[1]) {
                    return Some(format!("{d:.2}"));
                }
            }
        }
    }
    None
}

fn find_orphan_tva_amount(text: &str) -> Option<String> {
    // Après une ligne TTC, une ligne numérique seule est souvent la TVA
    let re = Regex::new(r"(?im)^\s*TTC\s+[^\n]+\n\s*(\d[\d\s]*(?:[.,]\d{2})?)\s*$").ok()?;
    if let Some(cap) = re.captures(text) {
        if let Some(d) = parse_french_amount(&cap[1]) {
            return Some(format!("{d:.2}"));
        }
    }
    None
}

fn find_tax_rate(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(?:Dont\s+)?TVA\s*(?:à|a)?\s*(\d{1,2}(?:[.,]\d+)?)\s*%").ok()?;
    if let Some(cap) = re.captures(text) {
        let rate = cap[1].replace(',', ".");
        return Some(rate);
    }
    None
}

fn find_related_ref(text: &str) -> Option<String> {
    let re = Regex::new(
        r"(?i)(?:facture\s+d['']origine|facture\s+liée|ref\.?|réf\.?)\s*N[°ºo]?\s*[:.]?\s*([A-Za-z0-9][A-Za-z0-9\-/\.]{1,30})",
    )
    .ok()?;
    re.captures(text).map(|c| c[1].trim().to_string())
}

/// Vérifie HT + TVA ≈ TTC (tolérance 0,05 €).
pub fn check_amounts_coherence(ht: Option<&str>, tva: Option<&str>, ttc: Option<&str>) -> Option<bool> {
    let ht = ht.and_then(|s| parse_french_amount(s))?;
    let tva = tva.and_then(|s| parse_french_amount(s))?;
    let ttc = ttc.and_then(|s| parse_french_amount(s))?;
    let sum = ht + tva;
    let diff = (sum - ttc).abs();
    Some(diff <= dec!(0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
SOCIETE EXEMPLE SARL
SIRET : 123 456 789 00012
FACTURE N° FA-2024-0042
Date : 15/03/2024
Prestation audiovisuelle

Total HT 1 200,00 €
TVA à 20 %
Total TVA 240,00 €
Total TTC 1 440,00 €
"#;

    const MULTI_CARRETERIE: &str = r#"
101 rue de la carreterie – Avignon
• Adresse : 25 rue Pierre Curie, 78670 Médan
• SIREN : 933 697 930
Facture N° 1 en date du 15 novembre 2025
TTC 2200,00
366,67
HT 1833,33
Montant réglé
2200 € par virement bancaire au 15/11/25
Au profit de LA BELLE SCÈNE PRODUCTIONS.
Siret : 914 131 222 R.C.S. Paris
• Client : Compagnie KIZUNA
Prestation : Location salle , juillet 2026 (1er acompte)
Dont TVA 20 %

-- 1 of 11 --

101 rue de la carreterie – Avignon
Facture N° 2 en date du 27 novembre 2025
TTC 1920,00
320,00
HT 1600,00
Theatre Ronin
Prestation : Location salle , juillet 2026 (1er acompte)
Dont TVA 20 %

-- 2 of 11 --

101 rue de la carreterie – Avignon
Avoir N°A1 date du 13 novembre 2026
Remboursement client (doublon)
TTC 22,00
0,45
HT 21,55
Client : Pedro Martinez Mora
Spectacle : Le petit prince
Dont TVA 2,1 %
"#;

    #[test]
    fn parse_sample_invoice() {
        let r = parse_french_invoice_text(SAMPLE);
        assert_eq!(r.document_type, ImportDocumentType::Facture);
        assert_eq!(
            r.fields.invoice_number.as_deref(),
            Some("FA-2024-0042")
        );
        assert_eq!(r.fields.siret.as_deref(), Some("12345678900012"));
        assert_eq!(r.fields.issue_date.as_deref(), Some("2024-03-15"));
        assert_eq!(r.fields.total_ht.as_deref(), Some("1200.00"));
        assert_eq!(r.fields.total_tva.as_deref(), Some("240.00"));
        assert_eq!(r.fields.total_ttc.as_deref(), Some("1440.00"));
        assert_eq!(r.amounts_coherent, Some(true));
    }

    #[test]
    fn detect_avoir() {
        let r = parse_french_invoice_text("AVOIR N° AV-2024-0001\nTotal TTC 100,00");
        assert_eq!(r.document_type, ImportDocumentType::Avoir);
    }

    #[test]
    fn split_multi_document_pdf() {
        let parts = split_invoice_documents(MULTI_CARRETERIE);
        assert!(parts.len() >= 3, "got {}", parts.len());
        let types: Vec<_> = parts
            .iter()
            .map(|p| parse_french_invoice_text(p).document_type)
            .collect();
        assert_eq!(types[0], ImportDocumentType::Acompte);
        assert_eq!(types[1], ImportDocumentType::Acompte);
        assert_eq!(types[2], ImportDocumentType::Avoir);
    }

    #[test]
    fn parse_carreterie_acompte() {
        let parts = split_invoice_documents(MULTI_CARRETERIE);
        let r = parse_french_invoice_text(&parts[0]);
        assert_eq!(r.document_type, ImportDocumentType::Acompte);
        assert_eq!(r.fields.invoice_number.as_deref(), Some("1"));
        assert_eq!(r.fields.issue_date.as_deref(), Some("2025-11-15"));
        assert_eq!(r.fields.party_name.as_deref(), Some("Compagnie KIZUNA"));
        assert_eq!(r.fields.siren.as_deref(), Some("933697930"));
        assert_eq!(r.fields.total_ttc.as_deref(), Some("2200.00"));
        assert_eq!(r.fields.total_ht.as_deref(), Some("1833.33"));
        assert_eq!(r.fields.tax_rate.as_deref(), Some("20"));
        assert_eq!(r.fields.payment_status.as_deref(), Some("payee"));
    }

    #[test]
    fn parse_carreterie_avoir() {
        let parts = split_invoice_documents(MULTI_CARRETERIE);
        let avoir = parts
            .iter()
            .map(|p| parse_french_invoice_text(p))
            .find(|r| r.document_type == ImportDocumentType::Avoir)
            .expect("avoir");
        assert_eq!(avoir.fields.invoice_number.as_deref(), Some("A1"));
        assert_eq!(
            avoir.fields.party_name.as_deref(),
            Some("Pedro Martinez Mora")
        );
        assert_eq!(avoir.fields.total_ttc.as_deref(), Some("22.00"));
    }

    /// Texte collé comme `pdf-extract` (sans form-feed ni `-- N of M --`).
    #[test]
    fn split_real_extract_style_without_page_breaks() {
        let text = r#"
101 rue de la carreterie – Avignon
• Adresse : 25 rue Pierre Curie, 78670 Médan
• SIREN : 933 697 930
Facture N° 1 en date du 15 novembre 2025
TTC 2200,00
HT 1833,33
• Client : Compagnie KIZUNA
Prestation : Location salle (1er acompte)

101 rue de la carreterie – Avignon
Facture N° 2 en date du 27 novembre 2025
TTC 1920,00
HT 1600,00
Theatre Ronin
Prestation : Location salle (1er acompte)

101 rue de la carreterie – Avignon
Compagnie du fil à retordre
Adresse : 9 rue du Genevois, 73000 Chambéry
SIRET N° 75244412500021
Facture N°3 en date du 1 décembre 2025
TTC 1920,00
HT 1600,00
Prestation : Location salle (1er acompte)

101 rue de la carreterie – Avignon
Avoir N°A1 date du 13 novembre 2026
TTC 22,00
HT 21,55
Client : Pedro Martinez Mora
"#;
        let parts = split_invoice_documents(text);
        assert_eq!(parts.len(), 4, "segments={parts:?}");
        let parsed: Vec<_> = parts.iter().map(|p| parse_french_invoice_text(p)).collect();
        assert_eq!(parsed[0].fields.invoice_number.as_deref(), Some("1"));
        assert_eq!(parsed[0].fields.party_name.as_deref(), Some("Compagnie KIZUNA"));
        assert_eq!(parsed[1].fields.invoice_number.as_deref(), Some("2"));
        assert_eq!(parsed[2].fields.invoice_number.as_deref(), Some("3"));
        assert_eq!(
            parsed[2].fields.siret.as_deref(),
            Some("75244412500021")
        );
        assert_eq!(parsed[3].document_type, ImportDocumentType::Avoir);
        assert_eq!(parsed[3].fields.invoice_number.as_deref(), Some("A1"));
    }

    #[test]
    fn amounts_incoherent() {
        let incoherent = check_amounts_coherence(Some("100.00"), Some("20.00"), Some("150.00"));
        assert_eq!(incoherent, Some(false));
        let ok = check_amounts_coherence(Some("100.00"), Some("20.00"), Some("120.00"));
        assert_eq!(ok, Some(true));
    }

    #[test]
    fn conflicting_invoice_numbers_mark_uncertain() {
        let text = "Facture N° FA-001\nFacture N° FA-002\nTotal TTC 50,00";
        let r = parse_french_invoice_text(text);
        assert!(r
            .confidence
            .uncertain_fields
            .iter()
            .any(|f| f == "invoice_number"));
    }
}
