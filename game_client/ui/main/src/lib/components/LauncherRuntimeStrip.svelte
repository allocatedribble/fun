<script lang="ts">
  import type { EditorUiState } from '../types';

  export let state: EditorUiState;

  $: runtime = state.runtimeHostStatus;
  $: active = state.activeRuntime;
</script>

<section class="launcher-runtime-strip" aria-label="Runtime status">
  <div>
    <span class="label">Host</span>
    <strong>{runtime?.host_state ?? 'unknown'}</strong>
  </div>
  <div>
    <span class="label">Client</span>
    <strong>{runtime?.runtime_status.client_process.state ?? 'unknown'}</strong>
  </div>
  <div>
    <span class="label">Inspector</span>
    <strong>{runtime?.runtime_status.client_inspector.state ?? 'unknown'}</strong>
  </div>
  <div>
    <span class="label">Server</span>
    <strong>{runtime?.runtime_status.server_process.state ?? 'unknown'}</strong>
  </div>
  <div class="runtime-strip-active">
    <span class="label">Active</span>
    <strong>{active ? `${active.kind} / ${active.status}` : 'none'}</strong>
  </div>
</section>
