use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use virtual_geometry_bake::{VirtualGeometryBakeConfig, bake_virtual_geometry_lite};

#[derive(Debug)]
enum BakeToolError {
    Usage,
    MissingValue,
    Io(std::io::Error),
    Json(serde_json::Error),
    Bake(virtual_geometry_bake::VirtualGeometryBakeError),
}

impl std::fmt::Display for BakeToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage => formatter.write_str(
                "usage: virtual_geometry_bake [--sample | --input spec.json] --output asset.funvg.json",
            ),
            Self::MissingValue => formatter.write_str("argument requires a value"),
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::Bake(error) => write!(formatter, "bake error: {error:?}"),
        }
    }
}

impl std::error::Error for BakeToolError {}

impl From<std::io::Error> for BakeToolError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for BakeToolError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<virtual_geometry_bake::VirtualGeometryBakeError> for BakeToolError {
    fn from(error: virtual_geometry_bake::VirtualGeometryBakeError) -> Self {
        Self::Bake(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BakeToolArgs {
    input: Option<PathBuf>,
    output: PathBuf,
}

fn main() -> Result<(), BakeToolError> {
    let args = parse_args(env::args_os().skip(1))?;
    let config = match args.input.as_ref() {
        Some(input) => read_config(input)?,
        None => VirtualGeometryBakeConfig::default(),
    };
    let asset = bake_virtual_geometry_lite(&config)?;
    let payload = serde_json::to_vec_pretty(&asset)?;
    if let Some(parent) = args.output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(args.output, payload)?;
    Ok(())
}

fn read_config(path: &Path) -> Result<VirtualGeometryBakeConfig, BakeToolError> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn parse_args(args: impl IntoIterator<Item = OsString>) -> Result<BakeToolArgs, BakeToolError> {
    let mut input = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--input") => {
                input = Some(PathBuf::from(
                    iter.next().ok_or(BakeToolError::MissingValue)?,
                ));
            }
            Some("--output") => {
                output = Some(PathBuf::from(
                    iter.next().ok_or(BakeToolError::MissingValue)?,
                ));
            }
            Some("--sample") => {
                input = None;
            }
            _ => return Err(BakeToolError::Usage),
        }
    }
    Ok(BakeToolArgs {
        input,
        output: output.ok_or(BakeToolError::Usage)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_args_accepts_sample_output() {
        let args = parse_args([
            OsString::from("--sample"),
            OsString::from("--output"),
            OsString::from("out.funvg.json"),
        ])
        .expect("args should parse");

        assert_eq!(args.input, None);
        assert_eq!(args.output, PathBuf::from("out.funvg.json"));
    }

    #[test]
    fn parse_args_accepts_input_output() {
        let args = parse_args([
            OsString::from("--input"),
            OsString::from("mesh.funvg-bake.json"),
            OsString::from("--output"),
            OsString::from("mesh.funvg.json"),
        ])
        .expect("args should parse");

        assert_eq!(args.input, Some(PathBuf::from("mesh.funvg-bake.json")));
        assert_eq!(args.output, PathBuf::from("mesh.funvg.json"));
    }
}
