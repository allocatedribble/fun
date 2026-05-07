<script lang="ts">
  import LogPanel from '../lib/components/LogPanel.svelte';
  import RvelteDevIsland from '../lib/rvelte-dev/RvelteDevIsland.svelte';
  import { RVELTE_DEV_ISLAND_ENABLED } from '../lib/rvelte-dev/bridge';
  import WindowTitleBar from '../lib/components/WindowTitleBar.svelte';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;
</script>

<main class:rvelte-enabled={RVELTE_DEV_ISLAND_ENABLED} class="diagnostics-shell">
  <WindowTitleBar mode="editor" title="Diagnostics" subtitle={state.project?.display_name ?? 'runtime'} />
  {#if RVELTE_DEV_ISLAND_ENABLED}
    <RvelteDevIsland {state} />
  {/if}
  <LogPanel {state} />
</main>

<style>
  .diagnostics-shell {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr;
    background: #101316;
  }

  .diagnostics-shell.rvelte-enabled {
    grid-template-rows: auto auto minmax(0, 1fr);
  }
</style>
