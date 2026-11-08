//! `Oru.toml`：项目清单（M0 只做最小 package 字段，依赖解析留给后续）。

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const MANIFEST_NAME: &str = "Oru.toml";

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub package: Package,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_entry() -> String {
    "src/main.oru".to_string()
}

#[derive(Debug, Clone)]
pub struct ManifestError(pub String);

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ManifestError {}

impl ManifestError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| ManifestError::new(format!("读不到 {}：{error}", path.display())))?;
        let manifest: Self = toml::from_str(&text)
            .map_err(|error| ManifestError::new(format!("{} 不合法：{error}", path.display())))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn load_in_dir(dir: &Path) -> Result<Self, ManifestError> {
        Self::load(&dir.join(MANIFEST_NAME))
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let name = &self.package.name;
        if !(2..=50).contains(&name.len())
            || !name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_'
            })
            || !name.as_bytes()[0].is_ascii_lowercase()
        {
            return Err(ManifestError::new(
                "package.name 必须是小写字母/数字/`-`/`_`，2–50 个字符，并以字母开头",
            ));
        }
        if self.package.version.trim().is_empty() {
            return Err(ManifestError::new("package.version 不能为空"));
        }
        if self
            .package
            .edition
            .as_deref()
            .is_some_and(|edition| edition != "2026")
        {
            return Err(ManifestError::new("M0 只支持 package.edition = \"2026\""));
        }
        let entry = Path::new(&self.package.entry);
        if entry.is_absolute()
            || entry
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(ManifestError::new("package.entry 必须是项目内相对路径"));
        }
        if entry.extension().and_then(|extension| extension.to_str()) != Some("oru") {
            return Err(ManifestError::new("package.entry 必须是 .oru 文件"));
        }
        Ok(())
    }

    pub fn entry_path(&self, dir: &Path) -> PathBuf {
        dir.join(&self.package.entry)
    }
}

/// 从 `start` 向上找 `Oru.toml`，返回项目目录。
pub fn find_project(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    if current.is_file() {
        current.pop();
    }
    loop {
        if current.join(MANIFEST_NAME).is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}
