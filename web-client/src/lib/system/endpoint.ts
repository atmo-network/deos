import { DEFAULT_GOVERNANCE_WS_ENDPOINT } from "$lib/adapters/governance/constants";

let blockchainEndpoint = DEFAULT_GOVERNANCE_WS_ENDPOINT;

export function getBlockchainEndpoint(): string {
  return blockchainEndpoint;
}

export function setBlockchainEndpoint(endpoint: string): void {
  blockchainEndpoint = endpoint.trim();
}
