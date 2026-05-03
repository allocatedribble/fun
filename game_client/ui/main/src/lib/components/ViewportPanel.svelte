<script lang="ts">
  import type { EditorUiState, ViewportRect } from '../types';
  import LiveClientSurface from './LiveClientSurface.svelte';
  import PreviewSurface from './PreviewSurface.svelte';
  import StatusPill from './StatusPill.svelte';

  export let state: EditorUiState;
  export let onFocus: () => void | Promise<void>;
  export let onResize: (rect: ViewportRect) => void | Promise<void>;

  $: activeContext = state.editorContext;
  $: viewportContext =
    activeContext === 'live_client' || activeContext === 'server' ? activeContext : 'preview';
  $: activeScene = state.project?.bsn_index.records.find((record) => record.id === state.activeSceneId);
  $: contextTitle =
    viewportContext === 'preview'
      ? 'Current-client preview'
      : viewportContext === 'live_client'
        ? 'Current client'
        : 'Server authority';
  $: statusValue =
    viewportContext === 'preview'
      ? state.previewRenderStatus.status
      : viewportContext === 'live_client'
        ? state.liveClientStatus?.status ?? 'stopped'
        : state.serverRuntime?.process_status ?? state.runtimeStatus?.server_process.state ?? 'stopped';
</script>

<section class="viewport-panel">
  <div class="viewport-header">
    <div>
      <span class="eyebrow">Viewport</span>
      <h2>{state.project?.display_name ?? 'No project'} {contextTitle}</h2>
    </div>
    <div class="viewport-pills">
      <StatusPill label="context" value={viewportContext} />
      <StatusPill label="scene" value={activeScene?.scene_function_name ?? state.activeSceneId ?? 'default'} />
      <StatusPill label="render" value={state.renderMode} />
      <StatusPill label="status" value={statusValue} tone={statusValue === 'ready' || statusValue === 'running' ? 'good' : statusValue === 'failed' ? 'bad' : 'neutral'} />
    </div>
  </div>

  <div class="viewport-host" class:server-context={viewportContext === 'server'}>
    {#if viewportContext === 'preview'}
      <PreviewSurface
        {state}
        onResize={onResize}
      />
    {:else if viewportContext === 'live_client'}
      <LiveClientSurface {state} onResize={onResize} onFocus={onFocus} />
    {:else}
      <div class="server-surface">
        <div class="viewport-badges">
          <StatusPill label="context" value="Server" />
          <StatusPill label="status" value={statusValue} tone={statusValue === 'running' ? 'good' : 'neutral'} />
        </div>
        <div class="viewport-empty">
          <strong>Server authority</strong>
          <span>{state.serverRuntime?.process_status ?? state.runtimeStatus?.server_process.detail ?? 'Launch a server runtime to inspect authority state.'}</span>
        </div>
      </div>
    {/if}
  </div>

  <div class="runtime-command mono">
    {#if viewportContext === 'preview'}
      <span>preview.renderer.ensure - current client background - {state.previewRenderStatus.renderer?.frame.readback_status ?? state.previewRenderStatus.status}</span>
    {:else if viewportContext === 'live_client'}
      <span>viewport.client.focus - host.mode=game - input=gameplay</span>
    {:else if state.serverRuntime}
      <span>{state.serverRuntime.launch_command.join(' ')}</span>
    {:else}
      <span>runtime.server.launch</span>
    {/if}
  </div>
</section>
