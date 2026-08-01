/*
Domain: Numeric literal formatting
Owns: Complete-literal numeric parsers shared by UI forms and domain payload builders.
Excludes: Token precision policy, protocol arithmetic, widget state, and adapter transport.
Zone: Foundation format helper; dependency-free and safe for domain slices to import.
*/

const UNSIGNED_DECIMAL_INTEGER = /^\d+$/u;
const UNSIGNED_DECIMAL_NUMBER = /^\d+(?:\.\d+)?$/u;
const UNSIGNED_HEX = /^[0-9a-f]+$/iu;

function hexDigitValue(digit: string): number {
  const code = digit.charCodeAt(0);
  if (code >= 48 && code <= 57) {
    return code - 48;
  }
  return code >= 97 ? code - 87 : code - 55;
}

export function parseUnsignedHexByte(byte: string): number | null {
  if (!/^[0-9a-f]{2}$/iu.test(byte)) {
    return null;
  }
  return hexDigitValue(byte[0]) * 16 + hexDigitValue(byte[1]);
}

export function parseUnsignedHexNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!UNSIGNED_HEX.test(trimmed) || trimmed.length % 2 !== 0) {
    return null;
  }
  const bytes = Uint8Array.from(
    trimmed.match(/.{2}/gu)!.map((byte) => {
      const decoded = parseUnsignedHexByte(byte);
      if (decoded === null) {
        throw new Error(`invalid hex byte: ${byte}`);
      }
      return decoded;
    }),
  );
  if (bytes.length > 6) {
    return null;
  }
  const parsed = bytes.reduce((acc, byte) => acc * 256 + byte, 0);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

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
