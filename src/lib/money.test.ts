import { describe, expect, it } from "vitest";
import {
  formatEur,
  lineAmounts,
  parseMoney,
  roundMoney,
  sumLines,
  ttcFromHt,
  tvaFromHt,
} from "./money";

describe("money", () => {
  it("arrondit à 2 décimales", () => {
    expect(roundMoney(1.005)).toBe(1.01);
    expect(roundMoney(10.994)).toBe(10.99);
  });

  it("calcule TVA et TTC depuis HT", () => {
    expect(tvaFromHt(100, 20)).toBe(20);
    expect(ttcFromHt(100, 20)).toBe(120);
  });

  it("calcule les montants de ligne", () => {
    const amounts = lineAmounts(2, 50, 20);
    expect(amounts).toEqual({ ht: 100, tva: 20, ttc: 120 });
  });

  it("agrège les lignes", () => {
    const totals = sumLines([
      { quantity: 1, unit_price_ht: 100, tax_rate_percent: 20 },
      { quantity: 2, unit_price_ht: 50, tax_rate_percent: 10 },
    ]);
    expect(totals.totalHt).toBe(200);
    expect(totals.totalTva).toBe(30);
    expect(totals.totalTtc).toBe(230);
  });

  it("formate en euros français", () => {
    expect(formatEur(1234.5)).toMatch(/1.*234,50/);
  });

  it("parse les décimales string du backend", () => {
    expect(parseMoney("1234.56")).toBe(1234.56);
    expect(parseMoney("10,5")).toBe(10.5);
    expect(parseMoney("")).toBe(0);
  });
});