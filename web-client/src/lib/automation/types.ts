/*
Domain: Automation contracts
Owns: System AAA actor snapshot shapes surfaced to automation views.
Excludes: Runtime actor scheduling, adapter transport, and widget rendering.
Zone: Automation public contract; safe for adapters, stores, and widgets to import.
*/
export type AutomationActorSnapshot = {
  aaaId: number;
  label: string;
  role: string;
  exists: boolean;
  paused: boolean;
  lastCycleBlock: number | null;
  triggerLabel: string;
  nativeBalance: bigint;
};
