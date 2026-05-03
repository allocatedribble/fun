<script lang="ts">
  import type { EcsDomain, EntitySourceKind } from '../types';

  export let domain: EcsDomain;
  export let sourceKind: EntitySourceKind | null = null;
</script>

<span class={`domain-badge ${domain}`}>
  {domain}
  {#if sourceKind}
    <small>{sourceKind}</small>
  {/if}
</span>
