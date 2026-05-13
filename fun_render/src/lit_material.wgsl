#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material_base_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<uniform> material_light_terms: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var<uniform> material_light_direction: vec4<f32>;

@fragment
fn fragment(mesh: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    let normal = normalize(mesh.world_normal);
    let surface_to_light = normalize(material_light_direction.xyz);
    let front = max(dot(normal, surface_to_light), 0.0);
    let back = max(dot(-normal, surface_to_light), 0.0) * 0.35;
    let wrapped = (max(front, back) + material_light_terms.z) / (1.0 + material_light_terms.z);
    let intensity = material_light_terms.x + material_light_terms.y * clamp(wrapped, 0.0, 1.0);
    out.color = vec4(material_base_color.rgb * intensity, material_base_color.a);
    return out;
}
