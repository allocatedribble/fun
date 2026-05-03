<script lang="ts">
  import type { EditorContextId, EditorUiState } from '../types';

  export let state: EditorUiState;
  export let onContext: (contextId: EditorContextId) => void;

  type WorkspaceTab = {
    id: 'overview' | 'viewport' | 'graph' | 'diagnostics';
    label: string;
    status: string;
    context: EditorContextId;
    active: boolean;
  };

  $: viewportStatus =
    state.editorContext === 'live_client'
      ? state.liveClientStatus?.status ?? 'client'
      : state.editorContext === 'server'
        ? state.serverRuntime?.process_status ?? 'server'
        : state.previewRenderStatus.status;
  $: tabs = [
    {
      id: 'overview',
      label: 'Overview',
      status: `${state.entities.totalRows.toLocaleString()} entities`,
      context: 'overview',
      active: state.editorContext === 'overview'
    },
    {
      id: 'viewport',
      label: 'Viewport',
      status: viewportStatus,
      context: state.editorContext === 'live_client' || state.editorContext === 'server' ? state.editorContext : 'preview',
      active: state.editorContext === 'preview' || state.editorContext === 'live_client' || state.editorContext === 'server'
    },
    {
      id: 'graph',
      label: 'Graph',
      status: 'WGSL',
      context: 'graph',
      active: state.editorContext === 'graph'
    },
    {
      id: 'diagnostics',
      label: 'Log',
      status: `${state.diagnostics.length + (state.runtimeDiagnostics?.records.length ?? 0)} rows`,
      context: 'diagnostics',
      active: state.editorContext === 'diagnostics'
    }
  ] satisfies WorkspaceTab[];
</script>

<nav class="workspace-tabs tabs is-small" aria-label="Editor workspace tabs" data-no-drag>
  <ul>
    {#each tabs as tab (tab.id)}
      <li class:is-active={tab.active}>
        <button type="button" on:click={() => onContext(tab.context)}>
          <strong>{tab.label}</strong>
          <small>{tab.status}</small>
        </button>
      </li>
    {/each}
  </ul>
</nav>
