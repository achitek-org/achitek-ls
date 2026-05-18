//! Workspace and blueprint project discovery.
//!
//! The workspace is responsible for discovering blueprint projects from the
//! repository manifest. Individual projects remain independent analysis
//! universes; the workspace only routes a document to the project that owns it.

use anyhow::Context;
use lsp_types::Uri;
use std::{
    fs,
    path::{Path, PathBuf},
};

const MANIFEST_FILE_NAME: &str = "blueprints.toml";
const ACHITEKFILE_NAME: &str = "achitekfile";

/// The language surface represented by a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    /// The root Achitekfile for a blueprint project.
    Achitekfile,
    /// A Tera template file or Tera-templated path inside a blueprint project.
    TeraTemplate,
    /// The top-level blueprint workspace manifest.
    Manifest,
    /// A file outside the language server's known language surfaces.
    Unknown,
}

impl DocumentKind {
    /// Classifies a document by LSP language id and file path.
    pub fn classify(language_id: Option<&str>, path: Option<&Path>) -> Self {
        match language_id {
            Some("achitekfile") => return Self::Achitekfile,
            Some("tera") => return Self::TeraTemplate,
            Some("toml") if path.is_some_and(is_manifest_path) => return Self::Manifest,
            _ => {}
        }

        let Some(path) = path else {
            return Self::Unknown;
        };

        if is_manifest_path(path) {
            Self::Manifest
        } else if is_achitekfile_path(path) {
            Self::Achitekfile
        } else if is_template_path(path) {
            Self::TeraTemplate
        } else {
            Self::Unknown
        }
    }
}

/// Discovered workspace state.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    root: Option<PathBuf>,
    projects: Vec<BlueprintProject>,
}

impl Workspace {
    /// Discovers blueprint projects from `blueprints.toml` under `root`.
    pub fn discover(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = root.into();
        let manifest_path = root.join(MANIFEST_FILE_NAME);

        if !manifest_path.exists() {
            tracing::debug!(
                root = %root.display(),
                manifest = MANIFEST_FILE_NAME,
                "workspace manifest not found"
            );
            return Ok(Self {
                root: Some(root),
                projects: Vec::new(),
            });
        }

        let manifest_source = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read `{}`", manifest_path.display()))?;
        let manifest = WorkspaceManifest::parse(&manifest_source)
            .with_context(|| format!("failed to parse `{}`", manifest_path.display()))?;
        let projects = manifest
            .projects
            .into_iter()
            .map(|entry| BlueprintProject::discover(entry.name, root.join(entry.path)))
            .collect::<anyhow::Result<Vec<_>>>()?;

        tracing::debug!(
            root = %root.display(),
            project_count = projects.len(),
            "discovered blueprint workspace"
        );

        Ok(Self {
            root: Some(root),
            projects,
        })
    }

    /// Returns the workspace root used for discovery.
    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    /// Returns discovered blueprint projects.
    pub fn projects(&self) -> &[BlueprintProject] {
        &self.projects
    }

    /// Returns the project that owns `uri`, if any.
    pub fn project_for_uri(&self, uri: &Uri) -> Option<&BlueprintProject> {
        let path = file_path_from_uri(uri)?;
        self.project_for_path(&path)
    }

    /// Returns the project that owns `path`, if any.
    pub fn project_for_path(&self, path: &Path) -> Option<&BlueprintProject> {
        self.projects
            .iter()
            .filter(|project| project.contains_path(path))
            .max_by_key(|project| project.root.components().count())
    }
}

/// A manifest-defined blueprint project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintProject {
    name: String,
    root: PathBuf,
    achitekfile: PathBuf,
    templates: Vec<PathBuf>,
}

impl BlueprintProject {
    fn discover(name: String, root: PathBuf) -> anyhow::Result<Self> {
        let achitekfile = root.join(ACHITEKFILE_NAME);
        let templates = discover_templates(&root)?;

        tracing::debug!(
            project = name,
            root = %root.display(),
            achitekfile = %achitekfile.display(),
            template_count = templates.len(),
            "discovered blueprint project"
        );

        Ok(Self {
            name,
            root,
            achitekfile,
            templates,
        })
    }

    /// Returns the manifest name for the blueprint project.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the blueprint project root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the root Achitekfile path.
    pub fn achitekfile(&self) -> &Path {
        &self.achitekfile
    }

    /// Returns template paths discovered under the project root.
    pub fn templates(&self) -> &[PathBuf] {
        &self.templates
    }

    /// Returns whether `path` belongs to this blueprint project.
    pub fn contains_path(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }

