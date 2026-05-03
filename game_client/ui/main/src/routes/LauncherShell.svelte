<script lang="ts">
  import LauncherShellComponent from '../lib/components/LauncherShell.svelte';
  import type { EditorUiState } from '../lib/types';

  export let state: EditorUiState;
</script>

<LauncherShellComponent {state} />
