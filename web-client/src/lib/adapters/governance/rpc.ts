/*
Domain: Governance RPC adapter barrel
Owns: Legacy/stable RPC provider exports and endpoint re-exports for governance adapter consumers.
Excludes: Provider implementation, mock data, store lifecycle, and governance contracts ownership.
Zone: Adapter compatibility boundary; preserves public names while delegating to PAPI provider.
*/
export {
  DEFAULT_GOVERNANCE_RPC_ENDPOINT,
  DEFAULT_GOVERNANCE_WS_ENDPOINT,
} from '$lib/governance';
export {
  GovernancePapiProvider,
  GovernancePapiProvider as GovernanceRpcProvider,
} from './papi';
