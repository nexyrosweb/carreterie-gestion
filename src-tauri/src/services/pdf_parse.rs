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

pub fn parse_french_invoice_text(text: &str) -> ParseResult {
    let normalized = normalize_text(text);
    let upper = normalized.to_uppercase();

    let document_type = detect_document_type(&upper);
    let mut fields = ExtractedFields::default();
    let mut uncertain = Vec::new();

    if let Some(siret) = find_siret(&normalized) {
        fields.siret = Some(siret.clone());
        if siret.len() >= 9 {
            fields.siren = Some(siret[..9].to_string());
        }
    }
    if fields.siren.is_none() {
        if let Some(siren) = find_siren(&normalized) {
            fields.siren = Some(siren);
        }
    }

    match find_invoice_numbers(&normalized) {
        NumberMatch::Single(n) => fields.invoice_number = Some(n),
        NumberMatch::Multiple(candidates) => {
            fields.invoice_number = candidates.first().cloned();
            FieldConfidence::mark_uncertain(&mut uncertain, "invoice_number");
            let _ = candidates;
        }
        NumberMatch::None => {}
    }

    fields.issue_date = find_labeled_date(&normalized, &["DATE", "DATE DE FACTURE", "FACTURE DU"]);
    if fields.issue_date.is_none() {
        fields.issue_date = find_first_date(&normalized);
    }

    fields.payment_date =
        find_labeled_date(&normalized, &["DATE D'ÉCHÉANCE", "DATE ECHEANCE", "ÉCHÉANCE", "ECHEANCE"]);

    fields.total_ht = find_amount_label(&normalized, &["TOTAL HT", "MONTANT HT", "NET HT"]);
    fields.total_tva = find_amount_label(&normalized, &["TOTAL TVA", "MONTANT TVA", "TVA"]);
    fields.total_ttc = find_amount_label(&normalized, &["TOTAL TTC", "NET À PAYER", "NET A PAYER", "MONTANT TTC"]);

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
    if upper.contains("AVOIR") {
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

fn find_siret(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)SIRET\s*[:\s]?\s*(\d[\d\s\.]{12,20}\d)").ok()?;
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
    let re = Regex::new(r"(?i)SIREN\s*[:\s]?\s*(\d[\d\s\.]{7,12})").ok()?;
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
        r"(?i)FACTURE\s*N[°ºo]?\s*[:.]?\s*([A-Za-z0-9][A-Za-z0-9\-/\.]{1,30})",
        r"(?i)\b(FA|AV)-[\w\-/]+",
        r"(?i)\bFA[\-/]?\d{4}[\-/]?\d+",
        r"(?i)\bAV[\-/]?\d{4}[\-/]?\d+",
        r"(?i)N[°ºo]\s*[:.]?\s*([A-Za-z0-9][A-Za-z0-9\-/\.]{1,30})",
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
                    if !v.is_empty() && !found.contains(&v) {
                        found.push(v);
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

fn find_labeled_date(text: &str, labels: &[&str]) -> Option<String> {
    let upper = text.to_uppercase();
    for label in labels {
        if let Some(pos) = upper.find(label) {
            let slice = &text[pos..text.len().min(pos + 80)];
            if let Some(d) = find_first_date(slice) {
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
    None
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
    let upper = text.to_uppercase();
    let amount_re = Regex::new(r"(\d[\d\s]*(?:[.,]\d{2})?)").ok()?;
    for label in labels {
        if let Some(pos) = upper.find(label) {
            let slice = &text[pos..text.len().min(pos + 60)];
            if let Some(cap) = amount_re.captures(slice) {
                let raw = cap[1].to_string();
                if let Some(d) = parse_french_amount(&raw) {
                    return Some(format!("{d:.2}"));
                }
            }
        }
    }
    None
}

fn find_tax_rate(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)TVA\s*(?:à|a)?\s*(\d{1,2}(?:[.,]\d+)?)\s*%").ok()?;
    if let Some(cap) = re.captures(text) {
        let rate = cap[1].replace(',', ".");
        return Some(rate);
    }
    None
}

fn find_related_ref(text: &str) -> Option<String> {
    let re = Regex::new(r"(?i)(?:facture|ref\.?|réf\.?)\s*N[°ºo]?\s*[:.]?\s*([A-Za-z0-9][A-Za-z0-9\-/\.]{1,30})").ok()?;
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