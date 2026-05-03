<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { getNodeDefinition, pinTypeColors, pinTypeLabels } from '../../graph/nodeRegistry';
  import {
    pinRefsForConnection,
    pinsCanConnect,
    selectedItems
  } from '../../graph/graphModel';
  import type {
    GraphConnection,
    GraphData,
    GraphNode,
    GraphNodeMove,
    GraphPin,
    GraphPoint,
    GraphSelection,
    GraphValidationIssue,
    GraphViewState
  } from '../../graph/types';

  export let graph: GraphData;
  export let validationIssues: GraphValidationIssue[];
  export let onViewState: (viewState: GraphViewState) => void;
  export let onSelection: (selections: GraphSelection[]) => void;
  export let onMovePreview: (moves: GraphNodeMove[]) => void;
  export let onMoveCommit: (moves: GraphNodeMove[]) => void;
  export let onConnectPins: (first: GraphPin, second: GraphPin) => void;
  export let onOpenCreateMenu: (position: GraphPoint, screen: GraphPoint) => void;

  interface PinHit {
    node: GraphNode;
    pin: GraphPin;
    point: GraphPoint;
  }

  interface ConnectionPathCache {
    signature: string;
    path: Path2D;
    samples: GraphPoint[];
  }

  type DragState =
    | {
        kind: 'pan';
        pointerId: number;
        start: GraphPoint;
        panX: number;
        panY: number;
      }
    | {
        kind: 'node';
        pointerId: number;
        start: GraphPoint;
        initial: Map<string, GraphPoint>;
        current: GraphNodeMove[];
        moved: boolean;
      }
    | {
        kind: 'connect';
        pointerId: number;
        from: PinHit;
        current: GraphPoint;
      }
    | {
        kind: 'box';
        pointerId: number;
        startScreen: GraphPoint;
        currentScreen: GraphPoint;
      };

  const nodeHeaderHeight = 30;
  const pinRowHeight = 22;

  let canvas: HTMLCanvasElement | undefined;
  let wrapper: HTMLDivElement | undefined;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrame = 0;
  let width = 1;
  let height = 1;
  let drag: DragState | null = null;
  let hoveredPin: PinHit | null = null;
  let hoveredConnectionId: string | null = null;
  let connectionPathCache = new Map<string, ConnectionPathCache>();

  $: nodeIssues = groupNodeIssues(validationIssues);
  $: connectionIssues = groupConnectionIssues(validationIssues);
  $: selected = selectedItems(graph);
  $: selectedNodeIds = new Set(selected.filter((item) => item.kind === 'node').map((item) => item.id));
  $: selectedConnectionIds = new Set(
    selected.filter((item) => item.kind === 'connection').map((item) => item.id)
  );
  $: graph, validationIssues, scheduleDraw();

  onMount(() => {
    measure();
    resizeObserver = new ResizeObserver(measure);
    if (wrapper) {
      resizeObserver.observe(wrapper);
    }
    scheduleDraw();
  });

  onDestroy(() => {
    resizeObserver?.disconnect();
    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
    }
  });

  export function focusCanvas(): void {
    canvas?.focus();
  }

  export function frameSelection(): void {
    const targets = selectedNodeIds.size > 0 ? graph.nodes.filter((node) => selectedNodeIds.has(node.id)) : graph.nodes;
    if (targets.length === 0 || !wrapper) {
      return;
    }
    const bounds = nodeBounds(targets);
    const nextZoom = clamp(Math.min(width / Math.max(bounds.width + 160, 1), height / Math.max(bounds.height + 160, 1)), 0.25, 1.4);
    onViewState({
      ...graph.viewState,
      zoom: nextZoom,
      panX: width / 2 - (bounds.x + bounds.width / 2) * nextZoom,
      panY: height / 2 - (bounds.y + bounds.height / 2) * nextZoom
    });
  }

  function measure(): void {
    if (!wrapper || !canvas) {
      return;
    }
    width = Math.max(1, wrapper.clientWidth);
    height = Math.max(1, wrapper.clientHeight);
    const scale = window.devicePixelRatio || 1;
    canvas.width = Math.round(width * scale);
    canvas.height = Math.round(height * scale);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    scheduleDraw();
  }

  function scheduleDraw(): void {
    if (!canvas || animationFrame) {
      return;
    }
    animationFrame = requestAnimationFrame(() => {
      animationFrame = 0;
      draw();
    });
  }

  function draw(): void {
    if (!canvas) {
      return;
    }
    const context = canvas.getContext('2d');
    if (!context) {
      return;
    }
    const scale = window.devicePixelRatio || 1;
    context.setTransform(scale, 0, 0, scale, 0, 0);
    context.clearRect(0, 0, width, height);
    drawGrid(context);

    const visibleNodes = cullVisibleNodes();
    context.save();
    context.translate(graph.viewState.panX, graph.viewState.panY);
    context.scale(graph.viewState.zoom, graph.viewState.zoom);
    drawConnections(context, visibleNodes);
    drawNodes(context, visibleNodes);
    drawPreviewWire(context);
    context.restore();
    drawSelectionBox(context);
    drawEmptyState(context);
  }

  function drawGrid(context: CanvasRenderingContext2D): void {
    context.fillStyle = '#070a0d';
    context.fillRect(0, 0, width, height);

    const zoom = graph.viewState.zoom;
    const majorStep = 96 * zoom;
    const minorStep = 24 * zoom;
    const offsetX = positiveModulo(graph.viewState.panX, majorStep);
    const offsetY = positiveModulo(graph.viewState.panY, majorStep);

    if (minorStep >= 10) {
      context.strokeStyle = 'rgba(126, 144, 157, 0.10)';
      context.lineWidth = 1;
      context.beginPath();
      for (let x = positiveModulo(graph.viewState.panX, minorStep); x < width; x += minorStep) {
        context.moveTo(x, 0);
        context.lineTo(x, height);
      }
      for (let y = positiveModulo(graph.viewState.panY, minorStep); y < height; y += minorStep) {
        context.moveTo(0, y);
        context.lineTo(width, y);
      }
      context.stroke();
    }

    context.strokeStyle = 'rgba(126, 144, 157, 0.22)';
    context.lineWidth = 1;
    context.beginPath();
    for (let x = offsetX; x < width; x += majorStep) {
      context.moveTo(x, 0);
      context.lineTo(x, height);
    }
    for (let y = offsetY; y < height; y += majorStep) {
      context.moveTo(0, y);
      context.lineTo(width, y);
    }
    context.stroke();
  }

  function drawConnections(context: CanvasRenderingContext2D, visibleNodes: GraphNode[]): void {
    const visibleNodeIds = new Set(visibleNodes.map((node) => node.id));
    for (const connection of graph.connections) {
      const refs = pinRefsForConnection(graph, connection);
      if (!refs.fromNode || !refs.toNode || !refs.fromPin || !refs.toPin) {
        continue;
      }
      if (!visibleNodeIds.has(refs.fromNode.id) && !visibleNodeIds.has(refs.toNode.id)) {
        continue;
      }

      const cached = connectionPath(connection, refs.fromNode, refs.fromPin, refs.toNode, refs.toPin);
      const hasIssue = connectionIssues.has(connection.id);
      const selectedConnection = selectedConnectionIds.has(connection.id);
      const hovered = hoveredConnectionId === connection.id;
      context.strokeStyle = hasIssue ? '#d56f66' : selectedConnection || hovered ? '#d8e0e6' : '#6da8db';
      context.lineWidth = (selectedConnection || hovered ? 3 : 2) / graph.viewState.zoom;
      context.stroke(cached.path);
    }
  }

  function drawNodes(context: CanvasRenderingContext2D, visibleNodes: GraphNode[]): void {
    for (const node of visibleNodes) {
      const definition = getNodeDefinition(node.type);
      const issues = nodeIssues.get(node.id) ?? [];
      const selectedNode = selectedNodeIds.has(node.id);
      const category = definition?.category ?? 'Utility';
      const headerColor = categoryColor(category);

      context.fillStyle = '#11171c';
      roundedRect(context, node.position.x, node.position.y, node.size.width, node.size.height, 6);
      context.fill();
      context.strokeStyle = issues.some((issue) => issue.level === 'error')
        ? '#d56f66'
        : selectedNode
          ? '#d8e0e6'
          : '#2c3a43';
      context.lineWidth = (selectedNode ? 2.2 : 1) / graph.viewState.zoom;
      context.stroke();

      context.fillStyle = headerColor;
      roundedRect(context, node.position.x, node.position.y, node.size.width, 30, 6);
      context.fill();
      context.fillStyle = '#f4f7f9';
      context.font = '700 12px Inter, Segoe UI, sans-serif';
      context.textBaseline = 'middle';
      context.fillText(ellipsis(context, node.title, node.size.width - 44), node.position.x + 10, node.position.y + 15);

      if (issues.length > 0) {
        context.fillStyle = issues.some((issue) => issue.level === 'error') ? '#d56f66' : '#d7a85b';
        context.beginPath();
        context.arc(node.position.x + node.size.width - 16, node.position.y + 15, 6, 0, Math.PI * 2);
        context.fill();
      }

      if (graph.viewState.zoom >= 0.42) {
        drawPins(context, node);
      }
      if (graph.viewState.zoom >= 0.68) {
        drawPropertyPreview(context, node);
      }
    }
  }

  function drawPins(context: CanvasRenderingContext2D, node: GraphNode): void {
    context.font = '11px Inter, Segoe UI, sans-serif';
    context.textBaseline = 'middle';
    drawPinGroup(context, node, node.inputs, 'input');
    drawPinGroup(context, node, node.outputs, 'output');
  }

  function drawPinGroup(
    context: CanvasRenderingContext2D,
    node: GraphNode,
    pins: GraphPin[],
    direction: 'input' | 'output'
  ): void {
    for (const pin of pins) {
      const point = pinPoint(node, pin);
      const compatible = drag?.kind === 'connect' ? pinsCanConnectPair(drag.from.pin, pin) : true;
      context.fillStyle = compatible ? pinTypeColors[pin.type] : '#3d454b';
      context.beginPath();
      context.arc(point.x, point.y, 5, 0, Math.PI * 2);
      context.fill();
      context.strokeStyle = hoveredPin?.pin.id === pin.id && hoveredPin.node.id === node.id ? '#ffffff' : '#0a0d10';
      context.lineWidth = 1 / graph.viewState.zoom;
      context.stroke();

      context.fillStyle = compatible ? '#cfd8df' : '#59656e';
      const label = `${pin.name} ${pinTypeLabels[pin.type]}`;
      if (direction === 'input') {
        context.textAlign = 'left';
        context.fillText(ellipsis(context, label, node.size.width * 0.46), node.position.x + 14, point.y);
      } else {
        context.textAlign = 'right';
        context.fillText(ellipsis(context, label, node.size.width * 0.46), node.position.x + node.size.width - 14, point.y);
      }
    }
  }

  function drawPropertyPreview(context: CanvasRenderingContext2D, node: GraphNode): void {
    const entries = Object.entries(node.properties);
    if (entries.length === 0) {
      return;
    }
    const [key, value] = entries[0];
    const y = node.position.y + node.size.height - 18;
    context.fillStyle = '#8f9da8';
    context.font = '10px Cascadia Mono, Consolas, monospace';
    context.textAlign = 'left';
    if (typeof value === 'string' && value.startsWith('#')) {
      context.fillStyle = value;
      context.fillRect(node.position.x + 10, y - 6, 12, 12);
      context.strokeStyle = '#34434c';
      context.strokeRect(node.position.x + 10, y - 6, 12, 12);
      context.fillStyle = '#8f9da8';
      context.fillText(`${key}: ${value}`, node.position.x + 28, y);
    } else {
      context.fillText(ellipsis(context, `${key}: ${String(value)}`, node.size.width - 20), node.position.x + 10, y);
    }
  }

  function drawPreviewWire(context: CanvasRenderingContext2D): void {
    if (drag?.kind !== 'connect') {
      return;
    }
    const from = drag.from.point;
    const to = drag.current;
    context.strokeStyle = '#d8e0e6';
    context.lineWidth = 2 / graph.viewState.zoom;
    context.setLineDash([7 / graph.viewState.zoom, 5 / graph.viewState.zoom]);
    context.beginPath();
    context.moveTo(from.x, from.y);
    bezierPath(context, from, to);
    context.stroke();
    context.setLineDash([]);
  }

  function drawSelectionBox(context: CanvasRenderingContext2D): void {
    if (drag?.kind !== 'box') {
      return;
    }
    const x = Math.min(drag.startScreen.x, drag.currentScreen.x);
    const y = Math.min(drag.startScreen.y, drag.currentScreen.y);
    const w = Math.abs(drag.currentScreen.x - drag.startScreen.x);
    const h = Math.abs(drag.currentScreen.y - drag.startScreen.y);
    context.fillStyle = 'rgba(95, 168, 211, 0.12)';
    context.strokeStyle = 'rgba(95, 168, 211, 0.72)';
    context.lineWidth = 1;
    context.fillRect(x, y, w, h);
    context.strokeRect(x, y, w, h);
  }

  function drawEmptyState(context: CanvasRenderingContext2D): void {
    if (graph.nodes.length > 1) {
      return;
    }
    context.fillStyle = 'rgba(216, 224, 230, 0.62)';
    context.font = '12px Inter, Segoe UI, sans-serif';
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    const x = width / 2;
    const y = Math.max(72, height - 74);
    context.fillText('Right-click to add a node. Drag pins to connect nodes. Select a node to edit details.', x, y);
  }

  function handlePointerDown(event: PointerEvent): void {
    if (!canvas) {
      return;
    }
    canvas.focus();
    const screen = eventPoint(event);
    const graphPoint = screenToGraph(screen);
    if (event.button === 1) {
      event.preventDefault();
      drag = {
        kind: 'pan',
        pointerId: event.pointerId,
        start: screen,
        panX: graph.viewState.panX,
        panY: graph.viewState.panY
      };
      canvas.setPointerCapture(event.pointerId);
      return;
    }
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    canvas.setPointerCapture(event.pointerId);
    const pinHit = hitPin(graphPoint);
    if (pinHit) {
      drag = { kind: 'connect', pointerId: event.pointerId, from: pinHit, current: graphPoint };
      hoveredPin = pinHit;
      scheduleDraw();
      return;
    }

    const connectionHit = hitConnection(graphPoint);
    if (connectionHit) {
      onSelection([{ kind: 'connection', id: connectionHit.id }]);
      scheduleDraw();
      return;
    }

    const nodeHit = hitNode(graphPoint);
    if (nodeHit) {
      const additive = event.shiftKey || event.ctrlKey || event.metaKey;
      const currentlySelected = selectedNodeIds.has(nodeHit.id);
      if (additive && currentlySelected) {
        onSelection(selected.filter((item) => !(item.kind === 'node' && item.id === nodeHit.id)));
        drag = null;
        return;
      }
      const nextSelection = additive
        ? [...selected.filter((item) => item.kind !== 'connection'), { kind: 'node' as const, id: nodeHit.id }]
        : currentlySelected
          ? selected
          : [{ kind: 'node' as const, id: nodeHit.id }];
      onSelection(dedupeSelections(nextSelection));
      const selectedIds = new Set(nextSelection.filter((item) => item.kind === 'node').map((item) => item.id));
      const initial = new Map(
        graph.nodes
          .filter((node) => selectedIds.has(node.id))
          .map((node) => [node.id, { x: node.position.x, y: node.position.y }])
      );
      drag = { kind: 'node', pointerId: event.pointerId, start: graphPoint, initial, current: [], moved: false };
      return;
    }

    onSelection([]);
    drag = { kind: 'box', pointerId: event.pointerId, startScreen: screen, currentScreen: screen };
    scheduleDraw();
  }

  function handlePointerMove(event: PointerEvent): void {
    const screen = eventPoint(event);
    const graphPoint = screenToGraph(screen);
    hoveredPin = hitPin(graphPoint);
    hoveredConnectionId = hitConnection(graphPoint)?.id ?? null;

    if (!drag) {
      scheduleDraw();
      return;
    }
    if (drag.kind === 'pan') {
      onViewState({
        ...graph.viewState,
        panX: drag.panX + screen.x - drag.start.x,
        panY: drag.panY + screen.y - drag.start.y
      });
      return;
    }
    if (drag.kind === 'node') {
      const dx = graphPoint.x - drag.start.x;
      const dy = graphPoint.y - drag.start.y;
      const moves: GraphNodeMove[] = [];
      drag.initial.forEach((from, id) => {
        moves.push({
          id,
          from: { ...from },
          to: {
            x: Math.round(from.x + dx),
            y: Math.round(from.y + dy)
          }
        });
      });
      drag.current = moves;
      drag.moved = Math.abs(dx) > 1 || Math.abs(dy) > 1;
      onMovePreview(moves);
      return;
    }
    if (drag.kind === 'connect') {
      drag.current = graphPoint;
      scheduleDraw();
      return;
    }
    drag.currentScreen = screen;
    scheduleDraw();
  }

  function handlePointerUp(event: PointerEvent): void {
    if (!drag || !canvas || event.pointerId !== drag.pointerId) {
      return;
    }
    canvas.releasePointerCapture(event.pointerId);
    const graphPoint = screenToGraph(eventPoint(event));
    if (drag.kind === 'node' && drag.moved && drag.current.length > 0) {
      onMoveCommit(drag.current);
    } else if (drag.kind === 'connect') {
      const target = hitPin(graphPoint);
      if (target && target.pin.id !== drag.from.pin.id) {
        onConnectPins(drag.from.pin, target.pin);
      }
    } else if (drag.kind === 'box') {
      onSelection(nodesInsideBox(drag.startScreen, drag.currentScreen).map((node) => ({ kind: 'node', id: node.id })));
    }
    drag = null;
    scheduleDraw();
  }

  function handlePointerLeave(): void {
    if (drag) {
      return;
    }
    hoveredPin = null;
    hoveredConnectionId = null;
    scheduleDraw();
  }

  function handleWheel(event: WheelEvent): void {
    event.preventDefault();
    const screen = eventPoint(event);
    const before = screenToGraph(screen);
    const zoomFactor = Math.exp(-event.deltaY * 0.001);
    const nextZoom = clamp(graph.viewState.zoom * zoomFactor, 0.25, 2);
    onViewState({
      ...graph.viewState,
      zoom: nextZoom,
      panX: screen.x - before.x * nextZoom,
      panY: screen.y - before.y * nextZoom
    });
  }

  function handleContextMenu(event: MouseEvent): void {
    event.preventDefault();
    const screen = eventPoint(event);
    onOpenCreateMenu(screenToGraph(screen), screen);
  }

  function cullVisibleNodes(): GraphNode[] {
    const topLeft = screenToGraph({ x: -120, y: -120 });
    const bottomRight = screenToGraph({ x: width + 120, y: height + 120 });
    return graph.nodes.filter(
      (node) =>
        node.position.x + node.size.width >= topLeft.x &&
        node.position.x <= bottomRight.x &&
        node.position.y + node.size.height >= topLeft.y &&
        node.position.y <= bottomRight.y
    );
  }

  function hitPin(point: GraphPoint): PinHit | null {
    const radius = 14 / graph.viewState.zoom;
    for (const node of [...graph.nodes].reverse()) {
      for (const pin of [...node.inputs, ...node.outputs]) {
        const pinPosition = pinPoint(node, pin);
        if (distance(point, pinPosition) <= radius) {
          return { node, pin, point: pinPosition };
        }
      }
    }
    return null;
  }

  function hitNode(point: GraphPoint): GraphNode | null {
    for (const node of [...graph.nodes].reverse()) {
      if (
        point.x >= node.position.x &&
        point.x <= node.position.x + node.size.width &&
        point.y >= node.position.y &&
        point.y <= node.position.y + node.size.height
      ) {
        return node;
      }
    }
    return null;
  }

  function hitConnection(point: GraphPoint): GraphConnection | null {
    const threshold = 8 / graph.viewState.zoom;
    for (const connection of [...graph.connections].reverse()) {
      const refs = pinRefsForConnection(graph, connection);
      if (!refs.fromNode || !refs.toNode || !refs.fromPin || !refs.toPin) {
        continue;
      }
      const cached = connectionPath(connection, refs.fromNode, refs.fromPin, refs.toNode, refs.toPin);
      for (let index = 1; index < cached.samples.length; index += 1) {
        if (distanceToSegment(point, cached.samples[index - 1], cached.samples[index]) <= threshold) {
          return connection;
        }
      }
    }
    return null;
  }

  function nodesInsideBox(startScreen: GraphPoint, endScreen: GraphPoint): GraphNode[] {
    const a = screenToGraph(startScreen);
    const b = screenToGraph(endScreen);
    const left = Math.min(a.x, b.x);
    const right = Math.max(a.x, b.x);
    const top = Math.min(a.y, b.y);
    const bottom = Math.max(a.y, b.y);
    return graph.nodes.filter(
      (node) =>
        node.position.x >= left &&
        node.position.x + node.size.width <= right &&
        node.position.y >= top &&
        node.position.y + node.size.height <= bottom
    );
  }

  function connectionPath(
    connection: GraphConnection,
    fromNode: GraphNode,
    fromPin: GraphPin,
    toNode: GraphNode,
    toPin: GraphPin
  ): ConnectionPathCache {
    const from = pinPoint(fromNode, fromPin);
    const to = pinPoint(toNode, toPin);
    const signature = `${from.x}:${from.y}:${to.x}:${to.y}:${graph.viewState.zoom < 0.35 ? 'line' : 'curve'}`;
    const cached = connectionPathCache.get(connection.id);
    if (cached?.signature === signature) {
      return cached;
    }
    const path = new Path2D();
    const samples: GraphPoint[] = [];
    if (graph.viewState.zoom < 0.35) {
      path.moveTo(from.x, from.y);
      path.lineTo(to.x, to.y);
      samples.push(from, to);
    } else {
      path.moveTo(from.x, from.y);
      bezierPath(path, from, to);
      for (let index = 0; index <= 16; index += 1) {
        samples.push(bezierPoint(from, to, index / 16));
      }
    }
    const next = { signature, path, samples };
    connectionPathCache.set(connection.id, next);
    return next;
  }

  function pinPoint(node: GraphNode, pin: GraphPin): GraphPoint {
    const pins = pin.direction === 'input' ? node.inputs : node.outputs;
    const index = Math.max(0, pins.findIndex((candidate) => candidate.id === pin.id));
    return {
      x: pin.direction === 'input' ? node.position.x : node.position.x + node.size.width,
      y: node.position.y + nodeHeaderHeight + pinRowHeight * index + pinRowHeight / 2
    };
  }

  function pinsCanConnectPair(first: GraphPin, second: GraphPin): boolean {
    const output = first.direction === 'output' ? first : second.direction === 'output' ? second : null;
    const input = first.direction === 'input' ? first : second.direction === 'input' ? second : null;
    return Boolean(output && input && pinsCanConnect(output, input));
  }

  function screenToGraph(point: GraphPoint): GraphPoint {
    return {
      x: (point.x - graph.viewState.panX) / graph.viewState.zoom,
      y: (point.y - graph.viewState.panY) / graph.viewState.zoom
    };
  }

  function eventPoint(event: MouseEvent | PointerEvent | WheelEvent): GraphPoint {
    const rect = canvas?.getBoundingClientRect();
    return {
      x: event.clientX - (rect?.left ?? 0),
      y: event.clientY - (rect?.top ?? 0)
    };
  }

  function bezierPath(target: CanvasRenderingContext2D | Path2D, from: GraphPoint, to: GraphPoint): void {
    const tangent = Math.max(42, Math.abs(to.x - from.x) * 0.45);
    target.bezierCurveTo(from.x + tangent, from.y, to.x - tangent, to.y, to.x, to.y);
  }

  function bezierPoint(from: GraphPoint, to: GraphPoint, t: number): GraphPoint {
    const tangent = Math.max(42, Math.abs(to.x - from.x) * 0.45);
    const c1 = { x: from.x + tangent, y: from.y };
    const c2 = { x: to.x - tangent, y: to.y };
    const mt = 1 - t;
    return {
      x: mt ** 3 * from.x + 3 * mt ** 2 * t * c1.x + 3 * mt * t ** 2 * c2.x + t ** 3 * to.x,
      y: mt ** 3 * from.y + 3 * mt ** 2 * t * c1.y + 3 * mt * t ** 2 * c2.y + t ** 3 * to.y
    };
  }

  function groupNodeIssues(issues: GraphValidationIssue[]): Map<string, GraphValidationIssue[]> {
    const grouped = new Map<string, GraphValidationIssue[]>();
    for (const issue of issues) {
      if (!issue.nodeId) {
        continue;
      }
      grouped.set(issue.nodeId, [...(grouped.get(issue.nodeId) ?? []), issue]);
    }
    return grouped;
  }

  function groupConnectionIssues(issues: GraphValidationIssue[]): Map<string, GraphValidationIssue[]> {
    const grouped = new Map<string, GraphValidationIssue[]>();
    for (const issue of issues) {
      if (!issue.connectionId) {
        continue;
      }
      grouped.set(issue.connectionId, [...(grouped.get(issue.connectionId) ?? []), issue]);
    }
    return grouped;
  }

  function categoryColor(category: string): string {
    if (category === 'Events') {
      return '#7a5a2a';
    }
    if (category === 'Values') {
      return '#28506b';
    }
    if (category === 'Math') {
      return '#315b55';
    }
    if (category === 'Material') {
      return '#6a4e24';
    }
    if (category === 'Image') {
      return '#443f73';
    }
    if (category === 'Outputs') {
      return '#6c3c38';
    }
    return '#3a4650';
  }

  function roundedRect(context: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number): void {
    const radius = Math.min(r, w / 2, h / 2);
    context.beginPath();
    context.moveTo(x + radius, y);
    context.arcTo(x + w, y, x + w, y + h, radius);
    context.arcTo(x + w, y + h, x, y + h, radius);
    context.arcTo(x, y + h, x, y, radius);
    context.arcTo(x, y, x + w, y, radius);
    context.closePath();
  }

  function ellipsis(context: CanvasRenderingContext2D, text: string, maxWidth: number): string {
    if (context.measureText(text).width <= maxWidth) {
      return text;
    }
    let next = text;
    while (next.length > 1 && context.measureText(`${next}...`).width > maxWidth) {
      next = next.slice(0, -1);
    }
    return `${next}...`;
  }

  function nodeBounds(nodes: GraphNode[]): { x: number; y: number; width: number; height: number } {
    const left = Math.min(...nodes.map((node) => node.position.x));
    const top = Math.min(...nodes.map((node) => node.position.y));
    const right = Math.max(...nodes.map((node) => node.position.x + node.size.width));
    const bottom = Math.max(...nodes.map((node) => node.position.y + node.size.height));
    return { x: left, y: top, width: right - left, height: bottom - top };
  }

  function dedupeSelections(selections: GraphSelection[]): GraphSelection[] {
    const seen = new Set<string>();
    return selections.filter((selection) => {
      const key = `${selection.kind}:${selection.id}`;
      if (seen.has(key)) {
        return false;
      }
      seen.add(key);
      return true;
    });
  }

  function distance(a: GraphPoint, b: GraphPoint): number {
    return Math.hypot(a.x - b.x, a.y - b.y);
  }

  function distanceToSegment(point: GraphPoint, start: GraphPoint, end: GraphPoint): number {
    const lengthSquared = (end.x - start.x) ** 2 + (end.y - start.y) ** 2;
    if (lengthSquared === 0) {
      return distance(point, start);
    }
    const t = clamp(((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y)) / lengthSquared, 0, 1);
    return distance(point, { x: start.x + t * (end.x - start.x), y: start.y + t * (end.y - start.y) });
  }

  function positiveModulo(value: number, divisor: number): number {
    return ((value % divisor) + divisor) % divisor;
  }

  function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }
</script>

<div class="graph-canvas-wrap" bind:this={wrapper}>
  <canvas
    bind:this={canvas}
    class="graph-canvas"
    tabindex="0"
    on:pointerdown={handlePointerDown}
    on:pointermove={handlePointerMove}
    on:pointerup={handlePointerUp}
    on:pointercancel={handlePointerUp}
    on:pointerleave={handlePointerLeave}
    on:wheel={handleWheel}
    on:contextmenu={handleContextMenu}
  ></canvas>
</div>
