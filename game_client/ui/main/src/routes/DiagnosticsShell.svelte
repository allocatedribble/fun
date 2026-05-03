<script lang="ts">
  import LogPanel from '../lib/components/LogPanel.svelte';
  import WindowTitleBar from '../lib/components/WindowTitleBar.svelte';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;
</script>

<main class="diagnostics-shell">
  <WindowTitleBar mode="editor" title="Diagnostics" subtitle={state.project?.display_name ?? 'runtime'} />
  <LogPanel {state} />
</main>

<style>
  .diagnostics-shell {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto 1fr;
    background: #101316;
  }
</style>
