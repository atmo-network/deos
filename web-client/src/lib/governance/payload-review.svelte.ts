/*
Domain: Governance payload review state
Owns: Proposal payload review derivation, preimage-cost status, and note-preimage draft feedback.
Excludes: Payload byte encoding ownership, adapter RPC internals, and visual card markup.
Zone: Governance UI-state helper; bridges governance store state into review components.
*/
import type {
  GovernancePayloadHashPreimageStatus,
  GovernancePayloadPreimageNoteCost,
} from '$lib/governance';
import { hashGovernanceAdvisoryPayloadBytes } from '$lib/governance/advisory-payload';
import { governanceStore } from '$lib/governance/index.svelte';

export type PayloadReviewState = {
  payloadHash: string | null;
  payloadHashLoading: boolean;
  payloadHashPreimageStatus: GovernancePayloadHashPreimageStatus | null;
  payloadHashPreimageStatusLoading: boolean;
  payloadPreimageNoteCost: GovernancePayloadPreimageNoteCost | null;
  payloadPreimageNoteCostLoading: boolean;
};

export function createPayloadReview(
  getPayloadBytes: () => Uint8Array | null,
): PayloadReviewState {
  let payloadHash = $state<string | null>(null);
  let payloadHashLoading = $state(false);
  let payloadHashPreimageStatus =
    $state<GovernancePayloadHashPreimageStatus | null>(null);
  let payloadHashPreimageStatusLoading = $state(false);
  let payloadPreimageNoteCost =
    $state<GovernancePayloadPreimageNoteCost | null>(null);
  let payloadPreimageNoteCostLoading = $state(false);

  $effect(() => {
    const bytes = getPayloadBytes();
    if (bytes == null) {
      payloadHash = null;
      payloadHashLoading = false;
      return;
    }
    let cancelled = false;
    payloadHash = null;
    payloadHashLoading = true;
    void hashGovernanceAdvisoryPayloadBytes(bytes)
      .then((hash) => {
        if (cancelled) return;
        payloadHash = hash;
      })
      .catch(() => {
        if (cancelled) return;
        payloadHash = null;
      })
      .finally(() => {
        if (cancelled) return;
        payloadHashLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const bytes = getPayloadBytes();
    const currentHash = payloadHash;
    if (
      currentHash == null ||
      bytes == null ||
      governanceStore.state.providerState.status !== 'connected'
    ) {
      payloadHashPreimageStatus = null;
      payloadHashPreimageStatusLoading = false;
      payloadPreimageNoteCost = null;
      payloadPreimageNoteCostLoading = false;
      return;
    }
    let cancelled = false;
    payloadHashPreimageStatusLoading = true;
    payloadPreimageNoteCostLoading = true;
    void Promise.all([
      governanceStore.lookupPayloadHashPreimageStatus(currentHash),
      governanceStore.lookupPayloadPreimageNoteCost(bytes.length),
    ])
      .then(([status, noteCost]) => {
        if (cancelled) return;
        payloadHashPreimageStatus = status;
        payloadPreimageNoteCost = noteCost;
      })
      .catch(() => {
        if (cancelled) return;
        payloadHashPreimageStatus = null;
        payloadPreimageNoteCost = null;
      })
      .finally(() => {
        if (cancelled) return;
        payloadHashPreimageStatusLoading = false;
        payloadPreimageNoteCostLoading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  return {
    get payloadHash() {
      return payloadHash;
    },
    get payloadHashLoading() {
      return payloadHashLoading;
    },
    get payloadHashPreimageStatus() {
      return payloadHashPreimageStatus;
    },
    get payloadHashPreimageStatusLoading() {
      return payloadHashPreimageStatusLoading;
    },
    get payloadPreimageNoteCost() {
      return payloadPreimageNoteCost;
    },
    get payloadPreimageNoteCostLoading() {
      return payloadPreimageNoteCostLoading;
    },
  };
}
