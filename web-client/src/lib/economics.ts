/*
Domain: Economic numeric constants
Owns: Shared fixed-point scale constants used by UI-side protocol calculations and formatting bridges.
Excludes: Tokenomic policy, simulator formulas, runtime parameters, and presentation formatting.
Zone: Foundation contract; dependency-free and safe for domains/adapters to import.
*/
export const DECIMALS = 12n;
export const PRECISION = 10n ** DECIMALS;
export const PPM = 1_000_000n;
export const PPB = 1_000_000_000n;
