<script lang="ts">
  import { onMount } from 'svelte';
  import GraphCanvas from './GraphCanvas.svelte';
  import GraphInspectorPanel from './GraphInspectorPanel.svelte';
  import GraphLibraryPanel from './GraphLibraryPanel.svelte';
  import GraphStatusStrip from './GraphStatusStrip.svelte';
  import { listMaterialShaders, loadMaterialShader, saveMaterialShader } from '../../commands';
  import { searchNodeDefinitions } from '../../graph/nodeRegistry';
  import { defaultShaderRelativePath, graphToWgsl } from '../../graph/wgsl';
  import {
    connectionFromPins,
    createNodeFromType,
    findNode,
    selectedItems,
    validateGraph,
    withSelection
  } from '../../graph/graphModel';
  import {
    addNodeCommand,
    connectPinsCommand,
    deleteSelectionCommand,
    editNodePropertyCommand,
    moveNodesCommand,
    renameNodeCommand,
    replaceGraphCommand,
    UndoStack,
    type GraphCommand
  } from '../../graph/undoStack';
  import {
    type GraphConnection,
    type GraphData,
    type GraphNode,
    type GraphNodeMove,
    type GraphPin,
    type GraphPoint,
    type GraphPropertyValue,
    type GraphSelection,
    type GraphValidationIssue,
    type NodeDefinition
  } from '../../graph/types';
  import { createStarterGraph } from '../../graph/graphModel';
  import type { Diagnostic, MaterialShaderCatalog, ProjectSummary } from '../../types';

  interface GraphClipboardFragment {
    kind: 'fun.graph.fragment';
    version: 1;
    nodes: GraphNode[];
    connections: GraphConnection[];
  }

  interface AddMenuState {
    graphPosition: GraphPoint;
    screenPosition: GraphPoint;
    query: string;
  }

  const pasteOffset = 32;

  export let project: ProjectSummary | null = null;

  let graph = createStarterGraph();
  let validationIssues: GraphValidationIssue[] = validateGraph(graph);
  let savedAt: string | null = null;
  let message: string | null = null;
  let shaderCatalog: MaterialShaderCatalog | null = null;
  let shaderRelativePath = defaultShaderRelativePath(graph.name);
  let wgslSource = graphToWgsl(graph);
  let wgslEdited = false;
  let shaderLoading = false;
  let shaderSaving = false;
  let lastCatalogProjectId: string | null = null;
  let historyRevision = 0;
  let undoStack = new UndoStack();
  let canvasHost: HTMLDivElement | undefined;
  let canvasComponent:
    | {
        focusCanvas: () => void;
        frameSelection: () => void;
      }
    | undefined;
  let addMenu: AddMenuState | null = null;
  let fallbackClipboard = '';

  $: addMenuResults = addMenu ? searchNodeDefinitions(addMenu.query) : [];
  $: canUndo = historyRevision >= 0 && undoStack.canUndo;
  $: canRedo = historyRevision >= 0 && undoStack.canRedo;
  $: generatedWgsl = graphToWgsl(graph);
  $: if (!wgslEdited) {
    wgslSource = generatedWgsl;
  }

  onMount(() => {
    void refreshShaderCatalog();
  });

  $: if (project?.id && project.id !== lastCatalogProjectId) {
    lastCatalogProjectId = project.id;
    void refreshShaderCatalog();
  }

  function executeCommand(command: GraphCommand, validate = true): void {
    graph = undoStack.execute(graph, command);
    historyRevision += 1;
    afterGraphChange(validate);
  }

  function afterGraphChange(validate = true): void {
    if (validate) {
      validationIssues = validateGraph(graph);
    }
    message = null;
  }

  function updateViewState(viewState: GraphData['viewState']): void {
    graph = {
      ...graph,
      viewState
    };
  }

  function updateSelection(selections: GraphSelection[]): void {
    graph = withSelection(graph, selections);
  }

  function previewMoves(moves: GraphNodeMove[]): void {
    const byId = new Map(moves.map((move) => [move.id, move.to]));
    graph = {
      ...graph,
      nodes: graph.nodes.map((node) => {
        const position = byId.get(node.id);
        return position ? { ...node, position: { ...position } } : node;
      })
    };
  }

  function commitMoves(moves: GraphNodeMove[]): void {
    const changedMoves = moves.filter((move) => move.from.x !== move.to.x || move.from.y !== move.to.y);
    if (changedMoves.length > 0) {
      executeCommand(moveNodesCommand(changedMoves), false);
    }
  }

  function createNode(type: string, position = canvasCenterGraphPoint()): void {
    const node = createNodeFromType(graph, type, position);
    if (!node) {
      message = `Unknown node type ${type}.`;
      return;
    }
    executeCommand(addNodeCommand(node));
    addMenu = null;
  }

  function connectPins(first: GraphPin, second: GraphPin): void {
    const connection = connectionFromPins(graph, first, second);
    if (!connection) {
      message = 'Pin types are incompatible.';
      return;
    }
    const toPin = findNode(graph, connection.toNodeId)?.inputs.find((pin) => pin.id === connection.toPinId);
    const replaced = toPin?.isMultiConnect
      ? []
      : graph.connections.filter(
          (candidate) => candidate.toNodeId === connection.toNodeId && candidate.toPinId === connection.toPinId
        );
    executeCommand(connectPinsCommand(connection, replaced));
  }

  function deleteSelection(): void {
    const selections = selectedItems(graph);
    if (selections.length === 0) {
      return;
    }
    const selectedNodeIds = new Set(selections.filter((selection) => selection.kind === 'node').map((selection) => selection.id));
    const selectedConnectionIds = new Set(
      selections.filter((selection) => selection.kind === 'connection').map((selection) => selection.id)
    );
    const nodes = graph.nodes.filter((node) => selectedNodeIds.has(node.id));
    const connections = graph.connections.filter(
      (connection) =>
        selectedConnectionIds.has(connection.id) ||
        selectedNodeIds.has(connection.fromNodeId) ||
        selectedNodeIds.has(connection.toNodeId)
    );
    executeCommand(deleteSelectionCommand(nodes, connections, graph.viewState.selectedIds));
  }

  function renameNode(node: GraphNode, title: string): void {
    const trimmed = title.trim();
    if (!trimmed || trimmed === node.title) {
      return;
    }
    executeCommand(renameNodeCommand(node.id, node.title, trimmed), false);
  }

  function commitProperty(node: GraphNode, property: string, value: GraphPropertyValue): void {
    if (node.properties[property] === value) {
      return;
    }
    executeCommand(editNodePropertyCommand(node.id, property, node.properties[property] ?? null, value));
  }

  function undo(): void {
    graph = undoStack.undo(graph);
    historyRevision += 1;
    validationIssues = validateGraph(graph);
  }

  function redo(): void {
    graph = undoStack.redo(graph);
    historyRevision += 1;
    validationIssues = validateGraph(graph);
  }

  function validateNow(): void {
    validationIssues = validateGraph(graph);
    message = validationIssues.length === 0 ? 'Graph valid.' : 'Validation updated.';
  }

  async function refreshShaderCatalog(): Promise<void> {
    if (!project) {
      shaderCatalog = null;
      message = 'Open a project to list WGSL material shaders.';
      return;
    }
    shaderLoading = true;
    const result = await listMaterialShaders(project.id);
    shaderLoading = false;
    if (!result.ok || !result.value) {
      message = result.diagnostics[0]?.message ?? 'Could not list WGSL material shaders.';
      return;
    }
    shaderCatalog = result.value;
    message = `Found ${result.value.shaders.length} WGSL material shader files.`;
  }

  async function loadNativeShader(relativePath = shaderRelativePath): Promise<void> {
    if (!project) {
      message = 'Open a project to load WGSL material shaders.';
      return;
    }
    shaderLoading = true;
    const result = await loadMaterialShader({ project_id: project.id, relative_path: relativePath });
    shaderLoading = false;
    if (!result.ok || !result.value) {
      message = result.diagnostics[0]?.message ?? 'Could not load WGSL material shader.';
      return;
    }
    shaderRelativePath = result.value.summary.relative_path;
    wgslSource = result.value.source;
    wgslEdited = true;
    validationIssues = [...validateGraph(graph), ...shaderDiagnosticsToIssues(result.value.diagnostics)];
    message = `Loaded ${result.value.summary.relative_path}.`;
  }

  async function saveNativeShader(): Promise<void> {
    if (!project) {
      message = 'Open a project before saving WGSL material shaders.';
      return;
    }
    shaderSaving = true;
    const result = await saveMaterialShader({
      project_id: project.id,
      relative_path: shaderRelativePath,
      source: wgslSource
    });
    shaderSaving = false;
    if (!result.ok || !result.value) {
      message = result.diagnostics[0]?.message ?? 'Could not save WGSL material shader.';
      return;
    }
    shaderRelativePath = result.value.summary.relative_path;
    wgslSource = result.value.source;
    wgslEdited = true;
    savedAt = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    validationIssues = [...validateGraph(graph), ...shaderDiagnosticsToIssues(result.value.diagnostics)];
    message = `Saved native WGSL file ${result.value.summary.relative_path}.`;
    await refreshShaderCatalog();
  }

  function syncWgslFromGraph(): void {
    wgslSource = generatedWgsl;
    wgslEdited = false;
    message = 'WGSL source regenerated from graph.';
  }

  async function copyWgsl(): Promise<void> {
    fallbackClipboard = wgslSource;
    try {
      await navigator.clipboard?.writeText(wgslSource);
      message = 'Copied WGSL source.';
    } catch {
      message = 'WGSL source staged in session clipboard.';
    }
  }

  async function pasteWgsl(): Promise<void> {
    let raw = fallbackClipboard;
    try {
      raw = (await navigator.clipboard?.readText()) || raw;
    } catch {
      // Session fallback stays available when browser clipboard read is blocked.
    }
    if (!raw.trim()) {
      message = 'Clipboard has no WGSL source.';
      return;
    }
    wgslSource = raw;
    wgslEdited = true;
    message = 'Pasted WGSL source.';
  }

  function duplicateSelection(): void {
    const fragment = selectedFragment();
    if (!fragment) {
      return;
    }
    pasteFragment(fragment);
  }

  function pasteFragment(fragment: GraphClipboardFragment): void {
    const idMap = new Map<string, string>();
    const nodes = fragment.nodes.map((node) => {
      const nextId = nextNodeId(node.id, idMap);
      return {
        ...structuredClone(node),
        id: nextId,
        position: {
          x: node.position.x + pasteOffset,
          y: node.position.y + pasteOffset
        },
        inputs: node.inputs.map((pin) => ({ ...pin, nodeId: nextId })),
        outputs: node.outputs.map((pin) => ({ ...pin, nodeId: nextId }))
      };
    });
    const connections = fragment.connections
      .filter((connection) => idMap.has(connection.fromNodeId) && idMap.has(connection.toNodeId))
      .map((connection, index) => ({
        ...structuredClone(connection),
        id: uniqueConnectionId(`conn_paste_${index}`),
        fromNodeId: idMap.get(connection.fromNodeId) ?? connection.fromNodeId,
        toNodeId: idMap.get(connection.toNodeId) ?? connection.toNodeId
      }));
    const next = withSelection(
      {
        ...graph,
        nodes: [...graph.nodes, ...nodes],
        connections: [...graph.connections, ...connections]
      },
      nodes.map((node) => ({ kind: 'node', id: node.id }))
    );
    graph = undoStack.execute(graph, replaceGraphCommand(graph, next, 'Paste nodes'));
    historyRevision += 1;
    validationIssues = validateGraph(graph);
  }

  function arrangeSelection(): void {
    const nodeIds = new Set(selectedItems(graph).filter((selection) => selection.kind === 'node').map((selection) => selection.id));
    const nodes = graph.nodes.filter((node) => nodeIds.size === 0 || nodeIds.has(node.id));
    if (nodes.length <= 1) {
      return;
    }
    const sorted = [...nodes].sort((a, b) => a.position.x - b.position.x || a.position.y - b.position.y);
    const startX = Math.min(...sorted.map((node) => node.position.x));
    const startY = Math.min(...sorted.map((node) => node.position.y));
    const moves = sorted.map((node, index) => ({
      id: node.id,
      from: { ...node.position },
      to: {
        x: startX + (index % 3) * 280,
        y: startY + Math.floor(index / 3) * 150
      }
    }));
    executeCommand(moveNodesCommand(moves), false);
  }

  function frameSelection(): void {
    canvasComponent?.frameSelection();
  }

  function openCreateMenu(position: GraphPoint, screenPosition: GraphPoint): void {
    addMenu = { graphPosition: position, screenPosition, query: '' };
  }

  function createFromMenu(definition: NodeDefinition): void {
    if (!addMenu) {
      return;
    }
    createNode(definition.type, addMenu.graphPosition);
  }

  function closeMenu(): void {
    addMenu = null;
  }

  function setGraphName(name: string): void {
    graph = {
      ...graph,
      name: name.trim() || graph.name
    };
    if (!wgslEdited) {
      shaderRelativePath = defaultShaderRelativePath(name);
    }
  }

  function setGraphDescription(description: string): void {
    graph = {
      ...graph,
      metadata: {
        ...graph.metadata,
        description
      }
    };
  }

  function setShaderPath(relativePath: string): void {
    shaderRelativePath = relativePath.trim().replaceAll('\\', '/') || defaultShaderRelativePath(graph.name);
  }

  function editWgslSource(source: string): void {
    wgslSource = source;
    wgslEdited = source !== generatedWgsl;
  }

  function shaderDiagnosticsToIssues(diagnostics: Diagnostic[]): GraphValidationIssue[] {
    return diagnostics.map((diagnostic, index) => ({
      id: `wgsl:${diagnostic.code}:${index}`,
      level: diagnostic.level === 'error' ? 'error' : diagnostic.level === 'warning' ? 'warning' : 'info',
      message: diagnostic.message
    }));
  }

  function togglePanel(panel: keyof GraphData['viewState']['collapsedPanels']): void {
    graph = {
      ...graph,
      viewState: {
        ...graph.viewState,
        collapsedPanels: {
          ...graph.viewState.collapsedPanels,
          [panel]: !graph.viewState.collapsedPanels[panel]
        }
      }
    };
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (isTextInput(event.target)) {
      return;
    }
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && key === 'z' && !event.shiftKey) {
      event.preventDefault();
      undo();
    } else if ((event.ctrlKey || event.metaKey) && (key === 'y' || (key === 'z' && event.shiftKey))) {
      event.preventDefault();
      redo();
    } else if ((event.ctrlKey || event.metaKey) && key === 'c') {
      event.preventDefault();
      void copyWgsl();
    } else if ((event.ctrlKey || event.metaKey) && key === 'v') {
      event.preventDefault();
      void pasteWgsl();
    } else if ((event.ctrlKey || event.metaKey) && key === 'd') {
      event.preventDefault();
      duplicateSelection();
    } else if (event.key === 'Delete' || event.key === 'Backspace') {
      event.preventDefault();
      deleteSelection();
    } else if (key === 'f') {
      event.preventDefault();
      frameSelection();
    } else if (key === 'a' && !event.ctrlKey && !event.metaKey) {
      event.preventDefault();
      openCreateMenu(canvasCenterGraphPoint(), canvasCenterScreenPoint());
    } else if (event.key === 'Escape') {
      closeMenu();
    }
  }

  function handleDrop(event: DragEvent): void {
    event.preventDefault();
    const type = event.dataTransfer?.getData('application/x-fun-graph-node') || event.dataTransfer?.getData('text/plain');
    if (!type) {
      return;
    }
    createNode(type, clientToGraphPoint(event.clientX, event.clientY));
  }

  function selectedFragment(): GraphClipboardFragment | null {
    const selections = selectedItems(graph);
    const selectedNodeIds = new Set(selections.filter((selection) => selection.kind === 'node').map((selection) => selection.id));
    if (selectedNodeIds.size === 0) {
      return null;
    }
    return {
      kind: 'fun.graph.fragment',
      version: 1,
      nodes: graph.nodes.filter((node) => selectedNodeIds.has(node.id)).map((node) => structuredClone(node)),
      connections: graph.connections
        .filter((connection) => selectedNodeIds.has(connection.fromNodeId) && selectedNodeIds.has(connection.toNodeId))
        .map((connection) => structuredClone(connection))
    };
  }

  function nextNodeId(sourceId: string, idMap: Map<string, string>): string {
    let index = graph.nodes.length + idMap.size + 1;
    let id = `${sourceId}_copy`;
    const ids = new Set([...graph.nodes.map((node) => node.id), ...idMap.values()]);
    while (ids.has(id)) {
      index += 1;
      id = `${sourceId}_copy_${index.toString(36)}`;
    }
    idMap.set(sourceId, id);
    return id;
  }

  function uniqueConnectionId(seed: string): string {
    const ids = new Set(graph.connections.map((connection) => connection.id));
    let id = seed;
    let index = 1;
    while (ids.has(id)) {
      index += 1;
      id = `${seed}_${index}`;
    }
    return id;
  }

  function canvasCenterGraphPoint(): GraphPoint {
    const center = canvasCenterScreenPoint();
    return {
      x: (center.x - graph.viewState.panX) / graph.viewState.zoom,
      y: (center.y - graph.viewState.panY) / graph.viewState.zoom
    };
  }

  function canvasCenterScreenPoint(): GraphPoint {
    const rect = canvasHost?.getBoundingClientRect();
    return {
      x: (rect?.width ?? 900) / 2,
      y: (rect?.height ?? 520) / 2
    };
  }

  function clientToGraphPoint(clientX: number, clientY: number): GraphPoint {
    const rect = canvasHost?.getBoundingClientRect();
    return {
      x: (clientX - (rect?.left ?? 0) - graph.viewState.panX) / graph.viewState.zoom,
      y: (clientY - (rect?.top ?? 0) - graph.viewState.panY) / graph.viewState.zoom
    };
  }

  function isTextInput(target: EventTarget | null): boolean {
    return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement;
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<section
  class:left-collapsed={graph.viewState.collapsedPanels.left}
  class:right-collapsed={graph.viewState.collapsedPanels.right}
  class="graph-tab"
>
  <GraphLibraryPanel
    {graph}
    collapsed={graph.viewState.collapsedPanels.left}
    onToggle={() => togglePanel('left')}
    onCreateNode={createNode}
  />

  <div
    class="graph-center"
    role="region"
    aria-label="Graph canvas"
    bind:this={canvasHost}
    on:dragover|preventDefault
    on:drop={handleDrop}
  >
    <GraphCanvas
      bind:this={canvasComponent}
      {graph}
      {validationIssues}
      onViewState={updateViewState}
      onSelection={updateSelection}
      onMovePreview={previewMoves}
      onMoveCommit={commitMoves}
      onConnectPins={connectPins}
      onOpenCreateMenu={openCreateMenu}
    />

    {#if addMenu}
      <div
        class="graph-add-menu"
        style={`left: ${addMenu.screenPosition.x}px; top: ${addMenu.screenPosition.y}px;`}
      >
        <input
          class="input is-small"
          type="search"
          placeholder="Search nodes"
          bind:value={addMenu.query}
        />
        <div class="graph-add-menu-list">
          {#each addMenuResults.slice(0, 10) as definition (definition.type)}
            <button type="button" on:click={() => createFromMenu(definition)}>
              <strong>{definition.title}</strong>
              <small>{definition.category}</small>
            </button>
          {:else}
            <p class="muted">No matching nodes.</p>
          {/each}
        </div>
      </div>
      <button class="graph-menu-scrim" type="button" aria-label="Close node menu" on:click={closeMenu}></button>
    {/if}

    <section class="graph-wgsl-panel">
      <div class="graph-wgsl-header">
        <div>
          <span class="eyebrow">WGSL Material Shader</span>
          <strong>{wgslEdited ? 'native source edited' : 'generated from graph'}</strong>
        </div>
        <label class="field graph-shader-path">
          <span class="label">Path</span>
          <input
            class="input is-small"
            value={shaderRelativePath}
            spellcheck="false"
            on:change={(event) => setShaderPath(event.currentTarget.value)}
          />
        </label>
        <span class="select is-small graph-shader-select">
          <select
            value={shaderRelativePath}
            disabled={!shaderCatalog || shaderCatalog.shaders.length === 0 || shaderLoading}
            on:change={(event) => loadNativeShader(event.currentTarget.value)}
          >
            <option value={shaderRelativePath}>{shaderCatalog?.shaders.length ? 'Load shader file' : 'No WGSL files found'}</option>
            {#each shaderCatalog?.shaders ?? [] as shader (shader.id)}
              <option value={shader.relative_path}>{shader.relative_path}</option>
            {/each}
          </select>
        </span>
        <button class="button is-small" type="button" on:click={syncWgslFromGraph}>Sync From Graph</button>
        <button class="button is-small is-primary" type="button" on:click={saveNativeShader} disabled={!project || shaderSaving}>
          {shaderSaving ? 'Saving' : 'Save WGSL'}
        </button>
      </div>
      <textarea
        class="textarea graph-wgsl-source mono"
        spellcheck="false"
        value={wgslSource}
        on:input={(event) => editWgslSource(event.currentTarget.value)}
      ></textarea>
    </section>

    <GraphStatusStrip
      {graph}
      {validationIssues}
      expanded={graph.viewState.collapsedPanels.status}
      {savedAt}
      {message}
      shaderPath={shaderRelativePath}
      {canUndo}
      {canRedo}
      onToggle={() => togglePanel('status')}
      onValidate={validateNow}
      onSaveNative={saveNativeShader}
      onRefreshShaders={refreshShaderCatalog}
      onUndo={undo}
      onRedo={redo}
      onCopyWgsl={copyWgsl}
      onPasteWgsl={pasteWgsl}
    />
  </div>

  <GraphInspectorPanel
    {graph}
    {validationIssues}
    collapsed={graph.viewState.collapsedPanels.right}
    onToggle={() => togglePanel('right')}
    onRenameNode={renameNode}
    onPropertyCommit={commitProperty}
    onDeleteSelection={deleteSelection}
    onDuplicateSelection={duplicateSelection}
    onArrangeSelection={arrangeSelection}
    onGraphName={setGraphName}
    onGraphDescription={setGraphDescription}
  />
</section>
