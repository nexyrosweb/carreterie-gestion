import { describe, expect, it } from "vitest";
import { generateSecurePassword, isPasswordStrongEnough } from "./password";

describe("generateSecurePassword", () => {
  it("génère 12 caractères par défaut", () => {
    const pwd = generateSecurePassword();
    expect(pwd).toHaveLength(12);
  });

  it("contient majuscule, minuscule et chiffre", () => {
    for (let i = 0; i < 20; i++) {
      const pwd = generateSecurePassword(12);
      expect(isPasswordStrongEnough(pwd, 12)).toBe(true);
    }
  });

  it("produit des valeurs distinctes", () => {
    const a = generateSecurePassword(12);
    const b = generateSecurePassword(12);
    expect(a).not.toBe(b);
  });
});
