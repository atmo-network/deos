/*
Domain: UI Kit formatting helpers
Owns: Presentation-only numeric conversion and display formatting for UI surfaces.
Excludes: Protocol arithmetic decisions, persistence, and product state.
Zone: Foundation UI helper; must remain dependency-light and domain-agnostic.
*/
import { DECIMALS, PRECISION } from '$lib/economics';

export const TOKEN_DECIMALS = Number(DECIMALS);
export const TOKEN_BASE_UNITS_PER_TOKEN = PRECISION;

const TOKEN_INPUT_PATTERN = new RegExp(
  `^(\\d+)(?:\\.(\\d{1,${TOKEN_DECIMALS}}))?$`,
);

export function toFloat(v: bigint): number {
  return Number(v) / Number(TOKEN_BASE_UNITS_PER_TOKEN);
}

export function parseTokenInputAmount(value: string): bigint | null {
  const trimmed = value.trim();
  const match = TOKEN_INPUT_PATTERN.exec(trimmed);
  if (!match) {
    return null;
  }
  const whole = BigInt(match[1]);
  const fraction = BigInt((match[2] ?? '').padEnd(TOKEN_DECIMALS, '0'));
  const amount = whole * TOKEN_BASE_UNITS_PER_TOKEN + fraction;
  return amount > 0n ? amount : null;
}

export function formatTokenInputAmount(value: bigint): string {
  const whole = value / TOKEN_BASE_UNITS_PER_TOKEN;
  const fraction = (value % TOKEN_BASE_UNITS_PER_TOKEN)
    .toString()
    .padStart(TOKEN_DECIMALS, '0')
    .replace(/0+$/u, '');
  return fraction.length > 0 ? `${whole}.${fraction}` : whole.toString();
}

export function fmt(n: number): string {
  if (Math.abs(n) >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (Math.abs(n) >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (Math.abs(n) >= 1e4) return (n / 1e3).toFixed(1) + 'K';
  if (Math.abs(n) >= 1) return n.toFixed(2);
  if (Math.abs(n) >= 0.01) return n.toFixed(4);
  return n.toFixed(6);
}

export function fmtPrice(n: number): string {
  if (n === 0) return '0.0000';
  if (n >= 100) return n.toFixed(2);
  if (n >= 1) return n.toFixed(4);
  if (n >= 0.0001) return n.toFixed(6);
  return n.toExponential(2);
}

export function fmtOut(n: number): string {
  if (n >= 1e6) return (n / 1e6).toFixed(3) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(3) + 'K';
  if (n >= 1) return n.toFixed(4);
  return n.toFixed(6);
}

export function fmtInputAmount(n: number, decimals = 6): string {
  if (!Number.isFinite(n) || n <= 0) return '';
  const fixed = n.toFixed(decimals);
  return fixed.replace(/\.0+$|(?<=\.[0-9]*?)0+$/u, '');
}

export function fmtBigInt(v: bigint): string {
  return fmt(toFloat(v));
}

export function fmtPriceBigInt(v: bigint): string {
  return fmtPrice(toFloat(v));
}
