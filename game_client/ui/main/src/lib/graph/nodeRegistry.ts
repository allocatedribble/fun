import type { GraphNodeCategory, GraphPinType, NodeDefinition } from './types';

export const nodeCategories: GraphNodeCategory[] = [
  'Events',
  'Values',
  'Math',
  'Material',
  'Image',
  'Utility',
  'Outputs'
];

export const pinTypeLabels: Record<GraphPinType, string> = {
  exec: 'exec',
  number: 'number',
  integer: 'integer',
  boolean: 'boolean',
  string: 'string',
  color: 'color',
  vector2: 'vec2',
  vector3: 'vec3',
  texture: 'texture',
  material: 'material',
  asset: 'asset',
  any: 'any'
};

export const pinTypeColors: Record<GraphPinType, string> = {
  exec: '#d7a85b',
  number: '#78b7dd',
  integer: '#6da8db',
  boolean: '#c786d8',
  string: '#7cbf8d',
  color: '#e2a6b4',
  vector2: '#8bd3c7',
  vector3: '#6fc4b9',
  texture: '#b9a7ff',
  material: '#f0b84e',
  asset: '#a8b4bf',
  any: '#c7d0d7'
};

export const nodeDefinitions: NodeDefinition[] = [
  {
    type: 'event.update',
    title: 'On Update',
    category: 'Events',
    description: 'Starts a lightweight logic flow each editor update tick.',
    keywords: ['event', 'tick', 'frame', 'logic'],
    inputs: [],
    outputs: [{ id: 'exec_out', name: 'Then', direction: 'output', type: 'exec' }],
    defaultProperties: {},
    defaultSize: { width: 184, height: 58 }
  },
  {
    type: 'value.number',
    title: 'Number',
    category: 'Values',
    description: 'Editable floating point value.',
    keywords: ['float', 'scalar', 'value', 'parameter'],
    inputs: [],
    outputs: [{ id: 'value', name: 'Value', direction: 'output', type: 'number' }],
    defaultProperties: { value: 1 },
    defaultSize: { width: 174, height: 88 }
  },
  {
    type: 'value.string',
    title: 'String',
    category: 'Values',
    description: 'Editable text value.',
    keywords: ['text', 'url', 'parameter', 'value'],
    inputs: [],
    outputs: [{ id: 'value', name: 'Value', direction: 'output', type: 'string' }],
    defaultProperties: { value: '' },
    defaultSize: { width: 196, height: 88 }
  },
  {
    type: 'value.color',
    title: 'Color',
    category: 'Values',
    description: 'Editable color value.',
    keywords: ['rgba', 'tint', 'base', 'albedo', 'value'],
    inputs: [],
    outputs: [{ id: 'value', name: 'Value', direction: 'output', type: 'color' }],
    defaultProperties: { value: '#d8e0e6' },
    defaultSize: { width: 182, height: 88 }
  },
  {
    type: 'image.load',
    title: 'Load Image',
    category: 'Image',
    description: 'Loads an image from a URL and outputs a texture.',
    keywords: ['image', 'url', 'texture', 'download', 'async'],
    inputs: [
      { id: 'exec_in', name: 'In', direction: 'input', type: 'exec' },
      { id: 'url', name: 'URL', direction: 'input', type: 'string', isRequired: true }
    ],
    outputs: [
      { id: 'exec_out', name: 'Loaded', direction: 'output', type: 'exec' },
      { id: 'texture', name: 'Texture', direction: 'output', type: 'texture' }
    ],
    defaultProperties: { cache: true },
    defaultSize: { width: 218, height: 92 }
  },
  {
    type: 'material.texture_sample',
    title: 'Texture Sample',
    category: 'Material',
    description: 'Reads a texture and outputs a color sample.',
    keywords: ['texture', 'sample', 'uv', 'material', 'color'],
    inputs: [
      { id: 'texture', name: 'Texture', direction: 'input', type: 'texture', isRequired: true },
      { id: 'uv', name: 'UV', direction: 'input', type: 'vector2' }
    ],
    outputs: [{ id: 'color', name: 'Color', direction: 'output', type: 'color' }],
    defaultProperties: {},
    defaultSize: { width: 226, height: 88 }
  },
  {
    type: 'material.output',
    title: 'Output Material',
    category: 'Outputs',
    description: 'Defines the material-like graph result.',
    keywords: ['output', 'material', 'surface', 'result'],
    inputs: [
      { id: 'base_color', name: 'Base Color', direction: 'input', type: 'color' },
      { id: 'texture', name: 'Texture', direction: 'input', type: 'texture' },
      { id: 'opacity', name: 'Opacity', direction: 'input', type: 'number' }
    ],
    outputs: [],
    defaultProperties: { opacity: 1, roughness: 0.55, metallic: 0 },
    defaultSize: { width: 232, height: 112 }
  },
  {
    type: 'math.multiply',
    title: 'Multiply',
    category: 'Math',
    description: 'Multiplies compatible scalar or color values.',
    keywords: ['math', 'mul', 'scale', 'color', 'tint'],
    inputs: [
      { id: 'a', name: 'A', direction: 'input', type: 'any', isRequired: true },
      { id: 'b', name: 'B', direction: 'input', type: 'any', isRequired: true }
    ],
    outputs: [{ id: 'result', name: 'Result', direction: 'output', type: 'any' }],
    defaultProperties: {},
    defaultSize: { width: 196, height: 88 }
  },
  {
    type: 'math.lerp',
    title: 'Lerp',
    category: 'Math',
    description: 'Interpolates between A and B by Alpha.',
    keywords: ['mix', 'blend', 'interpolate', 'math'],
    inputs: [
      { id: 'a', name: 'A', direction: 'input', type: 'any', isRequired: true },
      { id: 'b', name: 'B', direction: 'input', type: 'any', isRequired: true },
      { id: 'alpha', name: 'Alpha', direction: 'input', type: 'number' }
    ],
    outputs: [{ id: 'result', name: 'Result', direction: 'output', type: 'any' }],
    defaultProperties: { alpha: 0.5 },
    defaultSize: { width: 196, height: 110 }
  },
  {
    type: 'utility.reroute',
    title: 'Reroute',
    category: 'Utility',
    description: 'Keeps long connections readable without changing graph behavior.',
    keywords: ['wire', 'route', 'organize', 'utility'],
    inputs: [{ id: 'in', name: 'In', direction: 'input', type: 'any', isRequired: true }],
    outputs: [{ id: 'out', name: 'Out', direction: 'output', type: 'any' }],
    defaultProperties: {},
    defaultSize: { width: 168, height: 72 }
  },
  {
    type: 'utility.comment',
    title: 'Comment',
    category: 'Utility',
    description: 'Annotation box for graph notes.',
    keywords: ['note', 'annotation', 'comment', 'label'],
    inputs: [],
    outputs: [],
    defaultProperties: { text: 'Comment' },
    defaultSize: { width: 240, height: 110 }
  }
];

const definitionsByType = new Map(nodeDefinitions.map((definition) => [definition.type, definition]));
const searchIndex = nodeDefinitions.map((definition) => ({
  definition,
  text: [definition.title, definition.type, definition.category, definition.description, ...definition.keywords]
    .join(' ')
    .toLowerCase()
}));

export function getNodeDefinition(type: string): NodeDefinition | undefined {
  return definitionsByType.get(type);
}

export function searchNodeDefinitions(query: string): NodeDefinition[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) {
    return nodeDefinitions;
  }
  return searchIndex
    .map((entry) => ({ entry, score: scoreSearch(entry.text, normalized) }))
    .filter((entry) => entry.score > 0)
    .sort((a, b) => b.score - a.score || a.entry.definition.title.localeCompare(b.entry.definition.title))
    .map((entry) => entry.entry.definition);
}

function scoreSearch(text: string, query: string): number {
  if (text.includes(query)) {
    return 100 + query.length;
  }

  let cursor = 0;
  let matched = 0;
  for (const character of query) {
    const index = text.indexOf(character, cursor);
    if (index < 0) {
      return 0;
    }
    matched += index === cursor ? 2 : 1;
    cursor = index + 1;
  }
  return matched;
}
