/** Alphabet sans caractères ambigus (0/O, 1/l/I). */
const UPPER = "ABCDEFGHJKLMNPQRSTUVWXYZ";
const LOWER = "abcdefghijkmnopqrstuvwxyz";
const DIGITS = "23456789";
const SYMBOLS = "!@#$%&*?";

const ALL = UPPER + LOWER + DIGITS + SYMBOLS;

function randomIndex(max: number): number {
  const buf = new Uint32Array(1);
  crypto.getRandomValues(buf);
  return (buf[0] ?? 0) % max;
}

function pick(alphabet: string): string {
  return alphabet.charAt(randomIndex(alphabet.length));
}

/**
 * Génère un mot de passe sécurisé (longueur par défaut : 12).
 * Garantit au moins une majuscule, une minuscule, un chiffre et un symbole.
 */
export function generateSecurePassword(length = 12): string {
  const size = Math.max(length, 12);
  const chars: string[] = [
    pick(UPPER),
    pick(LOWER),
    pick(DIGITS),
    pick(SYMBOLS),
  ];
  while (chars.length < size) {
    chars.push(pick(ALL));
  }
  // Mélange Fisher–Yates
  for (let i = chars.length - 1; i > 0; i--) {
    const j = randomIndex(i + 1);
    const tmp = chars[i]!;
    chars[i] = chars[j]!;
    chars[j] = tmp;
  }
  return chars.join("");
}

export function isPasswordStrongEnough(password: string, minLength = 12): boolean {
  if (password.length < minLength) return false;
  const hasUpper = /[A-Z]/.test(password);
  const hasLower = /[a-z]/.test(password);
  const hasDigit = /\d/.test(password);
  return hasUpper && hasLower && hasDigit;
}
