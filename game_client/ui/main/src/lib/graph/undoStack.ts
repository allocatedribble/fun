import { cloneGraph, withSelection } from './graphModel';
import type { GraphConnection, GraphData, GraphNode, GraphNodeMove } from './types';

export interface GraphCommand {
  label: string;
  redo: (graph: GraphData) => GraphData;
  undo: (graph: GraphData) => GraphData;
}

export class UndoStack {
  private readonly limit: number;
  private past: GraphCommand[] = [];
  private future: GraphCommand[] = [];

  constructor(limit = 80) {
    this.limit = limit;
  }

  get canUndo(): boolean {
    return this.past.length > 0;
  }

  get canRedo(): boolean {
    return this.future.length > 0;
  }

  execute(graph: GraphData, command: GraphCommand): GraphData {
    const next = command.redo(graph);
    this.past = [...this.past, command].slice(-this.limit);
    this.future = [];
    return next;
  }

  undo(graph: GraphData): GraphData {
    const command = this.past[this.past.length - 1];
    if (!command) {
      return graph;
    }
    this.past = this.past.slice(0, -1);
    this.future = [...this.future, command].slice(-this.limit);
    return command.undo(graph);
  }

  redo(graph: GraphData): GraphData {
    const command = this.future[this.future.length - 1];
    if (!command) {
      return graph;
    }
    this.future = this.future.slice(0, -1);
    this.past = [...this.past, command].slice(-this.limit);
    return command.redo(graph);
  }

  clear(): void {
    this.past = [];
    this.future = [];
  }
}

export function addNodeCommand(node: GraphNode): GraphCommand {
  const stored = cloneNode(node);
  return {
    label: `Add ${node.title}`,
    redo: (graph) =>
      withSelection(
        {
          ...graph,
          nodes: [...graph.nodes.filter((candidate) => candidate.id !== stored.id), cloneNode(stored)]
        },
        [{ kind: 'node', id: stored.id }]
      ),
    undo: (graph) =>
      withSelection(
        {
          ...graph,
          nodes: graph.nodes.filter((candidate) => candidate.id !== stored.id),
          connections: graph.connections.filter(
            (connection) => connection.fromNodeId !== stored.id && connection.toNodeId !== stored.id
          )
        },
        []
      )
  };
}

export function deleteSelectionCommand(
  nodes: GraphNode[],
  connections: GraphConnection[],
  selectionIds: string[]
): GraphCommand {
  const storedNodes = nodes.map(cloneNode);
  const storedConnections = connections.map(cloneConnection);
  const nodeIds = new Set(storedNodes.map((node) => node.id));
  const connectionIds = new Set(storedConnections.map((connection) => connection.id));

  return {
    label: 'Delete selection',
    redo: (graph) =>
      withSelection(
        {
          ...graph,
          nodes: graph.nodes.filter((node) => !nodeIds.has(node.id)),
          connections: graph.connections.filter(
            (connection) =>
              !connectionIds.has(connection.id) &&
              !nodeIds.has(connection.fromNodeId) &&
              !nodeIds.has(connection.toNodeId)
          )
        },
        []
      ),
    undo: (graph) => ({
      ...graph,
      nodes: mergeNodes(graph.nodes, storedNodes),
      connections: mergeConnections(graph.connections, storedConnections),
      viewState: {
        ...graph.viewState,
        selectedIds: selectionIds
      }
    })
  };
}

export function moveNodesCommand(moves: GraphNodeMove[]): GraphCommand {
  const stored = moves.map((move) => ({
    id: move.id,
    from: { ...move.from },
    to: { ...move.to }
  }));
  return {
    label: 'Move nodes',
    redo: (graph) => moveNodes(graph, stored, 'to'),
    undo: (graph) => moveNodes(graph, stored, 'from')
  };
}

