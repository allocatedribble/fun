import { getNodeDefinition } from './nodeRegistry';
import {
  GRAPH_SCHEMA_VERSION,
  decodeSelection,
  encodeSelection,
  type GraphConnection,
  type GraphData,
  type GraphNode,
  type GraphPin,
  type GraphPinRef,
  type GraphPinType,
  type GraphPoint,
  type GraphProperties,
  type GraphSelection,
  type GraphValidationIssue,
  type NodeDefinition
} from './types';

const defaultGraphId = 'graph_image_material';
const nodeHeaderHeight = 30;
const pinRowHeight = 22;
const propertyRowHeight = 20;
const nodeVerticalPadding = 14;

export function createStarterGraph(): GraphData {
  const outputDefinition = getNodeDefinition('material.output');
  const outputNode = outputDefinition
    ? createNodeFromDefinition(outputDefinition, { x: 520, y: 160 }, 'node_output_material')
    : null;

  return {
    id: defaultGraphId,
    name: 'Image Material Graph',
    version: GRAPH_SCHEMA_VERSION,
    nodes: outputNode ? [outputNode] : [],
    connections: [],
    variables: [],
    viewState: {
      panX: 0,
      panY: 0,
      zoom: 1,
      selectedIds: [],
      collapsedPanels: {
        left: false,
        right: false,
        status: false
      }
    },
    metadata: {
      description: 'Lightweight parameter graph for material and image workflows.',
      previewMode: 'manual'
    }
  };
}

export function createNodeFromType(graph: GraphData, type: string, position: GraphPoint): GraphNode | null {
  const definition = getNodeDefinition(type);
  if (!definition) {
    return null;
  }
  return createNodeFromDefinition(definition, position, uniqueId(graph, 'node'));
}

export function createNodeFromDefinition(
  definition: NodeDefinition,
  position: GraphPoint,
  id: string
): GraphNode {
  const size = measureNodeSize(definition.inputs.length, definition.outputs.length, definition.defaultProperties, definition);
  return {
    id,
    type: definition.type,
    title: definition.title,
    position: { x: Math.round(position.x), y: Math.round(position.y) },
    size,
    inputs: definition.inputs.map((pin) => ({ ...pin, nodeId: id })),
    outputs: definition.outputs.map((pin) => ({ ...pin, nodeId: id })),
    properties: cloneProperties(definition.defaultProperties),
    state: {},
    metadata: {}
  };
}

export function createUnknownNode(type: string, id: string, title: string, position: GraphPoint): GraphNode {
  return {
    id,
    type,
    title: title || 'Unknown Node',
    position,
    size: { width: 200, height: 74 },
    inputs: [],
    outputs: [],
    properties: {},
    state: { error: 'Missing node definition' },
    metadata: { missingType: type }
  };
}

export function measureNodeSize(
  inputCount: number,
  outputCount: number,
  properties: GraphProperties,
  definition?: NodeDefinition
): { width: number; height: number } {
  const defaultWidth = definition?.defaultSize?.width ?? 200;
  const minHeight = definition?.defaultSize?.height ?? 68;
  const propertyRows = Object.keys(properties).length > 0 ? 1 : 0;
  return {
    width: Math.max(160, Math.min(280, defaultWidth)),
    height: Math.max(
      minHeight,
      nodeHeaderHeight + Math.max(inputCount, outputCount, 1) * pinRowHeight + propertyRows * propertyRowHeight + nodeVerticalPadding
    )
  };
}

export function sanitizeGraph(value: unknown): GraphData {
  if (!isRecord(value)) {
    return createStarterGraph();
  }

  const fallback = createStarterGraph();
  const nodes = Array.isArray(value.nodes)
    ? value.nodes.map((node, index) => sanitizeNode(node, `node_${index}`))
    : fallback.nodes;
  const connections = Array.isArray(value.connections)
    ? value.connections.map((connection, index) => sanitizeConnection(connection, `conn_${index}`))
    : [];
  const viewState = readRecord(value.viewState);
  const selectedIds = viewState?.selectedIds;
  const collapsedPanels = readRecord(viewState?.collapsedPanels);
  const metadata = readRecord(value.metadata);
  const previewMode = metadata?.previewMode === 'debounced' || metadata?.previewMode === 'none' ? metadata.previewMode : 'manual';

  return {
    id: stringOr(value.id, fallback.id),
    name: stringOr(value.name, fallback.name),
    version: Number.isInteger(value.version) ? Number(value.version) : GRAPH_SCHEMA_VERSION,
    nodes,
    connections,
    variables: Array.isArray(value.variables)
      ? value.variables
          .filter(isRecord)
          .map((variable, index) => ({
            id: stringOr(variable.id, `variable_${index}`),
            name: stringOr(variable.name, `Variable ${index + 1}`),
            type: pinTypeOr(variable.type, 'number'),
            defaultValue: primitiveProperty(variable.defaultValue),
            description: typeof variable.description === 'string' ? variable.description : ''
          }))
      : [],
    viewState: {
      panX: finiteNumber(viewState?.panX, fallback.viewState.panX),
      panY: finiteNumber(viewState?.panY, fallback.viewState.panY),
      zoom: clamp(finiteNumber(viewState?.zoom, fallback.viewState.zoom), 0.25, 2),
      selectedIds: Array.isArray(selectedIds)
        ? selectedIds.filter((item): item is string => typeof item === 'string')
        : [],
      collapsedPanels: {
        left: Boolean(collapsedPanels?.left),
        right: Boolean(collapsedPanels?.right),
        status: Boolean(collapsedPanels?.status)
      }
    },
    metadata: {
      ...(metadata ?? {}),
      description: typeof metadata?.description === 'string' ? metadata.description : '',
      previewMode
    }
  };
}

