/*
Domain: Widget public barrel
Owns: Re-export boundary for dashboard, lane, account, governance, market, staking, log, and wiki widgets.
Excludes: Widget implementation internals, layout composition policy, domain stores, and UI Kit primitives.
Zone: Widget package API; imported by layout/widget loader and shell composition.
*/
export { default as AccountChip } from './AccountChip.svelte';
export { default as AccountWidget } from './AccountWidget.svelte';
export { default as AutomationWidget } from './AutomationWidget.svelte';
export { default as SettingsWidget } from './SettingsWidget.svelte';
export { default as ChartWidget } from './ChartWidget.svelte';
export { default as GovernanceWidget } from './GovernanceWidget.svelte';
export { default as StatisticsWidget } from './StatisticsWidget.svelte';
export { default as StatusWidget } from './StatusWidget.svelte';
export { default as LogWidget } from './LogWidget.svelte';
export { default as SwapWidget } from './SwapWidget.svelte';
export { default as WalletWidget } from './WalletWidget.svelte';
export { default as WikiWidget } from './WikiWidget.svelte';
