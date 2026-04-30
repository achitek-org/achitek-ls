//! Shared utilities for the server.
//!
//! Achitek templates are ordinary `.tera` files that can reference prompt names
//! declared in a nearby `Achitekfile`. These helpers preserve the cross-file
//! behavior used by diagnostics, go-to-definition, references, and rename.

#[cfg(test)]
use crate::server::Document;
use crate::{analysis, server::Documents};
use anyhow::Context;
use lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, GotoDefinitionResponse, Hover, HoverContents,
    Location, MarkupContent, MarkupKind, NumberOrString, Position, Range, Uri,
};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub fn diagnostics(
    uri: &Uri,
    documents: &Documents,
) -> anyhow::Result<Vec<(Uri, Vec<LspDiagnostic>)>> {
    let Some(document) = documents.get(uri.as_str()) else {
        return Ok(Vec::new());
    };
    let Some(blueprint_dir) = blueprint_dir_from_uri(uri) else {
        return Ok(Vec::new());
    };

    let analysis = analysis::analyze(&document.text)
        .with_context(|| format!("failed to analyze document `{:?}`", uri))?;
    let prompt_names = prompt_name_set(&analysis);
    tracing::debug!(
        ?uri,
        prompt_count = prompt_names.len(),
        directory = %blueprint_dir.display(),
        "scanning template diagnostics"
    );

    scan_diagnostics(&blueprint_dir, &prompt_names)
}

pub fn template_diagnostics(
    uri: &Uri,
    documents: &Documents,
) -> anyhow::Result<Vec<LspDiagnostic>> {
    let Some(template_path) = file_path_from_uri(uri) else {
        tracing::debug!(?uri, "template diagnostics skipped for non-file URI");
        return Ok(Vec::new());
    };
    if !is_template_path(&template_path) {
        tracing::debug!(?uri, path = %template_path.display(), "template diagnostics skipped for non-template file");
        return Ok(Vec::new());
    }

    let Some(achitek_path) = find_achitekfile_for_template(&template_path) else {
        tracing::warn!(?uri, path = %template_path.display(), "could not find Achitekfile for template");
        return Ok(Vec::new());
    };

    let achitek_uri = path_to_uri(&achitek_path)?;
    let achitek_source = documents
        .get(achitek_uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&achitek_path).unwrap_or_default());
    let analysis = analysis::analyze(&achitek_source)
        .with_context(|| format!("failed to analyze `{}`", achitek_path.display()))?;
    let prompt_names = prompt_name_set(&analysis);
    let source = documents
        .get(uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&template_path).unwrap_or_default());

    Ok(unknown_references(&source, uri, &prompt_names))
}

pub fn definition(
    uri: &Uri,
    position: Position,
    documents: &Documents,
) -> anyhow::Result<Option<GotoDefinitionResponse>> {
    let Some(template_path) = file_path_from_uri(uri) else {
        tracing::debug!(?uri, "template definition skipped for non-file URI");
        return Ok(None);
    };
    if !is_template_path(&template_path) {
        tracing::debug!(?uri, path = %template_path.display(), "template definition skipped for non-template file");
        return Ok(None);
    }

    let source = documents
        .get(uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&template_path).unwrap_or_default());
    let Some(reference_name) = reference_at_position(&source, position) else {
        tracing::debug!(
            ?uri,
            line = position.line,
            character = position.character,
            "no template reference at position"
        );
        return Ok(None);
    };
    let Some(achitek_path) = find_achitekfile_for_template(&template_path) else {
        tracing::warn!(?uri, path = %template_path.display(), "could not find Achitekfile for template");
        return Ok(None);
    };

    let achitek_uri = path_to_uri(&achitek_path)?;
    let achitek_source = documents
        .get(achitek_uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&achitek_path).unwrap_or_default());
    let analysis = analysis::analyze(&achitek_source)
        .with_context(|| format!("failed to analyze `{}`", achitek_path.display()))?;
    let Some(symbol) = analysis.symbols().iter().find(|symbol| {
        symbol.kind() == analysis::SymbolKind::Prompt && symbol.name() == reference_name
    }) else {
        tracing::debug!(
            ?uri,
            reference = reference_name,
            "template reference has no matching prompt"
        );
        return Ok(None);
    };
    tracing::debug!(
        ?uri,
        reference = reference_name,
        target = ?achitek_uri,
        "resolved template definition"
    );

    Ok(Some(GotoDefinitionResponse::Scalar(Location::new(
        achitek_uri,
        Range {
            start: to_lsp_position(symbol.selection_range().start_position),
            end: to_lsp_position(symbol.selection_range().end_position),
        },
    ))))
}

