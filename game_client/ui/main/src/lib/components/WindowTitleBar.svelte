<script lang="ts">
  import { toggleWindowMaximize } from '../windowChrome';
  import { isWindowDragExcluded, startWindowDrag } from '../windowDrag';
  import WindowControls from './WindowControls.svelte';

  export let mode: 'launcher' | 'editor';
  export let title: string;
  export let subtitle = '';
  export let showTabs = false;
  export let canReturnToGame = false;
  export let onDeactivateEditor: (() => void | Promise<void>) | null = null;
  export let onShowLauncher: (() => void | Promise<void>) | null = null;

  function onTitlebarDoubleClick(event: MouseEvent): void {
    if (event.button !== 0 || isWindowDragExcluded(event.target)) {
      return;
    }
    void toggleWindowMaximize();
  }
</script>

<header class={`window-titlebar ${mode}`} aria-label={`${mode} window title bar`}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="titlebar-drag-zone titlebar-identity"
    data-fun-drag-region
    on:pointerdown={startWindowDrag}
    on:dblclick={onTitlebarDoubleClick}
  >
    <strong data-fun-drag-region>{title}</strong>
    {#if subtitle}
      <span data-fun-drag-region>{subtitle}</span>
    {/if}
  </div>

  {#if showTabs}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="titlebar-drag-zone titlebar-tabs"
      data-fun-drag-region
      on:pointerdown={startWindowDrag}
      on:dblclick={onTitlebarDoubleClick}
    >
      <slot name="tabs" />
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="titlebar-drag-zone titlebar-fill"
      data-fun-drag-region
      on:pointerdown={startWindowDrag}
      on:dblclick={onTitlebarDoubleClick}
    ></div>
  {/if}

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="titlebar-drag-zone titlebar-actions"
    data-fun-drag-region
    on:pointerdown={startWindowDrag}
    on:dblclick={onTitlebarDoubleClick}
  >
    <slot name="actions" />
    {#if mode === 'editor' && onShowLauncher}
      <button class="button is-small" type="button" title="Show the launcher surface" on:click={onShowLauncher}>
        Launcher
      </button>
    {/if}
    {#if mode === 'editor' && onDeactivateEditor}
      <button class="button is-small" type="button" title="Return to launcher mode" on:click={onDeactivateEditor}>
        Deactivate
      </button>
    {/if}
  </div>

  <WindowControls {mode} {canReturnToGame} />
</header>
