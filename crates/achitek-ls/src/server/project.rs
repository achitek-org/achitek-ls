//! Blueprint project context for request handlers.
//!
//! Handlers often need the same small set of cross-file facts: which blueprint
//! project owns a URI, where its Achitekfile lives, and whether source should
//! come from an open editor buffer or disk. This module keeps that plumbing out
//! of individual LSP request handlers.

use crate::server::{ServerState, utils};
use anyhow::Context;
use lsp_types::{Location, Uri};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Project-level context for a single blueprint project.
#[derive(Debug, Clone)]
pub(crate) struct ProjectContext<'a> {
    state: &'a ServerState,
    root: PathBuf,
    achitekfile: PathBuf,
}

impl<'a> ProjectContext<'a> {
    /// Builds project context for the project that owns `uri`.
    pub(crate) fn for_uri(state: &'a ServerState, uri: &Uri) -> Option<Self> {
        let path = utils::file_path_from_uri(uri)?;
        Self::for_path(state, &path).or_else(|| {
            is_achitekfile_path(&path).then(|| Self::from_achitekfile_path(state, path))?
        })
    }

    /// Builds project context for the project that owns `template_path`.
    pub(crate) fn for_template_path(state: &'a ServerState, template_path: &Path) -> Option<Self> {
        Self::for_path(state, template_path).or_else(|| {
            let achitekfile = utils::find_achitekfile_for_template(template_path)?;
            Self::from_achitekfile_path(state, achitekfile)
        })
    }

    /// Returns the Achitekfile path for this project.
    pub(crate) fn achitekfile_path(&self) -> &Path {
        &self.achitekfile
    }

    /// Returns the Achitekfile URI for this project.
    pub(crate) fn achitekfile_uri(&self) -> anyhow::Result<Uri> {
        utils::path_to_uri(&self.achitekfile)
    }

    /// Returns Achitekfile source, preferring an open editor buffer over disk.
    pub(crate) fn achitekfile_source(&self) -> anyhow::Result<String> {
        let uri = self.achitekfile_uri()?;
        self.source_for(&uri, &self.achitekfile, "Achitekfile")
    }

    /// Returns template source, preferring an open editor buffer over disk.
    pub(crate) fn template_source(&self, uri: &Uri, path: &Path) -> anyhow::Result<String> {
        self.source_for(uri, path, "template")
    }

    /// Scans this project for Tera references to `prompt_name`.
    pub(crate) fn scan_template_references(
        &self,
        prompt_name: &str,
    ) -> anyhow::Result<Vec<Location>> {
        utils::scan_references(&self.root, prompt_name)
    }

    fn for_path(state: &'a ServerState, path: &Path) -> Option<Self> {
        let project = state.workspace.project_for_path(path)?;
        Some(Self {
            state,
            root: project.root().to_path_buf(),
            achitekfile: project.achitekfile().to_path_buf(),
        })
    }

    fn from_achitekfile_path(state: &'a ServerState, achitekfile: PathBuf) -> Option<Self> {
        let root = achitekfile.parent()?.to_path_buf();
        Some(Self {
            state,
            root,
            achitekfile,
        })
    }

    fn source_for(&self, uri: &Uri, path: &Path, label: &str) -> anyhow::Result<String> {
        self.state
            .documents
            .get(uri.as_str())
            .map(|document| Ok(document.text.clone()))
            .unwrap_or_else(|| {
                fs::read_to_string(path)
                    .with_context(|| format!("failed to read {label} `{}`", path.display()))
            })
    }
}

fn is_achitekfile_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("achitekfile"))
}
