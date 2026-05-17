/*
Domain: Governance public barrel
Owns: Re-export boundary for governance contracts, constants, write-surface helpers, and materialized fixtures.
Excludes: Governance store singleton, widgets, adapter providers, and payload review components.
Zone: Governance public API; import through this barrel when consumers need broad governance contracts.
*/
export * from './types';
export * from './constants';
export * from './write-surface';
export * from './materialized';
