<script lang="ts">
  import { onMount } from 'svelte';
  import type { EditorUiState, ViewportRect } from '../types';
  import StatusPill from './StatusPill.svelte';

  export let state: EditorUiState;
  export let onResize: (rect: ViewportRect) => void | Promise<void>;
  export let onFocus: () => void | Promise<void>;

  let surface: HTMLDivElement | undefined;
  let observer: ResizeObserver | null = null;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;

  $: viewport = state.liveClientStatus;
  $: canFocus = Boolean(viewport);

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

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="live-client-surface" bind:this={surface} role="presentation" on:click={() => canFocus && onFocus()}>
  <div class="viewport-badges">
    <StatusPill label="context" value="Current Client" />
    <StatusPill label="status" value={viewport?.status ?? 'in host'} tone={viewport?.status === 'running' ? 'good' : 'neutral'} />
    <StatusPill label="input" value={state.activeEditorContextId === 'live_client' ? 'editor overlay' : 'gameplay'} />
  </div>

  {#if viewport}
    <div class="viewport-empty live-placeholder">
      <strong>Current game world</strong>
      <span>The client is this host. Click to return input to gameplay.</span>
    </div>
  {:else}
    <div class="viewport-empty">
      <strong>Current game world</strong>
      <span>Join a game to make the running client state available here.</span>
    </div>
  {/if}
</div>
