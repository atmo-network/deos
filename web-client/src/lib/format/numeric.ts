/*
Domain: Numeric literal formatting
Owns: Complete-literal numeric parsers shared by UI forms and domain payload builders.
Excludes: Token precision policy, protocol arithmetic, widget state, and adapter transport.
Zone: Foundation format helper; dependency-free and safe for domain slices to import.
*/

const UNSIGNED_DECIMAL_INTEGER = /^\d+$/u;
const UNSIGNED_DECIMAL_NUMBER = /^\d+(?:\.\d+)?$/u;

export type DecimalNumberBounds = {
  min?: number;
  max?: number;
};

export type DecimalBigIntBounds = {
  min?: bigint;
  max?: bigint;
};

function applyNumberBounds(
  parsed: number,
  bounds: DecimalNumberBounds,
): number | null {
  if (!Number.isFinite(parsed)) {
    return null;
  }
  if (bounds.min !== undefined && parsed < bounds.min) {
    return null;
  }
  if (bounds.max !== undefined && parsed > bounds.max) {
    return null;
  }
  return parsed;
}

export function parseUnsignedDecimalNumber(
  value: string,
  bounds: DecimalNumberBounds = {},
): number | null {
  const trimmed = value.trim();
  if (!UNSIGNED_DECIMAL_INTEGER.test(trimmed)) {
    return null;
  }
  const parsed = Number(trimmed);
  return Number.isSafeInteger(parsed)
    ? applyNumberBounds(parsed, bounds)
    : null;
}

export function parseUnsignedDecimalFloat(
  value: string,
  bounds: DecimalNumberBounds = {},
): number | null {
  const trimmed = value.trim();
  if (!UNSIGNED_DECIMAL_NUMBER.test(trimmed)) {
    return null;
  }
  return applyNumberBounds(Number(trimmed), bounds);
}

export function parseUnsignedDecimalBigInt(
  value: string,
  bounds: DecimalBigIntBounds = {},
): bigint | null {
  const trimmed = value.trim();
  if (!UNSIGNED_DECIMAL_INTEGER.test(trimmed)) {
    return null;
  }
  const parsed = BigInt(trimmed);
  if (bounds.min !== undefined && parsed < bounds.min) {
    return null;
  }
  if (bounds.max !== undefined && parsed > bounds.max) {
    return null;
  }
  return parsed;
}
