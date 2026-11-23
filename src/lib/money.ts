const EUR_FORMATTER = new Intl.NumberFormat("fr-FR", {
  style: "currency",
  currency: "EUR",
});

export function roundMoney(value: number, decimals = 2): number {
  const factor = 10 ** decimals;
  return Math.round((value + Number.EPSILON) * factor) / factor;
}

export function tvaFromHt(ht: number, taxRatePercent: number): number {
  return roundMoney(ht * (taxRatePercent / 100));
}

export function ttcFromHt(ht: number, taxRatePercent: number): number {
  return roundMoney(ht + tvaFromHt(ht, taxRatePercent));
}

export function lineAmounts(
  quantity: number,
  unitPriceHt: number,
  taxRatePercent: number,
): { ht: number; tva: number; ttc: number } {
  const ht = roundMoney(quantity * unitPriceHt);
  const tva = tvaFromHt(ht, taxRatePercent);
  const ttc = roundMoney(ht + tva);
  return { ht, tva, ttc };
}

export function sumLines(
  lines: Array<{
    quantity: number;
    unit_price_ht: number;
    tax_rate_percent: number;
  }>,
): { totalHt: number; totalTva: number; totalTtc: number } {
  let totalHt = 0;
  let totalTva = 0;
  for (const line of lines) {
    const { ht, tva } = lineAmounts(
      line.quantity,
      line.unit_price_ht,
      line.tax_rate_percent,
    );
    totalHt = roundMoney(totalHt + ht);
    totalTva = roundMoney(totalTva + tva);
  }
  return {
    totalHt,
    totalTva,
    totalTtc: roundMoney(totalHt + totalTva),
  };
}

export function formatEur(amount: number): string {
  return EUR_FORMATTER.format(amount);
}

export function parseAmountInput(raw: string): number | null {
  const normalized = raw.trim().replace(/\s/g, "").replace(",", ".");
  if (!normalized) return null;
  const value = Number.parseFloat(normalized);
  if (!Number.isFinite(value)) return null;
  return roundMoney(value);
}

/** Parse un montant renvoyé par le backend (Decimal sérialisé en string ou number). */
export function parseMoney(value: string | number | null | undefined): number {
  if (value == null) return 0;
  if (typeof value === "number") {
    return Number.isFinite(value) ? roundMoney(value) : 0;
  }
  const normalized = value.trim().replace(/\s/g, "").replace(",", ".");
  if (!normalized) return 0;
  const n = Number.parseFloat(normalized);
  return Number.isFinite(n) ? roundMoney(n) : 0;
}

export function formatMoneyDecimal(value: string | number | null | undefined): string {
  return formatEur(parseMoney(value));
}