    /// Classifies a path in the context of this blueprint project.
    pub fn document_kind(&self, path: &Path) -> DocumentKind {
        DocumentKind::classify(None, Some(path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceManifest {
    projects: Vec<ManifestProject>,
}

impl WorkspaceManifest {
    fn parse(source: &str) -> anyhow::Result<Self> {
        let mut projects = Vec::new();
        let mut current: Option<ManifestProject> = None;

        for (index, raw_line) in source.lines().enumerate() {
            let line = raw_line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(name) = table_name(line) {
                if let Some(project) = current.take() {
                    projects.push(project);
                }
                current = Some(ManifestProject {
                    name: name.to_owned(),
                    path: PathBuf::new(),
                });
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                anyhow::bail!(
                    "expected TOML table or key-value pair on line {}",
                    index + 1
                );
            };

            if key.trim() != "path" {
                continue;
            }

            let Some(project) = current.as_mut() else {
                anyhow::bail!("`path` must appear inside a blueprint table");
            };
            project.path = parse_string(value.trim())
                .with_context(|| format!("failed to parse path on line {}", index + 1))?
                .into();
        }

        if let Some(project) = current {
            projects.push(project);
        }

        for project in &projects {
            if project.path.as_os_str().is_empty() {
                anyhow::bail!("blueprint `{}` is missing a path", project.name);
            }
        }

        Ok(Self { projects })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManifestProject {
    name: String,
    path: PathBuf,
}

fn table_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

fn parse_string(value: &str) -> anyhow::Result<String> {
    let value = value
        .split_once('#')
        .map_or(value, |(value, _comment)| value)
        .trim();
    let Some(value) = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        anyhow::bail!("expected quoted string");
    };

    Ok(value.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn discover_templates(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut templates = Vec::new();
    collect_templates(root, &mut templates)?;
    templates.sort();
    Ok(templates)
}

fn collect_templates(root: &Path, templates: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in
        fs::read_dir(root).with_context(|| format!("failed to read `{}`", root.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read `{}` entry", root.display()))?;
        let path = entry.path();

        if path.is_dir() {
            collect_templates(&path, templates)?;
        } else if is_template_path(&path) {
            templates.push(path);
        }
    }

    Ok(())
}

fn is_manifest_path(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_FILE_NAME)
}

fn is_achitekfile_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case(ACHITEKFILE_NAME))
}

fn is_template_path(path: &Path) -> bool {
    let path_text = path.to_string_lossy();

    path.extension().and_then(|ext| ext.to_str()) == Some("tera")
        || path_text.contains("{%")
        || path_text.contains("{{")
        || path_text.contains("{#")
}

/// Converts a file URI to a path.
pub fn file_path_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[test]
    fn parses_blueprint_manifest() {
        let manifest = WorkspaceManifest::parse(indoc! {r#"
            [rust]
            path = "./rust"

            [python-fast-api]
            path = "./python-fast-api"
        "#})
        .expect("manifest should parse");

        assert_eq!(manifest.projects.len(), 2);
        assert_eq!(manifest.projects[0].name, "rust");
        assert_eq!(manifest.projects[0].path, PathBuf::from("./rust"));
        assert_eq!(manifest.projects[1].name, "python-fast-api");
        assert_eq!(
            manifest.projects[1].path,
            PathBuf::from("./python-fast-api")
        );
    }

    #[test]
    fn discovers_manifest_defined_projects() -> anyhow::Result<()> {
        let root = temp_dir("achitek-workspace-discovery")?;
        fs::create_dir_all(root.join("rust/src"))?;
        fs::create_dir_all(root.join("python-fast-api"))?;
        fs::write(
            root.join(MANIFEST_FILE_NAME),
            indoc! {r#"
                [rust]
                path = "./rust"

                [python-fast-api]
                path = "./python-fast-api"
            "#},
        )?;
        fs::write(root.join("rust").join(ACHITEKFILE_NAME), "")?;
        fs::write(root.join("rust/Cargo.toml.tera"), "")?;
        fs::write(
            root.join(
                "rust/src/{% if kind == 'bin' %}main.rs.tera{% else %}lib.rs.tera{% endif %}",
            ),
            "",
        )?;
        fs::write(root.join("python-fast-api").join(ACHITEKFILE_NAME), "")?;
        fs::write(root.join("python-fast-api/README.md.tera"), "")?;

        let workspace = Workspace::discover(&root)?;

        assert_eq!(workspace.root(), Some(root.as_path()));
        assert_eq!(workspace.projects().len(), 2);
        assert_eq!(workspace.projects()[0].name(), "rust");
        assert_eq!(workspace.projects()[0].templates().len(), 2);
        assert_eq!(workspace.projects()[1].name(), "python-fast-api");
        assert_eq!(workspace.projects()[1].templates().len(), 1);

        fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn classifies_known_document_kinds() {
        assert_eq!(
            DocumentKind::classify(None, Some(Path::new("/workspace/rust/achitekfile"))),
            DocumentKind::Achitekfile
        );
        assert_eq!(
            DocumentKind::classify(None, Some(Path::new("/workspace/rust/Cargo.toml.tera"))),
            DocumentKind::TeraTemplate
        );
        assert_eq!(
            DocumentKind::classify(None, Some(Path::new("/workspace/blueprints.toml"))),
            DocumentKind::Manifest
        );
    }

    fn temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
        Ok(std::env::temp_dir().join(format!(
            "{prefix}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        )))
    }
}
