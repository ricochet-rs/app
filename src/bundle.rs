//! Packs a project folder the way the ricochet CLI does, for `POST /api/v0/content/upload`.

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::{Path, PathBuf};

/// Folders ricochet never wants in a bundle, as the CLI leaves them out.
const ALWAYS_EXCLUDED: [&str; 12] = [
    ".venv",
    ".venv/**",
    ".renv",
    ".renv/**",
    "__pycache__",
    "__pycache__/**",
    "**/__pycache__",
    "**/__pycache__/**",
    ".ipynb_checkpoints",
    ".ipynb_checkpoints/**",
    "**/.ipynb_checkpoints",
    "**/.ipynb_checkpoints/**",
];

/// A folder ready to upload: its `_ricochet.toml`, the item it deploys again if any, and its files as a gzip tar.
pub struct Bundle {
    pub config: String,
    pub id: Option<String>,
    pub bytes: Vec<u8>,
}

pub fn pack(folder: &Path) -> Result<Bundle, String> {
    let config = std::fs::read_to_string(folder.join("_ricochet.toml")).map_err(|_| {
        "The folder has no `_ricochet.toml`. Create one with `ricochet init` first.".to_owned()
    })?;
    let toml: toml_edit::DocumentMut = config
        .parse()
        .map_err(|error| format!("`_ricochet.toml` is invalid: {error}"))?;
    let content = &toml["content"];
    let text = |key: &str| content.get(key)?.as_str().map(str::to_owned);
    let patterns = |key: &str| -> Option<Vec<String>> {
        Some(
            content
                .get(key)?
                .as_array()?
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
        )
    };
    let files = bundled_files(
        folder,
        patterns("include"),
        patterns("exclude").unwrap_or_default(),
    )?;
    let mut bytes = Vec::new();
    {
        let encoder = flate2::write::GzEncoder::new(&mut bytes, flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for file in &files {
            tar.append_path_with_name(folder.join(file), file)
                .map_err(|error| format!("Couldn't read {}: {error}", file.display()))?;
        }
        tar.into_inner()
            .and_then(|encoder| encoder.finish())
            .map_err(|error| error.to_string())?;
    }
    Ok(Bundle {
        id: text("id"),
        config,
        bytes,
    })
}

fn glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).map_err(|error| error.to_string())?);
    }
    builder.build().map_err(|error| error.to_string())
}

/// The files under `folder`, relative to it, that the bundle carries.
fn bundled_files(
    folder: &Path,
    include: Option<Vec<String>>,
    exclude: Vec<String>,
) -> Result<Vec<PathBuf>, String> {
    let always = glob_set(&ALWAYS_EXCLUDED.map(str::to_owned))?;
    let include = include.map(|patterns| glob_set(&patterns)).transpose()?;
    let exclude = glob_set(&exclude)?;
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(folder)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(folder)
                .ok()
                .map(Path::to_path_buf)
        })
        .filter(|path| {
            !always.is_match(path)
                && include
                    .as_ref()
                    .is_none_or(|include| include.is_match(path))
                && !exclude.is_match(path)
        })
        .collect();
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_follow_the_cli_rules() {
        let folder = tempfile::tempdir().unwrap();
        let write = |path: &str| {
            let path = folder.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "x").unwrap();
        };
        std::fs::write(
            folder.path().join("_ricochet.toml"),
            "[content]\nid = \"01J\"\nname = \"Sales\"\nexclude = [\"data/**\"]\n",
        )
        .unwrap();
        for path in [
            "app.R",
            "data/big.csv",
            ".venv/lib/x.py",
            "R/__pycache__/x.pyc",
        ] {
            write(path);
        }
        let bundle = pack(folder.path()).unwrap();
        assert_eq!(bundle.id.as_deref(), Some("01J"));
        let names: Vec<String> = tar::Archive::new(flate2::read::GzDecoder::new(&bundle.bytes[..]))
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap().path().unwrap().display().to_string())
            .collect();
        assert_eq!(names, ["_ricochet.toml", "app.R"]);
    }

    #[test]
    fn a_folder_without_a_config_is_refused() {
        let folder = tempfile::tempdir().unwrap();
        assert!(pack(folder.path()).is_err());
    }
}
