import { pinRefsForConnection } from './graphModel';
import type { GraphConnection, GraphData, GraphNode, GraphPin, GraphPropertyValue } from './types';

interface ExprResult {
  expr: string;
  usesTexture: boolean;
}

export function graphToWgsl(graph: GraphData): string {
  const output = graph.nodes.find((node) => node.type === 'material.output');
  const baseColorPin = output?.inputs.find((pin) => pin.id === 'base_color') ?? null;
  const texturePin = output?.inputs.find((pin) => pin.id === 'texture') ?? null;
  const opacityPin = output?.inputs.find((pin) => pin.id === 'opacity') ?? null;
  const textureExpr = texturePin ? expressionForInput(graph, output, texturePin) : null;
  const colorExpr = baseColorPin ? expressionForInput(graph, output, baseColorPin) : null;
  const opacityExpr = opacityPin ? expressionForInput(graph, output, opacityPin) : null;
  const fallbackColor = propertyColor(output?.properties.baseColor, 'vec4<f32>(0.82, 0.88, 0.92, 1.0)');
  const color = colorExpr?.expr ?? (textureExpr?.usesTexture ? textureExpr.expr : fallbackColor);
  const opacity = opacityExpr?.expr ?? propertyNumber(output?.properties.opacity, '1.0');
  const usesTexture = Boolean(textureExpr?.usesTexture || colorExpr?.usesTexture);

  return `${generatedHeader(graph)}
struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

struct MaterialOutput {
  color: vec4<f32>,
};

${usesTexture ? '@group(0) @binding(0) var material_texture: texture_2d<f32>;\n@group(0) @binding(1) var material_sampler: sampler;\n' : ''}fn evaluate_material(input: VertexOutput) -> MaterialOutput {
  var material: MaterialOutput;
  material.color = ${ensureVec4(color)};
  material.color.a = material.color.a * ${ensureF32(opacity)};
  return material;
}

@fragment
fn fragment_main(input: VertexOutput) -> @location(0) vec4<f32> {
  return evaluate_material(input).color;
}
`;
}

export function defaultShaderRelativePath(graphName: string): string {
  const slug = graphName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
  return `assets/shaders/materials/${slug || 'material_graph'}.wgsl`;
}

function expressionForInput(graph: GraphData, node: GraphNode | undefined, pin: GraphPin): ExprResult | null {
  if (!node) {
    return null;
  }
  const connection = graph.connections.find((candidate) => candidate.toNodeId === node.id && candidate.toPinId === pin.id);
  if (!connection) {
    return pin.defaultValue === undefined ? null : literalFor(pin.defaultValue, pin.type);
  }
  return expressionForConnection(graph, connection);
}

function expressionForConnection(graph: GraphData, connection: GraphConnection): ExprResult | null {
  const refs = pinRefsForConnection(graph, connection);
  if (!refs.fromNode || !refs.fromPin) {
    return null;
  }
  return expressionForOutput(graph, refs.fromNode, refs.fromPin);
}

