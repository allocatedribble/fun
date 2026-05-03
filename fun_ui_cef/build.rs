use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct AssetEntry {
    route_path: String,
    source_path: PathBuf,
    mime_type: &'static str,
    content_type: String,
}

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let dist_dir = manifest_dir.join("../game_client/ui/main/dist");
    println!("cargo:rerun-if-changed={}", dist_dir.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let output_path = out_dir.join("fun_ui_assets.rs");
    if !dist_dir.is_dir() {
        let mut output = File::create(output_path)?;
        writeln!(
            output,
            "compile_error!(\"game_client/ui/main/dist is missing; run npm run build in game_client/ui/main before building fun_ui_cef.\");"
        )?;
        return Ok(());
    }

    let mut entries = Vec::new();
    collect_asset_entries(&dist_dir, &dist_dir, &mut entries)?;
    entries.sort_by(|left, right| left.route_path.cmp(&right.route_path));

    let mut output = File::create(output_path)?;
    writeln!(
        output,
        "pub const GENERATED_FUN_UI_ASSETS: &[GeneratedFunUiAsset] = &["
    )?;
    for entry in entries {
        println!("cargo:rerun-if-changed={}", entry.source_path.display());
        writeln!(
            output,
            "    GeneratedFunUiAsset {{ path: {:?}, mime_type: {:?}, content_type: {:?}, bytes: include_bytes!(r#\"{}\"#) }},",
            entry.route_path,
            entry.mime_type,
            entry.content_type,
            entry.source_path.display()
        )?;
    }
    writeln!(output, "];")?;
    Ok(())
}

fn collect_asset_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<AssetEntry>,
) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_asset_entries(root, &path, entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let relative_path = path.strip_prefix(root).expect("asset path under root");
        if has_hidden_segment(relative_path) {
            continue;
        }
        let route_path = format!("/{}", relative_path.to_string_lossy().replace('\\', "/"));
        let mime_type = mime_type_for(relative_path);
        entries.push(AssetEntry {
            route_path,
            source_path: path,
            mime_type,
            content_type: content_type_for(mime_type),
        });
    }
    Ok(())
}

fn has_hidden_segment(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}

fn mime_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html",
        Some("js") | Some("mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json") | Some("map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn content_type_for(mime_type: &'static str) -> String {
    match mime_type {
        "text/html" | "text/javascript" | "text/css" | "application/json" | "image/svg+xml" => {
            format!("{mime_type}; charset=utf-8")
        }
        _ => mime_type.to_owned(),
    }
}
