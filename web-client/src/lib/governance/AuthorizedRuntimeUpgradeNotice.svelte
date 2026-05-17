<!--
Domain: Governance authorized runtime-upgrade notice
Owns: Browser explanation of the post-governance authorized-code relay boundary.
Excludes: Runtime upgrade submission, governance store mutation, and active proposal card rendering.
Zone: Governance presentation component; separates ministerial relay guidance from voting/submission forms.
-->
<script lang="ts">
  import type { GovernanceAuthorizedRuntimeUpgrade } from '$lib/governance';
  import { runtimeUpgradeOperatorPathLabel } from '$lib/governance/labels';
  import { DetailRow, Notice } from '$lib/ui';

  type Props = {
    authorization: GovernanceAuthorizedRuntimeUpgrade;
  };

  let { authorization }: Props = $props();

  const authorizationLabel = $derived(
    authorization.checkVersion
      ? `${authorization.codeHash} · version checked`
      : `${authorization.codeHash} · no version check`,
  );
</script>

<Notice variant="warn">
  A governance-authorized runtime upgrade is pending authorized code relay ·
  {authorizationLabel}
</Notice>
<Notice variant="muted">
  System.apply_authorized_upgrade with matching code bytes is a separate
  system-level relay step that any origin may submit after governance
  authorization, and this browser intentionally does not expose that live write
  path
</Notice>
<Notice variant="muted">
  Any operator may relay the matching code bytes after governance authorization,
  but this step remains ministerial rather than a second governance decision
</Notice>
<DetailRow
  label="Operator path"
  value={runtimeUpgradeOperatorPathLabel()}
  valueClass="text-(--mono-text) break-all"
/>