pub fn hover(
    uri: &Uri,
    position: Position,
    documents: &Documents,
) -> anyhow::Result<Option<Hover>> {
    let Some(template_path) = file_path_from_uri(uri) else {
        tracing::debug!(?uri, "template hover skipped for non-file URI");
        return Ok(None);
    };
    if !is_template_path(&template_path) {
        tracing::debug!(?uri, path = %template_path.display(), "template hover skipped for non-template file");
        return Ok(None);
    }

    let source = documents
        .get(uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&template_path).unwrap_or_default());
    let Some(reference) = reference_at_position_with_range(&source, uri, position) else {
        return Ok(None);
    };
    let Some(achitek_path) = find_achitekfile_for_template(&template_path) else {
        tracing::warn!(?uri, path = %template_path.display(), "could not find Achitekfile for template");
        return Ok(None);
    };

    let achitek_uri = path_to_uri(&achitek_path)?;
    let achitek_source = documents
        .get(achitek_uri.as_str())
        .map(|document| document.text.clone())
        .unwrap_or_else(|| fs::read_to_string(&achitek_path).unwrap_or_default());
    let analysis = analysis::analyze(&achitek_source)
        .with_context(|| format!("failed to analyze `{}`", achitek_path.display()))?;
    let contents = if let Some(symbol) = analysis.symbols().iter().find(|symbol| {
        symbol.kind() == analysis::SymbolKind::Prompt && symbol.name() == reference.name
    }) {
        let detail = symbol
            .detail()
            .map(|detail| format!("\n\nKind: `{detail}`"))
            .unwrap_or_default();
        format!(
            "Achitek prompt reference `{}`{detail}\n\nDefined in `Achitekfile`.",
            symbol.name()
        )
    } else {
        format!(
            "Unknown Achitek prompt reference `{}`.\n\nDefine prompt `{}` in `Achitekfile` or rename this template reference.",
            reference.name, reference.name
        )
    };

    Ok(Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range: Some(reference.location.range),
    }))
}

pub fn scan_references(root: &Path, prompt_name: &str) -> anyhow::Result<Vec<Location>> {
    if !root.exists() {
        tracing::debug!(directory = %root.display(), prompt = prompt_name, "template reference scan skipped for missing directory");
        return Ok(Vec::new());
    }

    let mut locations = Vec::new();
    collect_references(root, prompt_name, &mut locations)?;
    tracing::debug!(
        directory = %root.display(),
        prompt = prompt_name,
        count = locations.len(),
        "scanned template references"
    );
    Ok(locations)
}

fn scan_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
) -> anyhow::Result<Vec<(Uri, Vec<LspDiagnostic>)>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut diagnostics = Vec::new();
    collect_diagnostics(root, prompt_names, &mut diagnostics)?;
    Ok(diagnostics)
}

fn collect_diagnostics(
    root: &Path,
    prompt_names: &HashSet<String>,
    diagnostics: &mut Vec<(Uri, Vec<LspDiagnostic>)>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_diagnostics(&path, prompt_names, diagnostics)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        diagnostics.push((uri.clone(), unknown_references(&source, &uri, prompt_names)));
    }

    Ok(())
}

