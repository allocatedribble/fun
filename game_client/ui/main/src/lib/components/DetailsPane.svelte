<script lang="ts">
  import type { EditorUiState, TransformPatchRequest } from '../types';
  import OverviewSelectionTooltip from './OverviewSelectionTooltip.svelte';

  export let state: EditorUiState;
  export let onPatchTransform: (request: TransformPatchRequest) => void | Promise<void>;

  $: hasSelection = Boolean(state.overview.selectedItem || state.entities.selectedDetails);
</script>

<section class="details-pane panel" aria-label="Details">
  {#if hasSelection}
    <OverviewSelectionTooltip {state} onPatchTransform={onPatchTransform} />
  {:else}
    <div class="details-empty">
      <span class="eyebrow">Details</span>
      <h2>No selection</h2>
      <p>Select a project item, asset, scene, diagnostic, or entity in Overview.</p>
    </div>
  {/if}
</section>
