/*
Domain: Materialized history adapter
Owns: External-indexer history provider contract for long-range chart data.
Excludes: Canonical chain projections, market store state, and chart rendering.
Zone: Adapter boundary for future materialized read-model data.
*/
import type { PricePoint } from '$lib/market/types';

export class MaterializedHistoryProvider {
  /**
   * Explicit materialized history contract for long-range charts.
   * A future external indexer implementation can fetch history beyond
   * the safe limit of on-chain bounded sampling through this boundary.
   */
  async getLongRangeHistory(limit: number): Promise<PricePoint[]> {
    void limit;
    return [];
  }
}

export const createMaterializedHistoryProvider = () => {
  return new MaterializedHistoryProvider();
};