fn unknown_references(
    source: &str,
    uri: &Uri,
    prompt_names: &HashSet<String>,
) -> Vec<LspDiagnostic> {
    identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| !prompt_names.contains(&reference.name))
        .map(|reference| LspDiagnostic {
            range: reference.location.range,
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("achitek".to_owned()),
            code: Some(NumberOrString::String(
                "unknown-template-prompt".to_owned(),
            )),
            message: format!(
                "unknown Achitek prompt reference `{}`; define prompt `{}` in Achitekfile or rename this template reference",
                reference.name, reference.name
            ),
            ..LspDiagnostic::default()
        })
        .collect()
}

fn collect_references(
    root: &Path,
    prompt_name: &str,
    locations: &mut Vec<Location>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)
        .with_context(|| format!("failed to read blueprint directory `{}`", root.display()))?
    {
        let entry = entry.context("failed to read blueprint directory entry")?;
        let path = entry.path();

        if path.is_dir() {
            collect_references(&path, prompt_name, locations)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("tera") {
            continue;
        }

        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read template `{}`", path.display()))?;
        let uri = path_to_uri(&path)
            .with_context(|| format!("failed to convert `{}` to a file URI", path.display()))?;
        locations.extend(references_in_source(&source, &uri, prompt_name));
    }

    Ok(())
}

fn references_in_source(source: &str, uri: &Uri, prompt_name: &str) -> Vec<Location> {
    identifiers_in_source(source, uri)
        .into_iter()
        .filter(|reference| reference.name == prompt_name)
        .map(|reference| reference.location)
        .collect()
}

fn reference_at_position(source: &str, position: Position) -> Option<String> {
    reference_at_position_with_range(source, &"file:///template.tera".parse().ok()?, position)
        .map(|reference| reference.name)
}

fn reference_at_position_with_range(
    source: &str,
    uri: &Uri,
    position: Position,
) -> Option<TemplateReference> {
    let column = usize::try_from(position.character).ok()?;

    identifiers_in_source(source, uri)
        .into_iter()
        .find(|reference| {
            let range = reference.location.range;
            range.start.line == position.line
                && usize::try_from(range.start.character)
                    .ok()
                    .is_some_and(|start| start <= column)
                && usize::try_from(range.end.character)
                    .ok()
                    .is_some_and(|end| column <= end)
        })
}

fn identifiers_in_source(source: &str, uri: &Uri) -> Vec<TemplateReference> {
    let mut references = Vec::new();
    let blocks = tera_blocks(source);
    let locals = template_locals(&blocks, source);

    for block in blocks {
        let tokens = tokenize_tera(source, block.content_start, block.content_end);
        match block.kind {
            TeraBlockKind::Output => {
                collect_expression_references(source, uri, &tokens, &locals, &mut references)
            }
            TeraBlockKind::Tag => {
                collect_tag_references(source, uri, &tokens, &locals, &mut references)
            }
        }
    }

    references
}

fn tera_blocks(source: &str) -> Vec<TeraBlock> {
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < source.len() {
        let Some(open_offset) = source[index..].find('{') else {
            break;
        };
        let open = index + open_offset;
        let Some(marker) = source.get(open..open + 2) else {
            break;
        };
        let Some((kind, close_marker)) = (match marker {
            "{{" => Some((TeraBlockKind::Output, "}}")),
            "{%" => Some((TeraBlockKind::Tag, "%}")),
            "{#" => {
                if let Some(close_offset) = source[open + 2..].find("#}") {
                    index = open + 2 + close_offset + 2;
                } else {
                    break;
                }
                continue;
            }
            _ => None,
        }) else {
            index = open + 1;
            continue;
        };

        let mut content_start = open + 2;
        if source[content_start..].starts_with('-') {
            content_start += 1;
        }

        let Some(close_offset) = source[content_start..].find(close_marker) else {
            break;
        };
        let close = content_start + close_offset;
        let content_end = close.saturating_sub(usize::from(
            close > content_start && source[..close].ends_with('-'),
        ));
        blocks.push(TeraBlock {
            kind,
            content_start,
            content_end,
        });
        index = close + 2;
    }

    blocks
}

