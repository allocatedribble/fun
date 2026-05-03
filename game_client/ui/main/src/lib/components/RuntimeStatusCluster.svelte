<script lang="ts">
  import type { EditorUiState } from '../types';
  import StatusPill from './StatusPill.svelte';

  export let state: EditorUiState;

  $: runtime = state.runtimeStatus ?? state.status?.runtime_status ?? null;
  $: authState = state.status?.auth_session.expired ? 'expired' : 'ready';
</script>

<div class="runtime-status-cluster">
  <StatusPill label="preview" value={state.previewRenderStatus.status} tone={state.previewRenderStatus.status === 'ready' ? 'good' : state.previewRenderStatus.status === 'failed' ? 'bad' : 'neutral'} />
  <StatusPill label="client" value={runtime?.client_process.state ?? 'unknown'} tone={runtime?.client_process.state === 'running' ? 'good' : 'neutral'} />
  <StatusPill label="server" value={runtime?.server_process.state ?? 'unknown'} tone={runtime?.server_process.state === 'running' ? 'good' : 'neutral'} />
  <StatusPill label="auth" value={authState} tone={authState === 'ready' ? 'good' : 'bad'} />
</div>
