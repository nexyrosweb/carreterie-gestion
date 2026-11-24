//! Validation SIRET (clé de Luhn) + dérivation SIREN / TVA intracommunautaire FR.

use crate::error::{AppError, AppResult};

/// Ne conserve que les chiffres.
pub fn digits_only(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Formate un SIRET : `XXX XXX XXX XXXXX`.
pub fn format_siret(s: &str) -> String {
    let d = digits_only(s);
    if d.len() != 14 {
        return d;
    }
    format!(
        "{} {} {} {}",
        &d[0..3],
        &d[3..6],
        &d[6..9],
        &d[9..14]
    )
}

/// Formate un SIREN : `XXX XXX XXX`.
pub fn format_siren(s: &str) -> String {
    let d = digits_only(s);
    if d.len() != 9 {
        return d;
    }
    format!("{} {} {}", &d[0..3], &d[3..6], &d[6..9])
}

/// Algorithme de Luhn (utilisé pour SIREN / SIRET français).
pub fn luhn_valid(digits: &str) -> bool {
    let d = digits_only(digits);
    if d.is_empty() || !d.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let mut sum = 0u32;
    let mut double = false;
    for c in d.chars().rev() {
        let mut n = c.to_digit(10).unwrap_or(0);
        if double {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        double = !double;
    }
    sum % 10 == 0
}

#[allow(dead_code)]
pub fn is_valid_siren(s: &str) -> bool {
    let d = digits_only(s);
    d.len() == 9 && luhn_valid(&d)
}

#[allow(dead_code)]
pub fn is_valid_siret(s: &str) -> bool {
    let d = digits_only(s);
    d.len() == 14 && luhn_valid(&d)
}

pub fn siren_from_siret(s: &str) -> Option<String> {
    let d = digits_only(s);
    if d.len() == 14 {
        Some(d[..9].to_string())
    } else if d.len() == 9 {
        Some(d)
    } else {
        None
    }
}

/// Clé TVA FR : `(12 + 3 × (SIREN mod 97)) mod 97` → `FRkkSIREN`.
pub fn french_vat_from_siren(siren: &str) -> Option<String> {
    let d = digits_only(siren);
    if d.len() != 9 {
        return None;
    }
    let n: u64 = d.parse().ok()?;
    let key = (12 + 3 * (n % 97)) % 97;
    Some(format!("FR{key:02}{d}"))
}

pub fn french_vat_from_siret(siret: &str) -> Option<String> {
    french_vat_from_siren(&siren_from_siret(siret)?)
}

/// Valide / normalise un SIRET saisi. Vide → Ok(None).
pub fn normalize_siret_opt(raw: Option<&str>) -> AppResult<Option<String>> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let d = digits_only(s);
    if d.len() != 14 {
        return Err(AppError::validation(
            "Le SIRET doit contenir exactement 14 chiffres.",
        ));
    }
    if !luhn_valid(&d) {
        return Err(AppError::validation(
            "SIRET invalide (clé de contrôle incorrecte).",
        ));
    }
    Ok(Some(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_valid_siret() {
        // Exemple pédagogique souvent utilisé (La Poste) — vérifier Luhn générique
        assert!(luhn_valid("73282932000074"));
        assert!(is_valid_siret("73282932000074"));
        assert_eq!(siren_from_siret("73282932000074").as_deref(), Some("732829320"));
        assert!(is_valid_siren("732829320"));
    }

    #[test]
    fn vat_key() {
        let vat = french_vat_from_siren("732829320").unwrap();
        assert!(vat.starts_with("FR"));
        assert_eq!(vat.len(), 13); // FR + 2 + 9
        assert!(vat.ends_with("732829320"));
    }

    #[test]
    fn format_ok() {
        assert_eq!(format_siret("73282932000074"), "732 829 320 00074");
    }
}