fn template_locals(blocks: &[TeraBlock], source: &str) -> HashSet<String> {
    let mut locals = HashSet::new();

    for block in blocks
        .iter()
        .filter(|block| block.kind == TeraBlockKind::Tag)
    {
        let tokens = tokenize_tera(source, block.content_start, block.content_end);
        let Some(first) = token_identifier(&tokens, 0, source) else {
            continue;
        };

        match first {
            "for" => {
                let mut index = 1;
                while index < tokens.len() {
                    if token_identifier(&tokens, index, source) == Some("in") {
                        break;
                    }
                    if let Some(name) = token_identifier(&tokens, index, source)
                        && !is_tera_keyword(name)
                    {
                        locals.insert(name.to_owned());
                    }
                    index += 1;
                }
            }
            "set" | "set_global" => {
                if let Some(name) = token_identifier(&tokens, 1, source) {
                    locals.insert(name.to_owned());
                }
            }
            "macro" => {
                if let Some(name) = token_identifier(&tokens, 1, source) {
                    locals.insert(name.to_owned());
                }
                let mut in_arguments = false;
                for token in tokens.iter().skip(2) {
                    match token.kind {
                        TeraTokenKind::Symbol('(') => in_arguments = true,
                        TeraTokenKind::Symbol(')') => break,
                        TeraTokenKind::Identifier if in_arguments => {
                            locals.insert(source[token.start..token.end].to_owned());
                        }
                        _ => {}
                    }
                }
            }
            "import" | "from" => {
                for (index, token) in tokens.iter().enumerate() {
                    if token.kind == TeraTokenKind::Identifier
                        && previous_identifier(&tokens, index, source) == Some("as")
                    {
                        locals.insert(source[token.start..token.end].to_owned());
                    }
                }
            }
            _ => {}
        }
    }

    locals
}

fn collect_tag_references(
    source: &str,
    uri: &Uri,
    tokens: &[TeraToken],
    locals: &HashSet<String>,
    references: &mut Vec<TemplateReference>,
) {
    let Some(first) = token_identifier(tokens, 0, source) else {
        return;
    };

    match first {
        "if" | "elif" | "unless" | "with" => {
            collect_expression_references(source, uri, &tokens[1..], locals, references);
        }
        "for" => {
            if let Some(index) = tokens
                .iter()
                .position(|token| token_identifier_value(token, source) == Some("in"))
            {
                collect_expression_references(
                    source,
                    uri,
                    &tokens[index + 1..],
                    locals,
                    references,
                );
            }
        }
        "set" | "set_global" => {
            if let Some(index) = tokens
                .iter()
                .position(|token| token.kind == TeraTokenKind::Symbol('='))
            {
                collect_expression_references(
                    source,
                    uri,
                    &tokens[index + 1..],
                    locals,
                    references,
                );
            }
        }
        _ => {}
    }
}

fn collect_expression_references(
    source: &str,
    uri: &Uri,
    tokens: &[TeraToken],
    locals: &HashSet<String>,
    references: &mut Vec<TemplateReference>,
) {
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TeraTokenKind::Identifier {
            continue;
        }

        let name = &source[token.start..token.end];
        if !is_prompt_reference_token(source, tokens, index, locals) {
            continue;
        }

        references.push(TemplateReference {
            name: name.to_owned(),
            location: Location::new(
                uri.clone(),
                range_for_source_span(source, token.start, token.end),
            ),
        });
    }
}

fn is_prompt_reference_token(
    source: &str,
    tokens: &[TeraToken],
    index: usize,
    locals: &HashSet<String>,
) -> bool {
    let token = &tokens[index];
    let name = &source[token.start..token.end];

    if is_tera_keyword(name) || is_tera_builtin(name) || locals.contains(name) {
        return false;
    }
    if previous_significant_token(tokens, index).is_some_and(|previous| {
        previous.kind == TeraTokenKind::Symbol('.')
            || previous.kind == TeraTokenKind::Symbol('|')
            || token_identifier_value(previous, source) == Some("is")
            || token_identifier_value(previous, source) == Some("as")
    }) {
        return false;
    }
    if previous_test_not(tokens, index, source) {
        return false;
    }
    if next_significant_token(tokens, index).is_some_and(|next| {
        next.kind == TeraTokenKind::Symbol('(') || next.kind == TeraTokenKind::Symbol('=')
    }) {
        return false;
    }

    true
}

