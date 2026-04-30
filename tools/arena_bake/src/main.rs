use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for entry in game_shared::DEMO_RENDER_CATALOG {
        hash = fnv1a(hash, entry.asset_id.0);
        hash = fnv1a(hash, entry.material.0);
        hash = fnv1a(hash, entry.occlusion_cell.0);
        hash = fnv1a(hash, entry.lighting.0);
    }

    let mut stdout = io::BufWriter::new(io::stdout().lock());
    writeln!(stdout, "arena_package=demo_arena")?;
    writeln!(stdout, "content_hash={hash:016x}")?;
    writeln!(
        stdout,
        "static_render_assets={}",
        game_shared::DEMO_RENDER_CATALOG.len()
    )?;
    for entry in game_shared::DEMO_RENDER_CATALOG {
        writeln!(
            stdout,
            "asset id={} name={} cost={:?} visual={} meshlet={} raster={} ray_proxy={} occlusion_cell={} tag={}",
            entry.asset_id.0,
            entry.name,
            entry.cost_class,
            entry.visual_mesh,
            entry.meshlet_mesh,
            entry.fallback_raster_mesh,
            entry.ray_proxy,
            entry.occlusion_cell.0,
            entry.gameplay_tag
        )?;
    }
    Ok(())
}

fn fnv1a(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