export function cloneGraph(graph: GraphData): GraphData {
  return structuredClone(graph);
}

export function uniqueId(graph: GraphData, prefix: string): string {
  const ids = new Set<string>([
    graph.id,
    ...graph.nodes.map((node) => node.id),
    ...graph.connections.map((connection) => connection.id),
    ...graph.variables.map((variable) => variable.id)
  ]);
  let index = Math.max(1, ids.size + 1);
  let id = `${prefix}_${index.toString(36)}`;
  while (ids.has(id)) {
    index += 1;
    id = `${prefix}_${index.toString(36)}`;
  }
  return id;
}

export function findNode(graph: GraphData, nodeId: string): GraphNode | undefined {
  return graph.nodes.find((node) => node.id === nodeId);
}

export function findPin(graph: GraphData, ref: GraphPinRef): GraphPin | undefined {
  const node = findNode(graph, ref.nodeId);
  return node ? [...node.inputs, ...node.outputs].find((pin) => pin.id === ref.pinId) : undefined;
}

export function findConnection(graph: GraphData, connectionId: string): GraphConnection | undefined {
  return graph.connections.find((connection) => connection.id === connectionId);
}

export function selectedItems(graph: GraphData): GraphSelection[] {
  return graph.viewState.selectedIds
    .map(decodeSelection)
    .filter((selection): selection is GraphSelection => Boolean(selection));
}

export function firstSelection(graph: GraphData): GraphSelection | null {
  return selectedItems(graph)[0] ?? null;
}

export function withSelection(graph: GraphData, selections: GraphSelection[]): GraphData {
  return {
    ...graph,
    viewState: {
      ...graph.viewState,
      selectedIds: selections.map(encodeSelection)
    }
  };
}

export function pinRefsForConnection(graph: GraphData, connection: GraphConnection): {
  fromNode?: GraphNode;
  toNode?: GraphNode;
  fromPin?: GraphPin;
  toPin?: GraphPin;
} {
  const fromNode = findNode(graph, connection.fromNodeId);
  const toNode = findNode(graph, connection.toNodeId);
  const fromPin = fromNode?.outputs.find((pin) => pin.id === connection.fromPinId);
  const toPin = toNode?.inputs.find((pin) => pin.id === connection.toPinId);
  return { fromNode, toNode, fromPin, toPin };
}

export function connectionFromPins(graph: GraphData, first: GraphPin, second: GraphPin): GraphConnection | null {
  const output = first.direction === 'output' ? first : second.direction === 'output' ? second : null;
  const input = first.direction === 'input' ? first : second.direction === 'input' ? second : null;
  if (!output || !input || !pinsCanConnect(output, input)) {
    return null;
  }

  return {
    id: uniqueId(graph, 'conn'),
    fromNodeId: output.nodeId,
    fromPinId: output.id,
    toNodeId: input.nodeId,
    toPinId: input.id,
    metadata: {}
  };
}

export function pinsCanConnect(output: GraphPin, input: GraphPin): boolean {
  if (output.direction !== 'output' || input.direction !== 'input') {
    return false;
  }
  if (output.nodeId === input.nodeId) {
    return false;
  }
  return pinTypesCompatible(output.type, input.type);
}

export function pinTypesCompatible(outputType: GraphPinType, inputType: GraphPinType): boolean {
  if (outputType === inputType) {
    return true;
  }
  if (outputType === 'exec' || inputType === 'exec') {
    return false;
  }
  if (outputType === 'any' || inputType === 'any') {
    return true;
  }
  return (
    (outputType === 'integer' && inputType === 'number') ||
    (outputType === 'number' && inputType === 'string') ||
    (outputType === 'color' && inputType === 'vector3')
  );
}