fn previous_test_not(tokens: &[TeraToken], index: usize, source: &str) -> bool {
    let Some(previous) = previous_significant_index(tokens, index) else {
        return false;
    };
    if token_identifier(&tokens, previous, source) != Some("not") {
        return false;
    }
    previous_significant_index(tokens, previous).is_some_and(|before_previous| {
        token_identifier(&tokens, before_previous, source) == Some("is")
    })
}

fn tokenize_tera(source: &str, start: usize, end: usize) -> Vec<TeraToken> {
    let mut tokens = Vec::new();
    let mut index = start;

    while index < end {
        let Some(ch) = source[index..end].chars().next() else {
            break;
        };

        if ch.is_whitespace() {
            index += ch.len_utf8();
            continue;
        }

        if ch == '"' || ch == '\'' {
            index = skip_quoted_string(source, index, end, ch);
            continue;
        }

        if is_identifier_start(ch) {
            let token_start = index;
            index += ch.len_utf8();
            while index < end {
                let Some(next) = source[index..end].chars().next() else {
                    break;
                };
                if !is_identifier_continue(next) {
                    break;
                }
                index += next.len_utf8();
            }
            tokens.push(TeraToken {
                kind: TeraTokenKind::Identifier,
                start: token_start,
                end: index,
            });
            continue;
        }

        if ch.is_ascii_digit() {
            index += ch.len_utf8();
            while index < end {
                let Some(next) = source[index..end].chars().next() else {
                    break;
                };
                if !(next.is_ascii_digit() || next == '.') {
                    break;
                }
                index += next.len_utf8();
            }
            continue;
        }

        let token_start = index;
        if is_two_char_operator(source, index, end) {
            index += 2;
            tokens.push(TeraToken {
                kind: TeraTokenKind::Operator,
                start: token_start,
                end: index,
            });
            continue;
        }

        index += ch.len_utf8();
        tokens.push(TeraToken {
            kind: match ch {
                '.' | '|' | '(' | ')' | ',' | '[' | ']' | ':' => TeraTokenKind::Symbol(ch),
                '=' => TeraTokenKind::Symbol('='),
                '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' => TeraTokenKind::Operator,
                _ => TeraTokenKind::Symbol(ch),
            },
            start: token_start,
            end: index,
        });
    }

    tokens
}

fn skip_quoted_string(source: &str, start: usize, end: usize, quote: char) -> usize {
    let mut escaped = false;
    let mut index = start + quote.len_utf8();

    while index < end {
        let Some(ch) = source[index..end].chars().next() else {
            break;
        };
        index += ch.len_utf8();
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == quote {
            break;
        }
    }

    index
}

fn is_two_char_operator(source: &str, index: usize, end: usize) -> bool {
    if index + 2 > end {
        return false;
    }

    matches!(
        &source[index..index + 2],
        "==" | "!=" | ">=" | "<=" | "//" | "**"
    )
}

fn range_for_source_span(source: &str, start: usize, end: usize) -> Range {
    Range {
        start: position_for_offset(source, start),
        end: position_for_offset(source, end),
    }
}

fn position_for_offset(source: &str, offset: usize) -> Position {
    let mut line = 0_u32;
    let mut line_start = 0;

    for (index, ch) in source.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = index + ch.len_utf8();
        }
    }

    Position {
        line,
        character: u32::try_from(offset.saturating_sub(line_start))
            .expect("column should fit into u32"),
    }
}

fn previous_significant_token(tokens: &[TeraToken], index: usize) -> Option<&TeraToken> {
    previous_significant_index(tokens, index).map(|previous| &tokens[previous])
}

