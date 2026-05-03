<script lang="ts">
  import type { EditorPaneDockZone, EditorPaneId } from '../types';

  export let paneId: EditorPaneId;
  export let title: string;
  export let subtitle = '';
  export let tone: 'amber' | 'blue' | 'green' | 'gray' | 'red' = 'gray';
  export let zone: EditorPaneDockZone;
  export let onDragStart: (paneId: EditorPaneId, event: DragEvent) => void;
  export let onActivate: (paneId: EditorPaneId) => void;
</script>

<section class={`editor-pane-frame ${paneId} tone-${tone}`} data-pane-id={paneId}>
  <header
    class="editor-pane-tab"
    draggable="true"
    role="tab"
    tabindex="0"
    title="Drag this pane to a side or bottom dock"
    on:dragstart={(event) => onDragStart(paneId, event)}
    on:mousedown={() => onActivate(paneId)}
    on:keydown={(event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        onActivate(paneId);
      }
    }}
  >
    <span class="pane-color-dot"></span>
    <strong>{title}</strong>
    {#if subtitle}
      <small>{subtitle}</small>
    {/if}
    <span class="pane-zone-label">{zone}</span>
  </header>
  <div class="editor-pane-content">
    <slot />
  </div>
</section>
