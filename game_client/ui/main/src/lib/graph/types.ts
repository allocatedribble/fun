export const GRAPH_SCHEMA_VERSION = 1;

export type GraphPinType =
  | 'exec'
  | 'number'
  | 'integer'
  | 'boolean'
  | 'string'
  | 'color'
  | 'vector2'
  | 'vector3'
  | 'texture'
  | 'material'
  | 'asset'
  | 'any';

export type GraphPinDirection = 'input' | 'output';
export type GraphNodeCategory = 'Events' | 'Values' | 'Math' | 'Material' | 'Image' | 'Utility' | 'Outputs';
export type GraphValidationLevel = 'info' | 'warning' | 'error';
export type GraphSelectionKind = 'node' | 'connection';

export interface GraphPoint {
  x: number;
  y: number;
}

export interface GraphSize {
  width: number;
  height: number;
}

export interface GraphMetadata {
  description?: string;
  previewMode?: 'none' | 'manual' | 'debounced';
  [key: string]: unknown;
}

export interface GraphPin {
  id: string;
  nodeId: string;
  name: string;
  direction: GraphPinDirection;
  type: GraphPinType;
  defaultValue?: GraphPropertyValue;
  isRequired?: boolean;
  isMultiConnect?: boolean;
  metadata?: Record<string, unknown>;
}

export type GraphPropertyValue = string | number | boolean | null;
export type GraphProperties = Record<string, GraphPropertyValue>;

export interface GraphNodeState {
  collapsed?: boolean;
  loading?: boolean;
  error?: string | null;
}

export interface GraphNode {
  id: string;
  type: string;
  title: string;
  position: GraphPoint;
  size: GraphSize;
  inputs: GraphPin[];
  outputs: GraphPin[];
  properties: GraphProperties;
  state?: GraphNodeState;
  metadata?: Record<string, unknown>;
}

export interface GraphConnection {
  id: string;
  fromNodeId: string;
  fromPinId: string;
  toNodeId: string;
  toPinId: string;
  metadata?: Record<string, unknown>;
}

export interface GraphVariable {
  id: string;
  name: string;
  type: GraphPinType;
  defaultValue?: GraphPropertyValue;
  description?: string;
}

export interface GraphPanelState {
  left: boolean;
  right: boolean;
  status: boolean;
}

export interface GraphViewState {
  panX: number;
  panY: number;
  zoom: number;
  selectedIds: string[];
  collapsedPanels: GraphPanelState;
}

export interface GraphData {
  id: string;
  name: string;
  version: number;
  nodes: GraphNode[];
  connections: GraphConnection[];
  variables: GraphVariable[];
  viewState: GraphViewState;
  metadata: GraphMetadata;
}

export interface NodeDefinition {
  type: string;
  title: string;
  category: GraphNodeCategory;
  description: string;
  keywords: string[];
  inputs: Omit<GraphPin, 'nodeId'>[];
  outputs: Omit<GraphPin, 'nodeId'>[];
  defaultProperties: GraphProperties;
  defaultSize?: GraphSize;
}

export interface GraphSelection {
  kind: GraphSelectionKind;
  id: string;
}

export interface GraphValidationIssue {
  id: string;
  level: GraphValidationLevel;
  message: string;
  nodeId?: string;
  pinId?: string;
  connectionId?: string;
}

export interface GraphCreateNodeRequest {
  type: string;
  position: GraphPoint;
}

export interface GraphPinRef {
  nodeId: string;
  pinId: string;
}

export interface GraphNodeMove {
  id: string;
  from: GraphPoint;
  to: GraphPoint;
}

export function encodeSelection(selection: GraphSelection): string {
  return `${selection.kind}:${selection.id}`;
}

export function decodeSelection(value: string): GraphSelection | null {
  const separator = value.indexOf(':');
  if (separator <= 0) {
    return null;
  }
  const kind = value.slice(0, separator);
  if (kind !== 'node' && kind !== 'connection') {
    return null;
  }
  return { kind, id: value.slice(separator + 1) };
}