fn previous_significant_index(_tokens: &[TeraToken], index: usize) -> Option<usize> {
    index.checked_sub(1)
}

fn next_significant_token(tokens: &[TeraToken], index: usize) -> Option<&TeraToken> {
    tokens.get(index + 1)
}

fn previous_identifier<'a>(tokens: &[TeraToken], index: usize, source: &'a str) -> Option<&'a str> {
    previous_significant_token(tokens, index)
        .and_then(|token| token_identifier_value(token, source))
}

fn token_identifier<'a>(tokens: &[TeraToken], index: usize, source: &'a str) -> Option<&'a str> {
    tokens
        .get(index)
        .and_then(|token| token_identifier_value(token, source))
}

fn token_identifier_value<'a>(token: &TeraToken, source: &'a str) -> Option<&'a str> {
    (token.kind == TeraTokenKind::Identifier).then_some(&source[token.start..token.end])
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_tera_keyword(name: &str) -> bool {
    matches!(
        name,
        "and"
            | "as"
            | "block"
            | "elif"
            | "else"
            | "endblock"
            | "endfor"
            | "endif"
            | "extends"
            | "false"
            | "filter"
            | "for"
            | "if"
            | "in"
            | "include"
            | "loop"
            | "not"
            | "or"
            | "set"
            | "set_global"
            | "true"
            | "unless"
            | "with"
    )
}

fn is_tera_builtin(name: &str) -> bool {
    matches!(
        name,
        "abs"
            | "attr"
            | "batch"
            | "capitalize"
            | "concat"
            | "config"
            | "date"
            | "default"
            | "dictsort"
            | "dump"
            | "escape"
            | "filesizeformat"
            | "filter"
            | "first"
            | "float"
            | "get"
            | "get_env"
            | "group_by"
            | "int"
            | "join"
            | "json_encode"
            | "last"
            | "length"
            | "linebreaksbr"
            | "lower"
            | "map"
            | "matching"
            | "not_matching"
            | "now"
            | "number"
            | "odd"
            | "pluralize"
            | "range"
            | "replace"
            | "reverse"
            | "round"
            | "safe"
            | "slice"
            | "slugify"
            | "sort"
            | "split"
            | "string"
            | "striptags"
            | "super"
            | "throw"
            | "title"
            | "trim"
            | "truncate"
            | "unique"
            | "upper"
            | "urlencode"
            | "wordcount"
    )
}

fn prompt_name_set(analysis: &analysis::Analysis) -> HashSet<String> {
    analysis
        .symbols()
        .iter()
        .filter(|symbol| symbol.kind() == analysis::SymbolKind::Prompt)
        .map(|symbol| symbol.name().to_owned())
        .collect()
}

pub fn is_template_uri(uri: &Uri) -> bool {
    file_path_from_uri(uri).is_some_and(|path| is_template_path(&path))
}

fn is_template_path(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("tera")
}

