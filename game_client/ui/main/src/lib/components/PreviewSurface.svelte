<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorUiState, ViewportRect } from '../types';
  import StatusPill from './StatusPill.svelte';

  export let state: EditorUiState;
  export let onResize: (rect: ViewportRect) => void | Promise<void>;

  let surface: HTMLDivElement | undefined;
  let observer: ResizeObserver | null = null;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;

  $: renderer = state.previewRenderStatus.renderer;
  $: status = state.previewRenderStatus.status;
  $: sceneLabel = state.previewRenderStatus.sceneId ?? state.activeSceneId ?? 'default scene';

  onMount(() => {
    if (surface && typeof ResizeObserver !== 'undefined') {
      observer = new ResizeObserver(queueResize);
      observer.observe(surface);
      queueResize();
    }

    return () => {
      observer?.disconnect();
      if (resizeTimer) {
        clearTimeout(resizeTimer);
      }
    };
  });

  function queueResize(): void {
    if (!surface) {
      return;
    }
    if (resizeTimer) {
      clearTimeout(resizeTimer);
    }
    resizeTimer = setTimeout(() => {
      if (!surface) {
        return;
      }
      const rect = surface.getBoundingClientRect();
      void onResize({
        x: rect.left,
        y: rect.top,
        width: rect.width,
        height: rect.height,
        scale_factor: window.devicePixelRatio || 1
      });
    }, 120);
  }
</script>

<div class={`preview-surface ${status}`} bind:this={surface} role="presentation" aria-label="Current client preview">
  <div class="viewport-badges">
    <StatusPill label="context" value="Preview" />
    <StatusPill label="scene" value={sceneLabel} />
    <StatusPill label="render" value={state.renderMode} />
    <StatusPill label="status" value={status} tone={status === 'ready' ? 'good' : status === 'failed' ? 'bad' : 'neutral'} />
    {#if state.previewRenderStatus.warning}
      <StatusPill label="signature" value={state.previewRenderStatus.warning} tone="warn" />
    {/if}
  </div>

  {#if status === 'preparing'}
    <div class="viewport-empty">
      <strong>Preparing current-client preview</strong>
      <span>Aligning the editor overlay with the running game render.</span>
    </div>
  {:else if status === 'failed'}
    <div class="viewport-empty error-state">
      <strong>Preview failed</strong>
      <span>{state.previewRenderStatus.error ?? 'No renderer diagnostic was returned.'}</span>
    </div>
  {:else if status === 'ready' && renderer}
    <div class="preview-through-window" aria-hidden="true"></div>
    <div class="scene-overlay compact">
      <span>current client render</span>
      <span>{renderer.target.width}x{renderer.target.height}</span>
      <span>frame {renderer.frame.frame_index}</span>
    </div>
  {:else if state.project}
    <div class="viewport-empty">
      <strong>Preparing current-client preview</strong>
      <span>Waiting for host preview state.</span>
    </div>
  {:else}
    <div class="viewport-empty">
      <strong>Select a project to start preview</strong>
      <span>Open Fun or choose a recent project.</span>
    </div>
  {/if}
</div>