export function validateGraph(graph: GraphData): GraphValidationIssue[] {
  const issues: GraphValidationIssue[] = [];
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const connectionIds = new Set<string>();

  for (const node of graph.nodes) {
    if (!getNodeDefinition(node.type)) {
      issues.push({
        id: `unknown:${node.id}`,
        level: 'warning',
        nodeId: node.id,
        message: `Missing registry definition for ${node.type}.`
      });
    }

    for (const input of node.inputs) {
      if (!input.isRequired) {
        continue;
      }
      const connected = graph.connections.some((connection) => connection.toNodeId === node.id && connection.toPinId === input.id);
      const hasDefault = input.defaultValue !== undefined && input.defaultValue !== null && input.defaultValue !== '';
      if (!connected && !hasDefault) {
        issues.push({
          id: `required:${node.id}:${input.id}`,
          level: 'warning',
          nodeId: node.id,
          pinId: input.id,
          message: `${node.title} requires ${input.name}.`
        });
      }
    }
  }

  for (const connection of graph.connections) {
    if (connectionIds.has(connection.id)) {
      issues.push({
        id: `duplicate:${connection.id}`,
        level: 'error',
        connectionId: connection.id,
        message: `Duplicate connection id ${connection.id}.`
      });
    }
    connectionIds.add(connection.id);

    if (!nodeIds.has(connection.fromNodeId) || !nodeIds.has(connection.toNodeId)) {
      issues.push({
        id: `broken-node:${connection.id}`,
        level: 'error',
        connectionId: connection.id,
        message: 'Connection references a missing node.'
      });
      continue;
    }

    const { fromPin, toPin } = pinRefsForConnection(graph, connection);
    if (!fromPin || !toPin) {
      issues.push({
        id: `broken-pin:${connection.id}`,
        level: 'error',
        connectionId: connection.id,
        message: 'Connection references a missing pin.'
      });
      continue;
    }
    if (!pinsCanConnect(fromPin, toPin)) {
      issues.push({
        id: `incompatible:${connection.id}`,
        level: 'error',
        connectionId: connection.id,
        nodeId: connection.toNodeId,
        pinId: connection.toPinId,
        message: `${fromPin.type} cannot connect to ${toPin.type}.`
      });
    }

    if (!toPin.isMultiConnect) {
      const duplicateInputs = graph.connections.filter(
        (candidate) => candidate.toNodeId === connection.toNodeId && candidate.toPinId === connection.toPinId
      );
      if (duplicateInputs.length > 1 && duplicateInputs[0].id !== connection.id) {
        issues.push({
          id: `multi-input:${connection.id}`,
          level: 'error',
          connectionId: connection.id,
          nodeId: connection.toNodeId,
          pinId: connection.toPinId,
          message: `${toPin.name} accepts only one connection.`
        });
      }
    }
  }

  if (!graph.nodes.some((node) => node.type === 'material.output')) {
    issues.push({
      id: 'missing:material.output',
      level: 'error',
      message: 'Output Material node is missing.'
    });
  }

  for (const cycle of detectCycles(graph)) {
    issues.push({
      id: `cycle:${cycle.join('>')}`,
      level: 'error',
      nodeId: cycle[0],
      message: `Cycle detected: ${cycle.join(' -> ')}.`
    });
  }

  return issues;
}

export function issuesByNode(issues: GraphValidationIssue[]): Map<string, GraphValidationIssue[]> {
  const grouped = new Map<string, GraphValidationIssue[]>();
  for (const issue of issues) {
    if (!issue.nodeId) {
      continue;
    }
    const list = grouped.get(issue.nodeId) ?? [];
    list.push(issue);
    grouped.set(issue.nodeId, list);
  }
  return grouped;
}

export function issuesByConnection(issues: GraphValidationIssue[]): Map<string, GraphValidationIssue[]> {
  const grouped = new Map<string, GraphValidationIssue[]>();
  for (const issue of issues) {
    if (!issue.connectionId) {
      continue;
    }
    const list = grouped.get(issue.connectionId) ?? [];
    list.push(issue);
    grouped.set(issue.connectionId, list);
  }
  return grouped;
}

