use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::error::{AppError, AppResult};

pub const EUR_SCALE: u32 = 2;

/// Arrondi à 2 décimales (EUR).
pub fn round_eur(value: Decimal) -> Decimal {
    value.round_dp(EUR_SCALE)
}

/// Montant TTC à partir du HT et du taux de TVA en pourcentage (ex. 20).
pub fn ttc_from_ht(ht: Decimal, tax_rate_percent: Decimal) -> AppResult<Decimal> {
    if tax_rate_percent < dec!(0) {
        return Err(AppError::validation("Le taux de TVA ne peut pas être négatif."));
    }
    let factor = dec!(1) + tax_rate_percent / dec!(100);
    Ok(round_eur(ht * factor))
}

/// Montant TVA à partir du HT.
pub fn tva_from_ht(ht: Decimal, tax_rate_percent: Decimal) -> AppResult<Decimal> {
    if tax_rate_percent < dec!(0) {
        return Err(AppError::validation("Le taux de TVA ne peut pas être négatif."));
    }
    let tva = ht * tax_rate_percent / dec!(100);
    Ok(round_eur(tva))
}

/// Décomposition HT / TVA / TTC pour une ligne (quantité × prix unitaire HT).
pub fn line_amounts(
    quantity: Decimal,
    unit_price_ht: Decimal,
    tax_rate_percent: Decimal,
) -> AppResult<(Decimal, Decimal, Decimal)> {
    // Quantité négative autorisée pour les avoirs ; prix unitaire toujours ≥ 0.
    if unit_price_ht < dec!(0) {
        return Err(AppError::validation(
            "Le prix unitaire doit être positif ou nul.",
        ));
    }
    let ht = round_eur(quantity * unit_price_ht);
    let tva = tva_from_ht(ht, tax_rate_percent)?;
    let ttc = ttc_from_ht(ht, tax_rate_percent)?;
    Ok((ht, tva, ttc))
}

pub fn parse_decimal_amount(s: &str) -> AppResult<Decimal> {
  s.trim()
        .parse::<Decimal>()
        .map(round_eur)
        .map_err(|_| AppError::validation(format!("Montant invalide : « {s} »")))
}

pub fn format_amount(d: Decimal) -> String {
    format!("{:.2}", round_eur(d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tva_ttc_20_pourcent() {
        let ht = dec!(100.00);
        let tva = tva_from_ht(ht, dec!(20)).unwrap();
        assert_eq!(tva, dec!(20.00));
        let ttc = ttc_from_ht(ht, dec!(20)).unwrap();
        assert_eq!(ttc, dec!(120.00));
    }

    #[test]
    fn ligne_quantite_prix() {
        let (ht, tva, ttc) = line_amounts(dec!(2.5), dec!(40.00), dec!(20)).unwrap();
        assert_eq!(ht, dec!(100.00));
        assert_eq!(tva, dec!(20.00));
        assert_eq!(ttc, dec!(120.00));
    }

    #[test]
    fn arrondi_centimes() {
        let (ht, tva, ttc) = line_amounts(dec!(3), dec!(10.33), dec!(20)).unwrap();
        assert_eq!(ht, dec!(30.99));
        assert_eq!(tva, dec!(6.20));
        assert_eq!(ttc, dec!(37.19));
    }
}