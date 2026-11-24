/** SIRET / SIREN / TVA FR — validation Luhn + formatage. */

export function digitsOnly(value: string): string {
  return value.replace(/\D/g, "");
}

export function formatSiretDisplay(value: string): string {
  const d = digitsOnly(value).slice(0, 14);
  const parts = [d.slice(0, 3), d.slice(3, 6), d.slice(6, 9), d.slice(9, 14)].filter(Boolean);
  return parts.join(" ");
}

export function formatSirenDisplay(value: string): string {
  const d = digitsOnly(value).slice(0, 9);
  const parts = [d.slice(0, 3), d.slice(3, 6), d.slice(6, 9)].filter(Boolean);
  return parts.join(" ");
}

export function luhnValid(digits: string): boolean {
  const d = digitsOnly(digits);
  if (!d) return false;
  let sum = 0;
  let double = false;
  for (let i = d.length - 1; i >= 0; i--) {
    let n = Number(d[i]);
    if (double) {
      n *= 2;
      if (n > 9) n -= 9;
    }
    sum += n;
    double = !double;
  }
  return sum % 10 === 0;
}

export function isValidSiren(value: string): boolean {
  const d = digitsOnly(value);
  return d.length === 9 && luhnValid(d);
}

export function isValidSiret(value: string): boolean {
  const d = digitsOnly(value);
  return d.length === 14 && luhnValid(d);
}

export function sirenFromSiret(value: string): string | null {
  const d = digitsOnly(value);
  if (d.length === 14) return d.slice(0, 9);
  if (d.length === 9) return d;
  return null;
}

/** Clé TVA FR : (12 + 3 × (SIREN mod 97)) mod 97 */
export function frenchVatFromSiren(siren: string): string | null {
  const d = digitsOnly(siren);
  if (d.length !== 9) return null;
  const n = Number(d);
  if (!Number.isFinite(n)) return null;
  const key = (12 + 3 * (n % 97)) % 97;
  return `FR${String(key).padStart(2, "0")}${d}`;
}

export function frenchVatFromSiret(siret: string): string | null {
  const siren = sirenFromSiret(siret);
  return siren ? frenchVatFromSiren(siren) : null;
}

export function siretFeedback(value: string): { ok: boolean; message?: string } {
  const d = digitsOnly(value);
  if (!d) return { ok: true };
  if (d.length < 14) return { ok: false, message: `${d.length}/14 chiffres` };
  if (d.length > 14) return { ok: false, message: "Trop de chiffres (14 max)." };
  if (!luhnValid(d)) return { ok: false, message: "Clé de contrôle SIRET invalide." };
  return { ok: true };
}