export function connectPinsCommand(connection: GraphConnection, replacedConnections: GraphConnection[]): GraphCommand {
  const stored = cloneConnection(connection);
  const replaced = replacedConnections.map(cloneConnection);
  const replacedIds = new Set(replaced.map((item) => item.id));
  return {
    label: 'Connect pins',
    redo: (graph) =>
      withSelection(
        {
          ...graph,
          connections: [
            ...graph.connections.filter((candidate) => candidate.id !== stored.id && !replacedIds.has(candidate.id)),
            cloneConnection(stored)
          ]
        },
        [{ kind: 'connection', id: stored.id }]
      ),
    undo: (graph) =>
      withSelection(
        {
          ...graph,
          connections: mergeConnections(
            graph.connections.filter((candidate) => candidate.id !== stored.id),
            replaced
          )
        },
        []
      )
  };
}

export function deleteConnectionCommand(connection: GraphConnection): GraphCommand {
  const stored = cloneConnection(connection);
  return {
    label: 'Delete connection',
    redo: (graph) =>
      withSelection(
        {
          ...graph,
          connections: graph.connections.filter((candidate) => candidate.id !== stored.id)
        },
        []
      ),
    undo: (graph) =>
      withSelection(
        {
          ...graph,
          connections: mergeConnections(graph.connections, [stored])
        },
        [{ kind: 'connection', id: stored.id }]
      )
  };
}

export function editNodePropertyCommand(
  nodeId: string,
  property: string,
  before: string | number | boolean | null,
  after: string | number | boolean | null
): GraphCommand {
  return {
    label: `Edit ${property}`,
    redo: (graph) => setNodeProperty(graph, nodeId, property, after),
    undo: (graph) => setNodeProperty(graph, nodeId, property, before)
  };
}

export function renameNodeCommand(nodeId: string, before: string, after: string): GraphCommand {
  return {
    label: 'Rename node',
    redo: (graph) => setNodeTitle(graph, nodeId, after),
    undo: (graph) => setNodeTitle(graph, nodeId, before)
  };
}

export function replaceGraphCommand(before: GraphData, after: GraphData, label = 'Load graph'): GraphCommand {
  const storedBefore = cloneGraph(before);
  const storedAfter = cloneGraph(after);
  return {
    label,
    redo: () => cloneGraph(storedAfter),
    undo: () => cloneGraph(storedBefore)
  };
}

function moveNodes(graph: GraphData, moves: GraphNodeMove[], target: 'from' | 'to'): GraphData {
  const byId = new Map(moves.map((move) => [move.id, move[target]]));
  return {
    ...graph,
    nodes: graph.nodes.map((node) => {
      const position = byId.get(node.id);
      return position ? { ...node, position: { ...position } } : node;
    })
  };
}

function setNodeProperty(
  graph: GraphData,
  nodeId: string,
  property: string,
  value: string | number | boolean | null
): GraphData {
  return {
    ...graph,
    nodes: graph.nodes.map((node) =>
      node.id === nodeId
        ? {
            ...node,
            properties: {
              ...node.properties,
              [property]: value
            }
          }
        : node
    )
  };
}

function setNodeTitle(graph: GraphData, nodeId: string, title: string): GraphData {
  return {
    ...graph,
    nodes: graph.nodes.map((node) => (node.id === nodeId ? { ...node, title } : node))
  };
}

function mergeNodes(current: GraphNode[], nodes: GraphNode[]): GraphNode[] {
  const ids = new Set(nodes.map((node) => node.id));
  return [...current.filter((node) => !ids.has(node.id)), ...nodes.map(cloneNode)];
}

function mergeConnections(current: GraphConnection[], connections: GraphConnection[]): GraphConnection[] {
  const ids = new Set(connections.map((connection) => connection.id));
  return [...current.filter((connection) => !ids.has(connection.id)), ...connections.map(cloneConnection)];
}

function cloneNode(node: GraphNode): GraphNode {
  return structuredClone(node);
}

function cloneConnection(connection: GraphConnection): GraphConnection {
  return { ...connection, metadata: connection.metadata ? { ...connection.metadata } : {} };
}
