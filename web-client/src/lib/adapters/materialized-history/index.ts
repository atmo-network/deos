import type { PricePoint } from "$lib/shared/types";

export class MaterializedHistoryProvider {
  /**
   * Explicit materialized history contract for long-range charts.
   * This is a stub for an external indexer (e.g., Subsquid / SubQuery)
   * to fetch history beyond the safe limit of on-chain bounded sampling.
   */
  async getLongRangeHistory(limit: number): Promise<PricePoint[]> {
    // In production, this would make a GraphQL query to an indexer
    return [];
  }
}

export const createMaterializedHistoryProvider = () => {
  return new MaterializedHistoryProvider();
};
