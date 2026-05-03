<script lang="ts">
  import { editorStore } from '../stores/editor';
  import type { EditorContextId, EditorPaneDockZone, EditorPaneId } from '../types';
  import DetailsPane from './DetailsPane.svelte';
  import EditorPaneFrame from './EditorPaneFrame.svelte';
  import EditorTitleBar from './EditorTitleBar.svelte';
  import GraphTab from './graph/GraphTab.svelte';
  import LogPanel from './LogPanel.svelte';
  import OverviewPanel from './OverviewPanel.svelte';
  import ViewportPanel from './ViewportPanel.svelte';

  const paneOrder: EditorPaneId[] = ['details', 'overview', 'viewport', 'graph', 'log'];

  let paneZones: Record<EditorPaneId, EditorPaneDockZone> = {
    details: 'left',
    overview: 'left',
    viewport: 'center',
    graph: 'center',
    log: 'bottom'
  };
  let activePane: EditorPaneId = 'viewport';
  let draggingPane: EditorPaneId | null = null;
  let dragOverZone: EditorPaneDockZone | null = null;
  let leftWidth = 380;
  let bottomHeight = 230;

  $: leftPanes = panesInZone('left');
  $: centerPanes = panesInZone('center');
  $: rightPanes = panesInZone('right');
  $: bottomPanes = panesInZone('bottom');
  $: workspaceStyle = `--left-pane-width: ${leftWidth}px; --bottom-pane-height: ${bottomHeight}px;`;

  function shortcutOwnsInput(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) {
      return false;
    }
    return (
      target.isContentEditable ||
      target.closest('input, textarea, select, [role="textbox"], [data-commandbar-confirmation]') !== null
    );
  }

  function onShellKeydown(event: KeyboardEvent): void {
    if ($editorStore.launcherMode !== 'editor') {
      return;
    }
    const key = event.key.toLowerCase();
    if (event.ctrlKey && !event.altKey && !event.metaKey && key === ' ') {
      if (shortcutOwnsInput(event.target)) {
        return;
      }
      event.preventDefault();
      dockPane('log', paneZones.log === 'bottom' ? 'center' : 'bottom');
      editorStore.setActiveEditorContext('diagnostics');
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && key === 'k') {
      event.preventDefault();
      editorStore.focusCommandbar();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && !event.altKey && key === 'p') {
      event.preventDefault();
      editorStore.focusCommandbar();
    }
  }

  function panesInZone(zone: EditorPaneDockZone): EditorPaneId[] {
    return paneOrder.filter((paneId) => paneZones[paneId] === zone);
  }

  function titleForPane(paneId: EditorPaneId): string {
    if (paneId === 'viewport') {
      return 'Viewport';
    }
    if (paneId === 'graph') {
      return 'Graph';
    }
    if (paneId === 'log') {
      return 'Log';
    }
    if (paneId === 'details') {
      return 'Details';
    }
    return 'Overview';
  }

  function subtitleForPane(paneId: EditorPaneId): string {
    if (paneId === 'viewport') {
      return $editorStore.editorContext === 'live_client' ? 'client' : 'preview';
    }
    if (paneId === 'graph') {
      return 'material shader';
    }
    if (paneId === 'log') {
      return `${$editorStore.diagnostics.length} rows`;
    }
    if (paneId === 'details') {
      return $editorStore.overview.selectedItem?.kind ?? $editorStore.entities.selectedDetails?.row.domain ?? 'selection';
    }
    return $editorStore.project?.display_name ?? 'project';
  }

  function toneForPane(paneId: EditorPaneId): 'amber' | 'blue' | 'green' | 'gray' | 'red' {
    if (paneId === 'graph' || paneId === 'details') {
      return 'amber';
    }
    if (paneId === 'viewport') {
      return 'blue';
    }
    if (paneId === 'log') {
      return $editorStore.diagnosticsView.unreadCriticalCount > 0 ? 'red' : 'green';
    }
    return 'gray';
  }

  function contextForPane(paneId: EditorPaneId): EditorContextId {
    if (paneId === 'graph') {
      return 'graph';
    }
    if (paneId === 'log') {
      return 'diagnostics';
    }
    if (paneId === 'viewport') {
      return $editorStore.editorContext === 'live_client' || $editorStore.editorContext === 'server'
        ? $editorStore.editorContext
        : 'preview';
    }
    return 'overview';
  }

  function activatePane(paneId: EditorPaneId): void {
    activePane = paneId;
    editorStore.setActiveEditorContext(contextForPane(paneId));
  }

  function startPaneDrag(paneId: EditorPaneId, event: DragEvent): void {
    draggingPane = paneId;
    activePane = paneId;
    event.dataTransfer?.setData('text/plain', paneId);
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = 'move';
    }
  }

  function paneIdFromDrop(event: DragEvent): EditorPaneId | null {
    const dropped = event.dataTransfer?.getData('text/plain') as EditorPaneId | undefined;
    return dropped && paneOrder.includes(dropped) ? dropped : draggingPane;
  }

  function dockPane(paneId: EditorPaneId, zone: EditorPaneDockZone): void {
    paneZones = { ...paneZones, [paneId]: zone };
    activePane = paneId;
    dragOverZone = null;
    draggingPane = null;
  }

  function dropPane(zone: EditorPaneDockZone, event: DragEvent): void {
    event.preventDefault();
    const paneId = paneIdFromDrop(event);
    if (paneId) {
      dockPane(paneId, zone);
    }
  }

  function dragOver(zone: EditorPaneDockZone, event: DragEvent): void {
    event.preventDefault();
    dragOverZone = zone;
  }

  function endDrag(): void {
    draggingPane = null;
    dragOverZone = null;
  }

  function startLeftResize(event: PointerEvent): void {
    const startX = event.clientX;
    const startWidth = leftWidth;
    const move = (moveEvent: PointerEvent): void => {
      leftWidth = Math.round(Math.min(560, Math.max(240, startWidth + moveEvent.clientX - startX)));
    };
    const up = (): void => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  function startBottomResize(event: PointerEvent): void {
    const startY = event.clientY;
    const startHeight = bottomHeight;
    const move = (moveEvent: PointerEvent): void => {
      bottomHeight = Math.round(Math.min(window.innerHeight * 0.46, Math.max(120, startHeight - moveEvent.clientY + startY)));
    };
    const up = (): void => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }
</script>

<svelte:window on:keydown={onShellKeydown} on:dragend={endDrag} />

<main class="editor-shell">
  <EditorTitleBar
    state={$editorStore}
    onShowLauncher={editorStore.showLauncherShell}
    onDeactivateEditor={editorStore.deactivateEditorShell}
    onCommandInput={editorStore.setCommandbarInput}
    onCommandFocus={editorStore.focusCommandbar}
    onCommandBlur={editorStore.blurCommandbar}
    onCommandClear={editorStore.clearCommandbar}
    onCommandMove={editorStore.moveCommandbarSelection}
    onCommandExecute={editorStore.executeCommandbarResult}
    onToggleAccount={editorStore.toggleAccountPanel}
    onCloseAccount={editorStore.closeAccountPanel}
    onRequestAccountTicket={editorStore.requestAccountTicket}
    onRefreshAccountTicket={editorStore.refreshAccountTicket}
    onLogoutAccount={editorStore.logoutAccount}
  />

  {#if $editorStore.error}
    <section class="notification is-danger is-light shell-error">{$editorStore.error}</section>
  {/if}

  <section
    class:has-right-dock={rightPanes.length > 0}
    class="advanced-workspace"
    style={workspaceStyle}
    aria-label="Dockable editor panes"
  >
    <section
      class:drag-over={dragOverZone === 'left'}
      class="pane-drop-zone pane-zone-left"
      aria-label="Left pane dock"
      on:dragover={(event) => dragOver('left', event)}
      on:dragleave={() => (dragOverZone = null)}
      on:drop={(event) => dropPane('left', event)}
    >
      {#each leftPanes as paneId (paneId)}
        <EditorPaneFrame
          {paneId}
          title={titleForPane(paneId)}
          subtitle={subtitleForPane(paneId)}
          tone={toneForPane(paneId)}
          zone={paneZones[paneId]}
          onDragStart={startPaneDrag}
          onActivate={activatePane}
        >
          {#if paneId === 'overview'}
            <OverviewPanel
              state={$editorStore}
              onRange={editorStore.ensureEntityRange}
              onSelectEntity={editorStore.selectEntity}
              onSearch={editorStore.setEntitySearch}
              onDomain={editorStore.setEntityDomain}
              onSource={editorStore.setEntitySource}
              onSelectOverview={editorStore.selectOverviewItem}
            />
          {:else if paneId === 'details'}
            <DetailsPane state={$editorStore} onPatchTransform={editorStore.patchSelectedTransform} />
          {/if}
        </EditorPaneFrame>
      {/each}
    </section>

    <button class="pane-resize-handle vertical" type="button" aria-label="Resize left panes" on:pointerdown={startLeftResize}></button>

    <section
      class:drag-over={dragOverZone === 'center'}
      class={`pane-drop-zone pane-zone-center active-${activePane}`}
      aria-label="Center pane dock"
      on:dragover={(event) => dragOver('center', event)}
      on:dragleave={() => (dragOverZone = null)}
      on:drop={(event) => dropPane('center', event)}
    >
      {#each centerPanes as paneId (paneId)}
        <EditorPaneFrame
          {paneId}
          title={titleForPane(paneId)}
          subtitle={subtitleForPane(paneId)}
          tone={toneForPane(paneId)}
          zone={paneZones[paneId]}
          onDragStart={startPaneDrag}
          onActivate={activatePane}
        >
          {#if paneId === 'viewport'}
            <ViewportPanel
              state={$editorStore}
              onFocus={editorStore.focusClient}
              onResize={editorStore.reportViewportBounds}
            />
          {:else if paneId === 'graph'}
            <section class="graph-workspace panel" aria-label="Material graph">
              <GraphTab project={$editorStore.project} />
            </section>
          {:else if paneId === 'log'}
            <LogPanel state={$editorStore} />
          {:else if paneId === 'overview'}
            <OverviewPanel
              state={$editorStore}
              onRange={editorStore.ensureEntityRange}
              onSelectEntity={editorStore.selectEntity}
              onSearch={editorStore.setEntitySearch}
              onDomain={editorStore.setEntityDomain}
              onSource={editorStore.setEntitySource}
              onSelectOverview={editorStore.selectOverviewItem}
            />
          {:else if paneId === 'details'}
            <DetailsPane state={$editorStore} onPatchTransform={editorStore.patchSelectedTransform} />
          {/if}
        </EditorPaneFrame>
      {/each}
    </section>

    {#if rightPanes.length > 0}
      <section
        class:drag-over={dragOverZone === 'right'}
        class="pane-drop-zone pane-zone-right"
        aria-label="Right pane dock"
        on:dragover={(event) => dragOver('right', event)}
        on:dragleave={() => (dragOverZone = null)}
        on:drop={(event) => dropPane('right', event)}
      >
        {#each rightPanes as paneId (paneId)}
          <EditorPaneFrame
            {paneId}
            title={titleForPane(paneId)}
            subtitle={subtitleForPane(paneId)}
            tone={toneForPane(paneId)}
            zone={paneZones[paneId]}
            onDragStart={startPaneDrag}
            onActivate={activatePane}
          >
            {#if paneId === 'log'}
              <LogPanel state={$editorStore} />
            {:else if paneId === 'details'}
              <DetailsPane state={$editorStore} onPatchTransform={editorStore.patchSelectedTransform} />
            {/if}
          </EditorPaneFrame>
        {/each}
      </section>
    {/if}

    <button class="pane-resize-handle horizontal" type="button" aria-label="Resize bottom panes" on:pointerdown={startBottomResize}></button>

    <section
      class:drag-over={dragOverZone === 'bottom'}
      class="pane-drop-zone pane-zone-bottom"
      aria-label="Bottom pane dock"
      on:dragover={(event) => dragOver('bottom', event)}
      on:dragleave={() => (dragOverZone = null)}
      on:drop={(event) => dropPane('bottom', event)}
    >
      {#each bottomPanes as paneId (paneId)}
        <EditorPaneFrame
          {paneId}
          title={titleForPane(paneId)}
          subtitle={subtitleForPane(paneId)}
          tone={toneForPane(paneId)}
          zone={paneZones[paneId]}
          onDragStart={startPaneDrag}
          onActivate={activatePane}
        >
          {#if paneId === 'log'}
            <LogPanel state={$editorStore} />
          {:else if paneId === 'graph'}
            <section class="graph-workspace panel" aria-label="Material graph">
              <GraphTab project={$editorStore.project} />
            </section>
          {:else if paneId === 'overview'}
            <OverviewPanel
              state={$editorStore}
              onRange={editorStore.ensureEntityRange}
              onSelectEntity={editorStore.selectEntity}
              onSearch={editorStore.setEntitySearch}
              onDomain={editorStore.setEntityDomain}
              onSource={editorStore.setEntitySource}
              onSelectOverview={editorStore.selectOverviewItem}
            />
          {:else if paneId === 'details'}
            <DetailsPane state={$editorStore} onPatchTransform={editorStore.patchSelectedTransform} />
          {:else if paneId === 'viewport'}
            <ViewportPanel
              state={$editorStore}
              onFocus={editorStore.focusClient}
              onResize={editorStore.reportViewportBounds}
            />
          {/if}
        </EditorPaneFrame>
      {/each}
    </section>
  </section>
</main>
