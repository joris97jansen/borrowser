//! AG6 build input only. Execution consumes the generated existing Fixture model.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureMetadata {
    id: String,
    input: String,
    requires_javascript: bool,
    classification: ClassificationMetadata,
    observations: ObservationMetadata,
    expectation: Option<ExpectationMetadata>,
    expected: Option<ExpectedMetadata>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ClassificationMetadata {
    Runnable {},
    Unsupported { reason: String },
    Skipped { reason: String },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ExpectationMetadata {
    Match {},
    KnownFailure { reason: String },
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ExpectedMetadata {
    Canvas { rgb8: [u8; 3] },
    Projection { path: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationMetadata {
    dom: Option<FamilyMetadata>,
    #[serde(default)]
    styles: Vec<StyleMetadata>,
    layout: Option<LayoutMetadata>,
    canvas: Option<FamilyMetadata>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum FamilyMetadata {
    Compare {},
    Unsupported { reason: String },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StyleMetadata {
    target: String,
    property: String,
    selection: FamilyMetadata,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum LayoutMetadata {
    Compare { targets: Vec<String> },
    Unsupported { reason: String },
}

// Rust's string Debug formatter emits a quoted, escaped Rust string literal.
// Never interpolate metadata as identifiers, expressions, or raw string bodies.
fn literal(value: &str) -> String {
    format!("{value:?}")
}

fn reason(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err("reason must not be blank".into())
    } else {
        Ok(())
    }
}

fn relative(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|part| matches!(part, "" | "." | ".."))
    {
        Err(format!("invalid relative path {value:?}"))
    } else {
        Ok(())
    }
}

fn walk(root: &Path, relative_dir: &str, files: &mut BTreeSet<String>) -> Result<(), String> {
    let directory = root.join(relative_dir);
    let metadata =
        fs::symlink_metadata(&directory).map_err(|e| format!("fixtures/{relative_dir}: {e}"))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "fixtures/{relative_dir}: expected a regular directory"
        ));
    }
    let mut entries = fs::read_dir(&directory)
        .map_err(|e| format!("fixtures/{relative_dir}: {e}"))?
        .map(|entry| {
            let entry = entry.map_err(|e| format!("fixtures/{relative_dir}: {e}"))?;
            entry
                .file_name()
                .into_string()
                .map_err(|_| format!("fixtures/{relative_dir}: non-UTF-8 entry name"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort();
    for name in entries {
        let path = if relative_dir.is_empty() {
            name
        } else {
            format!("{relative_dir}/{name}")
        };
        relative(&path).map_err(|e| format!("fixtures/{path}: {e}"))?;
        let metadata =
            fs::symlink_metadata(root.join(&path)).map_err(|e| format!("fixtures/{path}: {e}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("fixtures/{path}: symlinks are not allowed"));
        }
        if metadata.is_dir() {
            walk(root, &path, files)?;
        } else if metadata.is_file() {
            files.insert(path);
        } else {
            return Err(format!("fixtures/{path}: expected a regular file"));
        }
    }
    Ok(())
}

fn reference(declaration: &str, value: &str, files: &BTreeSet<String>) -> Result<String, String> {
    relative(value)?;
    let path = match declaration.rsplit_once('/') {
        Some((parent, _)) => format!("{parent}/{value}"),
        None => value.into(),
    };
    if !files.contains(&path) {
        return Err(format!("missing regular file fixtures/{path}"));
    }
    Ok(path)
}

fn embedded(path: &str, text: bool) -> String {
    format!(
        "{}!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {}))",
        if text { "include_str" } else { "include_bytes" },
        literal(&format!("/fixtures/{path}"))
    )
}

fn family(selection: Option<&FamilyMetadata>) -> Result<String, String> {
    Ok(match selection {
        None => "Request::NotRequested".into(),
        Some(FamilyMetadata::Compare {}) => "Request::Compare(())".into(),
        Some(FamilyMetadata::Unsupported { reason: text }) => {
            reason(text)?;
            format!("Request::Unsupported {{ reason: {} }}", literal(text))
        }
    })
}

fn observations(value: &ObservationMetadata) -> Result<String, String> {
    if value.dom.is_none()
        && value.styles.is_empty()
        && value.layout.is_none()
        && value.canvas.is_none()
    {
        return Err("observations: at least one observation must be requested".into());
    }
    let mut styles = Vec::new();
    for field in &value.styles {
        if field.target.is_empty() || field.property.trim().is_empty() {
            return Err("observations.styles: target and property must not be empty".into());
        }
        let property = match &field.selection {
            FamilyMetadata::Compare {} => {
                let variant = match field.property.as_str() {
                    "color" => "Color",
                    "background-color" => "BackgroundColor",
                    "font-size" => "FontSize",
                    "display" => "Display",
                    _ => {
                        return Err(format!(
                            "observations.styles: unsupported compared property {:?}",
                            field.property
                        ));
                    }
                };
                format!("Request::Compare(StyleProperty::{variant})")
            }
            FamilyMetadata::Unsupported { reason: text } => {
                reason(text)?;
                format!("Request::Unsupported {{ reason: {} }}", literal(text))
            }
        };
        styles.push(format!(
            "StyleField {{ target: {}, name: {}, property: {property} }}",
            literal(&field.target),
            literal(&field.property)
        ));
    }
    let layout = match &value.layout {
        None => "Request::NotRequested".into(),
        Some(LayoutMetadata::Compare { targets }) => format!(
            "Request::Compare(&[{}])",
            targets
                .iter()
                .map(|s| literal(s))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Some(LayoutMetadata::Unsupported { reason: text }) => {
            reason(text)?;
            format!("Request::Unsupported {{ reason: {} }}", literal(text))
        }
    };
    Ok(format!(
        "ObservationRequest {{ dom: {}, styles: &[{}], layout: {layout}, canvas: {} }}",
        family(value.dom.as_ref())?,
        styles.join(","),
        family(value.canvas.as_ref())?
    ))
}

/// Discover only the fixed crate fixture root. No runtime files are produced.
/// The returned source is deterministic across checkout/build locations.
pub fn generate(manifest_dir: &Path) -> Result<String, String> {
    let root = manifest_dir.join("fixtures");
    let mut files = BTreeSet::new();
    walk(&root, "", &mut files)?;
    let declarations: Vec<_> = files
        .iter()
        .filter(|s| s.ends_with(".fixture.json"))
        .collect();
    if declarations.is_empty() {
        return Err("fixtures: no *.fixture.json declarations discovered".into());
    }
    let mut fixtures = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    for path in declarations {
        let compile = || -> Result<(String, String, Vec<String>), String> {
            let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
            let metadata: FixtureMetadata =
                serde_json::from_slice(&bytes).map_err(|e| format!("metadata: {e}"))?;
            if metadata.id.split('/').any(|part| {
                part.is_empty()
                    || !part.as_bytes()[0].is_ascii_alphanumeric()
                    || !part.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'
                    })
            }) {
                return Err(format!("id: invalid fixture identity {:?}", metadata.id));
            }
            let input =
                reference(path, &metadata.input, &files).map_err(|e| format!("input: {e}"))?;
            if !input.ends_with(".html") {
                return Err("input: expected an .html file".into());
            }
            let html = fs::read(root.join(&input)).map_err(|e| format!("input: {e}"))?;
            std::str::from_utf8(&html).map_err(|e| format!("input: expected UTF-8: {e}"))?;
            let mut references = vec![input.clone()];
            let request = observations(&metadata.observations)?;
            let kind = match &metadata.classification {
                ClassificationMetadata::Runnable {} => {
                    if metadata.requires_javascript {
                        return Err("requires_javascript: runnable fixtures must not require page JavaScript".into());
                    }
                    let expectation = match metadata.expectation.as_ref() {
                        Some(ExpectationMetadata::Match {}) => "Expectation::Match".into(),
                        Some(ExpectationMetadata::KnownFailure { reason: text }) => {
                            reason(text).map_err(|e| format!("expectation: {e}"))?;
                            format!("Expectation::KnownFailure {{ reason: {} }}", literal(text))
                        }
                        None => return Err("expectation: required for runnable fixtures".into()),
                    };
                    let expected = match metadata.expected.as_ref() {
                        Some(ExpectedMetadata::Canvas { rgb8 }) => {
                            format!("Expected::Canvas(CanvasColor({rgb8:?}))")
                        }
                        Some(ExpectedMetadata::Projection {
                            path: expected_path,
                        }) => {
                            let expected_path = reference(path, expected_path, &files)
                                .map_err(|e| format!("expected.path: {e}"))?;
                            if !expected_path.ends_with(".expected.json") {
                                return Err("expected.path: expected an .expected.json file".into());
                            }
                            let bytes = fs::read(root.join(&expected_path))
                                .map_err(|e| format!("expected.path: {e}"))?;
                            std::str::from_utf8(&bytes)
                                .map_err(|e| format!("expected.path: expected UTF-8: {e}"))?;
                            references.push(expected_path.clone());
                            format!("Expected::Projection({})", embedded(&expected_path, true))
                        }
                        None => return Err("expected: required for runnable fixtures".into()),
                    };
                    format!(
                        "FixtureKind::Runnable {{ html: {}, expected: {expected}, expectation: {expectation} }}",
                        embedded(&input, false)
                    )
                }
                ClassificationMetadata::Unsupported { reason: text }
                | ClassificationMetadata::Skipped { reason: text } => {
                    if metadata.expectation.is_some() || metadata.expected.is_some() {
                        return Err("classification: excluded fixtures must not declare expectation or expected data".into());
                    }
                    reason(text).map_err(|e| format!("classification: {e}"))?;
                    let variant = if matches!(
                        metadata.classification,
                        ClassificationMetadata::Unsupported { .. }
                    ) {
                        "Unsupported"
                    } else {
                        "Skipped"
                    };
                    format!("FixtureKind::{variant} {{ reason: {} }}", literal(text))
                }
            };
            Ok((
                metadata.id.clone(),
                format!(
                    "Fixture {{ id: TestId({}), source: {}, metadata_source: {}, requires_javascript: {}, request: {request}, kind: {kind} }}",
                    literal(&metadata.id),
                    literal(&format!("fixtures/{input}")),
                    literal(&format!("fixtures/{path}")),
                    metadata.requires_javascript
                ),
                references,
            ))
        };
        let (id, initializer, references) =
            compile().map_err(|e| format!("fixtures/{path}: {e}"))?;
        if let Some((previous, _)) = fixtures.insert(id.clone(), (path, initializer)) {
            return Err(format!(
                "fixtures/{path}: duplicate id {id:?}, first declared in fixtures/{previous}"
            ));
        }
        referenced.extend(references);
    }
    for path in &files {
        if (path.ends_with(".html") || path.ends_with(".expected.json"))
            && !referenced.contains(path)
        {
            return Err(format!("fixtures/{path}: orphan fixture input/expectation"));
        }
    }
    Ok(format!(
        "// Generated by build.rs; fixture metadata is the source of truth.\nconst GENERATED_FIXTURES: &[Fixture] = &[\n{}\n];\n",
        fixtures
            .values()
            .map(|(_, source)| format!("{source},"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}