fn find_achitekfile_for_template(template_path: &Path) -> Option<PathBuf> {
    let mut dir = template_path.parent()?;
    loop {
        let candidate = dir.join("Achitekfile");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

pub fn blueprint_dir_from_uri(uri: &Uri) -> Option<PathBuf> {
    let raw = uri.as_str();
    let path = raw.strip_prefix("file://")?;
    let path = if cfg!(windows) && path.starts_with('/') {
        &path[1..]
    } else {
        path
    };
    Path::new(path).parent().map(Path::to_path_buf)
}

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

pub fn path_to_uri(path: &Path) -> anyhow::Result<Uri> {
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize `{}`", path.display()))?;
    let value = format!("file://{}", path.to_string_lossy());
    value
        .parse()
        .with_context(|| format!("failed to parse `{value}` as a URI"))
}

fn to_lsp_position(position: crate::syntax::TextPosition) -> Position {
    Position {
        line: u32::try_from(position.row).expect("line should fit into u32"),
        character: u32::try_from(position.column).expect("column should fit into u32"),
    }
}

#[derive(Debug, Clone)]
struct TemplateReference {
    name: String,
    location: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeraBlockKind {
    Output,
    Tag,
}

#[derive(Debug, Clone, Copy)]
struct TeraBlock {
    kind: TeraBlockKind,
    content_start: usize,
    content_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TeraTokenKind {
    Identifier,
    Symbol(char),
    Operator,
}

#[derive(Debug, Clone, Copy)]
struct TeraToken {
    kind: TeraTokenKind,
    start: usize,
    end: usize,
}

/// Returns a unique temporary directory path for a server test.
///
/// This helper is meant only for tests. The directory is not created
/// automatically; callers should create it with `fs::create_dir_all` and remove
/// it when the test is done.
#[cfg(test)]
pub fn temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
    Ok(std::env::temp_dir().join(format!(
        "{prefix}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    )))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::server::utils;
    use indoc::indoc;

    #[test]
    fn scan_references_finds_prompt_uses_in_template_files() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-references")?;
        fs::create_dir_all(&temp_root)?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                [package]
                name = "{{project}}"
                repository = "{{repo}}"

                {% if dev_profile == "FastCompile" -%}
                [profile.dev]
                debug = 0
                {% endif %}
            "#},
        )?;

        let references = scan_references(&temp_root, "repo")?;

        assert_eq!(references.len(), 1);
        assert_eq!(references[0].range.start.line, 2);
        assert_eq!(references[0].range.start.character, 16);

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn diagnostics_reports_unknown_template_references() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-diagnostics")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                [package]
                name = "{{project_name}}"
                description = "{{missing_prompt}}"
            "#},
        )?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let template_uri = path_to_uri(&template_path)?;
        let documents = Documents::from([(
            achitek_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source(),
            },
        )]);

        let diagnostics = diagnostics(&achitek_uri, &documents)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].0, template_uri);
        assert_eq!(diagnostics[0].1.len(), 1);
        assert_eq!(
            diagnostics[0].1[0].message,
            "unknown Achitek prompt reference `missing_prompt`; define prompt `missing_prompt` in Achitekfile or rename this template reference"
        );
        assert_eq!(diagnostics[0].1[0].source.as_deref(), Some("achitek"));
        assert_eq!(
            diagnostics[0].1[0].code,
            Some(NumberOrString::String("unknown-template-prompt".to_owned()))
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    #[test]
    fn diagnostics_ignore_tera_builtins_and_template_locals() -> anyhow::Result<()> {
        let temp_root = utils::temp_dir("achitek-template-builtins")?;
        fs::create_dir_all(&temp_root)?;
        let achitek_path = temp_root.join("Achitekfile");
        fs::write(&achitek_path, source())?;
        let template_path = temp_root.join("Cargo.toml.tera");
        fs::write(
            &template_path,
            indoc! {r#"
                {% set title = project_name | upper %}
                {% for item in project_names | default(value=[]) %}
                name = "{{ item | lower }}"
                {% endfor %}
                year = "{{ now() | date(format="%Y") }}"
                missing = "{{ get_env(name="HOME", default=missing_prompt) }}"
            "#},
        )?;
        let achitek_uri = path_to_uri(&achitek_path)?;
        let documents = Documents::from([(
            achitek_uri.as_str().to_owned(),
            Document {
                version: 1,
                text: source_with_project_names(),
            },
        )]);

        let diagnostics = diagnostics(&achitek_uri, &documents)?;

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].1.len(), 1);
        assert_eq!(
            diagnostics[0].1[0].message,
            "unknown Achitek prompt reference `missing_prompt`; define prompt `missing_prompt` in Achitekfile or rename this template reference"
        );

        fs::remove_dir_all(&temp_root)?;
        Ok(())
    }

    fn source() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }
        "#}
        .to_owned()
    }

    fn source_with_project_names() -> String {
        indoc! {r#"
            blueprint {
              version = "1.0.0"
              name = "minimal"
            }

            prompt "project_name" {
              type = string
            }

            prompt "project_names" {
              type = multiselect
              choices = ["one"]
            }
        "#}
        .to_owned()
    }
}