function sanitizeNode(value: unknown, fallbackId: string): GraphNode {
  if (!isRecord(value)) {
    return createUnknownNode('unknown', fallbackId, 'Unknown Node', { x: 0, y: 0 });
  }

  const id = stringOr(value.id, fallbackId);
  const type = stringOr(value.type, 'unknown');
  const definition = getNodeDefinition(type);
  const position = sanitizePoint(value.position);
  const properties = sanitizeProperties(value.properties);
  const base = definition
    ? createNodeFromDefinition(definition, position, id)
    : createUnknownNode(type, id, stringOr(value.title, 'Unknown Node'), position);

  const inputs = Array.isArray(value.inputs)
    ? value.inputs.map((pin, index) => sanitizePin(pin, id, 'input', `in_${index}`))
    : base.inputs;
  const outputs = Array.isArray(value.outputs)
    ? value.outputs.map((pin, index) => sanitizePin(pin, id, 'output', `out_${index}`))
    : base.outputs;
  const sizeRecord = readRecord(value.size);

  return {
    ...base,
    title: stringOr(value.title, base.title),
    position,
    size: {
      width: clamp(finiteNumber(sizeRecord?.width, base.size.width), 160, 280),
      height: clamp(finiteNumber(sizeRecord?.height, base.size.height), 52, 420)
    },
    inputs,
    outputs,
    properties: Object.keys(properties).length > 0 ? properties : base.properties,
    state: isRecord(value.state) ? { ...value.state } : base.state,
    metadata: isRecord(value.metadata) ? { ...value.metadata } : base.metadata
  };
}

function sanitizePin(value: unknown, nodeId: string, fallbackDirection: 'input' | 'output', fallbackId: string): GraphPin {
  const record = readRecord(value);
  return {
    id: stringOr(record?.id, fallbackId),
    nodeId,
    name: stringOr(record?.name, fallbackId),
    direction: record?.direction === 'output' || record?.direction === 'input' ? record.direction : fallbackDirection,
    type: pinTypeOr(record?.type, 'any'),
    defaultValue: primitiveProperty(record?.defaultValue),
    isRequired: Boolean(record?.isRequired),
    isMultiConnect: Boolean(record?.isMultiConnect),
    metadata: isRecord(record?.metadata) ? { ...record.metadata } : {}
  };
}

function sanitizeConnection(value: unknown, fallbackId: string): GraphConnection {
  const record = readRecord(value);
  return {
    id: stringOr(record?.id, fallbackId),
    fromNodeId: stringOr(record?.fromNodeId, ''),
    fromPinId: stringOr(record?.fromPinId, ''),
    toNodeId: stringOr(record?.toNodeId, ''),
    toPinId: stringOr(record?.toPinId, ''),
    metadata: isRecord(record?.metadata) ? { ...record.metadata } : {}
  };
}

function detectCycles(graph: GraphData): string[][] {
  const adjacency = new Map<string, string[]>();
  for (const node of graph.nodes) {
    adjacency.set(node.id, []);
  }
  for (const connection of graph.connections) {
    const list = adjacency.get(connection.fromNodeId);
    if (list && connection.toNodeId) {
      list.push(connection.toNodeId);
    }
  }

  const visiting = new Set<string>();
  const visited = new Set<string>();
  const path: string[] = [];
  const cycles: string[][] = [];

  function visit(nodeId: string): void {
    if (visiting.has(nodeId)) {
      const start = path.indexOf(nodeId);
      if (start >= 0) {
        cycles.push(path.slice(start));
      }
      return;
    }
    if (visited.has(nodeId)) {
      return;
    }
    visiting.add(nodeId);
    path.push(nodeId);
    for (const next of adjacency.get(nodeId) ?? []) {
      visit(next);
    }
    path.pop();
    visiting.delete(nodeId);
    visited.add(nodeId);
  }

  for (const node of graph.nodes) {
    visit(node.id);
  }
  return cycles;
}

function cloneProperties(properties: GraphProperties): GraphProperties {
  return Object.fromEntries(Object.entries(properties).map(([key, value]) => [key, value]));
}

function sanitizeProperties(value: unknown): GraphProperties {
  if (!isRecord(value)) {
    return {};
  }
  const properties: GraphProperties = {};
  for (const [key, property] of Object.entries(value)) {
    const sanitized = primitiveProperty(property);
    if (sanitized !== undefined) {
      properties[key] = sanitized;
    }
  }
  return properties;
}

function primitiveProperty(value: unknown) {
  return typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean' || value === null
    ? value
    : undefined;
}

function sanitizePoint(value: unknown): GraphPoint {
  const record = readRecord(value);
  return {
    x: finiteNumber(record?.x, 0),
    y: finiteNumber(record?.y, 0)
  };
}

function pinTypeOr(value: unknown, fallback: GraphPinType): GraphPinType {
  return value === 'exec' ||
    value === 'number' ||
    value === 'integer' ||
    value === 'boolean' ||
    value === 'string' ||
    value === 'color' ||
    value === 'vector2' ||
    value === 'vector3' ||
    value === 'texture' ||
    value === 'material' ||
    value === 'asset' ||
    value === 'any'
    ? value
    : fallback;
}

function stringOr(value: unknown, fallback: string): string {
  return typeof value === 'string' && value.length > 0 ? value : fallback;
}

function finiteNumber(value: unknown, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function readRecord(value: unknown): Record<string, unknown> | undefined {
  return isRecord(value) ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