function expressionForOutput(graph: GraphData, node: GraphNode, pin: GraphPin): ExprResult | null {
  if (node.type === 'value.color') {
    return literalFor(node.properties.value, 'color');
  }
  if (node.type === 'value.number') {
    return literalFor(node.properties.value, 'number');
  }
  if (node.type === 'value.string') {
    return null;
  }
  if (node.type === 'image.load' && pin.id === 'texture') {
    return { expr: 'textureSample(material_texture, material_sampler, input.uv)', usesTexture: true };
  }
  if (node.type === 'material.texture_sample' && pin.id === 'color') {
    return { expr: 'textureSample(material_texture, material_sampler, input.uv)', usesTexture: true };
  }
  if (node.type === 'math.multiply') {
    const a = expressionForNamedInput(graph, node, 'a')?.expr ?? '1.0';
    const b = expressionForNamedInput(graph, node, 'b')?.expr ?? '1.0';
    return { expr: `(${a} * ${b})`, usesTexture: expressionUsesTexture(graph, node, 'a') || expressionUsesTexture(graph, node, 'b') };
  }
  if (node.type === 'math.lerp') {
    const a = expressionForNamedInput(graph, node, 'a')?.expr ?? 'vec4<f32>(0.0, 0.0, 0.0, 1.0)';
    const b = expressionForNamedInput(graph, node, 'b')?.expr ?? 'vec4<f32>(1.0, 1.0, 1.0, 1.0)';
    const alpha = expressionForNamedInput(graph, node, 'alpha')?.expr ?? propertyNumber(node.properties.alpha, '0.5');
    return {
      expr: `mix(${a}, ${b}, ${ensureF32(alpha)})`,
      usesTexture:
        expressionUsesTexture(graph, node, 'a') ||
        expressionUsesTexture(graph, node, 'b') ||
        expressionUsesTexture(graph, node, 'alpha')
    };
  }
  if (node.type === 'utility.reroute') {
    return expressionForNamedInput(graph, node, 'in');
  }
  return null;
}

function expressionForNamedInput(graph: GraphData, node: GraphNode, pinId: string): ExprResult | null {
  const pin = node.inputs.find((candidate) => candidate.id === pinId);
  return pin ? expressionForInput(graph, node, pin) : null;
}

function expressionUsesTexture(graph: GraphData, node: GraphNode, pinId: string): boolean {
  return expressionForNamedInput(graph, node, pinId)?.usesTexture ?? false;
}

function literalFor(value: GraphPropertyValue | undefined, type: GraphPin['type']): ExprResult | null {
  if (type === 'color') {
    return { expr: propertyColor(value, 'vec4<f32>(1.0, 1.0, 1.0, 1.0)'), usesTexture: false };
  }
  if (type === 'number' || type === 'integer') {
    return { expr: propertyNumber(value, '0.0'), usesTexture: false };
  }
  if (type === 'boolean') {
    return { expr: value === true ? 'true' : 'false', usesTexture: false };
  }
  return null;
}

function propertyColor(value: GraphPropertyValue | undefined, fallback: string): string {
  if (typeof value !== 'string' || !/^#[0-9a-f]{6}$/i.test(value)) {
    return fallback;
  }
  const red = Number.parseInt(value.slice(1, 3), 16) / 255;
  const green = Number.parseInt(value.slice(3, 5), 16) / 255;
  const blue = Number.parseInt(value.slice(5, 7), 16) / 255;
  return `vec4<f32>(${formatFloat(red)}, ${formatFloat(green)}, ${formatFloat(blue)}, 1.0)`;
}

function propertyNumber(value: GraphPropertyValue | undefined, fallback: string): string {
  return typeof value === 'number' && Number.isFinite(value) ? formatFloat(value) : fallback;
}

function ensureVec4(expr: string): string {
  return expr.startsWith('vec4') || expr.startsWith('textureSample') || expr.startsWith('mix(') || expr.includes(' * ')
    ? expr
    : `vec4<f32>(${ensureF32(expr)}, ${ensureF32(expr)}, ${ensureF32(expr)}, 1.0)`;
}

function ensureF32(expr: string): string {
  return /^-?\d+(\.\d+)?$/.test(expr) ? formatFloat(Number(expr)) : expr;
}

function formatFloat(value: number): string {
  if (!Number.isFinite(value)) {
    return '0.0';
  }
  const fixed = Number.isInteger(value) ? `${value}.0` : value.toFixed(4).replace(/0+$/, '').replace(/\.$/, '.0');
  return Object.is(value, -0) ? '0.0' : fixed;
}

function generatedHeader(graph: GraphData): string {
  return `// Generated by Fun Editor Graph from ${graph.name}.
// Standard WGSL material shader source; edit and save as a native .wgsl file.`;
}
