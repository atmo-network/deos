/*
Domain: System endpoint state
Owns: Browser-session blockchain endpoint selection and mutation helpers.
Excludes: Governance constants ownership, adapter connection lifecycle, and persistence policy.
Zone: System session helper; provides endpoint input to the system composition root.
*/
import { DEFAULT_GOVERNANCE_WS_ENDPOINT } from '$lib/governance/constants';

let blockchainEndpoint = DEFAULT_GOVERNANCE_WS_ENDPOINT;

export function getBlockchainEndpoint(): string {
  return blockchainEndpoint;
}

export function setBlockchainEndpoint(endpoint: string): void {
  blockchainEndpoint = endpoint.trim();
}
