//! AG6 build input only. Execution consumes the generated existing Fixture model.
mod external;
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
    provenance: Option<external::Provenance>,
    upstream_assertion: Option<external::UpstreamAssertion>,
    relationship: Option<external::Relationship>,
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

fn observations(value: &ObservationMetadata, runnable: bool) -> Result<String, String> {
    if runnable
        && value.dom.is_none()
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

fn provenance(p: &external::Provenance) -> String {
    format!(
        "UpstreamSource {{ repository: {}, revision: {}, path: {}, sha256: {}, license: {}, license_sha256: {}, attribution: {} }}",
        literal(&p.repository),
        literal(&p.revision),
        literal(&p.path),
        literal(&p.sha256),
        literal(&p.license.path),
        literal(&p.license.sha256),
        literal(&p.attribution)
    )
}

#[allow(clippy::too_many_arguments)]
fn document_source(
    root: &Path,
    declaration: &str,
    input: &str,
    bytes: &[u8],
    p: Option<&external::Provenance>,
    facts: Option<&external::Facts>,
    files: &BTreeSet<String>,
    references: &mut Vec<String>,
) -> Result<String, String> {
    if let Some(p) = p {
        external::validate_provenance(p, bytes)?;
        if !input.ends_with(&format!("/raw/{}", p.path)) {
            return Err("upstream path must be mirrored beneath raw/".into());
        }
        let license = reference(declaration, &p.license.path, files)?;
        if external::sha256(&fs::read(root.join(&license)).map_err(|e| e.to_string())?)
            != p.license.sha256
        {
            return Err("licence hash mismatch".into());
        }
        references.push(license);
    }
    let links = serde_json::to_string(&facts.map(|f| f.links.as_slice()).unwrap_or(&[]))
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "DocumentInput {{ source: {}, html: {}, sha256: {}, provenance: {}, metadata_links: {} }}",
        literal(&format!("fixtures/{input}")),
        embedded(input, false),
        literal(&external::sha256(bytes)),
        p.map_or("None".into(), |p| format!("Some({})", provenance(p))),
        literal(&links)
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
            let runnable = matches!(metadata.classification, ClassificationMetadata::Runnable {});
            let request = observations(&metadata.observations, runnable)?;
            if metadata.provenance.is_some() != metadata.upstream_assertion.is_some() {
                return Err("provenance and upstream_assertion must be declared together".into());
            }
            let inspected = metadata.provenance.is_some() || metadata.relationship.is_some();
            let facts = if inspected {
                Some(external::inspect(&html)?)
            } else {
                None
            };
            let document = document_source(
                &root,
                path,
                &input,
                &html,
                metadata.provenance.as_ref(),
                facts.as_ref(),
                &files,
                &mut references,
            )?;
            if let Some(facts) = &facts
                && runnable
            {
                external::links::validate(&facts.links)?;
                if !facts.static_exclusions.is_empty() {
                    return Err(format!(
                        "runnable source requires unsupported behavior: {:?}",
                        facts.static_exclusions
                    ));
                }
            }
            let mut reference_document = None;
            let mut reference_provenance = "None".to_string();
            if let Some(rel) = &metadata.relationship {
                if !matches!(rel.kind.as_str(), "match" | "mismatch") {
                    return Err("relationship.kind must be match or mismatch".into());
                }
                let source_relations =
                    external::relationships(facts.as_ref().expect("relationship source parsed"))?;
                if source_relations.len() != 1 || source_relations[0].0 != rel.kind {
                    return Err("relationship contradicts active source relationship".into());
                }
                let href = &source_relations[0].1;
                external::links::reference_filename(href)?;
                let ref_path = reference(path, &rel.input, &files)?;
                let sibling = input
                    .rsplit_once('/')
                    .map_or_else(|| href.clone(), |(p, _)| format!("{p}/{href}"));
                if ref_path != sibling {
                    return Err("relationship target contradicts same-directory source href".into());
                }
                match (&metadata.provenance, &rel.provenance) {
                    (Some(p), Some(r)) => {
                        let expected = p
                            .path
                            .rsplit_once('/')
                            .map_or_else(|| href.clone(), |(parent, _)| format!("{parent}/{href}"));
                        if r.repository != p.repository
                            || r.revision != p.revision
                            || r.path != expected
                        {
                            return Err("relationship upstream target/pin contradiction".into());
                        }
                        reference_provenance = format!("Some({})", provenance(r));
                    }
                    (None, None) => {}
                    _ => {
                        return Err(
                            "test/reference provenance must both be upstream or both local".into(),
                        );
                    }
                }
                let bytes = fs::read(root.join(&ref_path)).map_err(|e| e.to_string())?;
                std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
                let ref_facts = external::inspect(&bytes)?;
                if runnable {
                    external::links::validate(&ref_facts.links)?;
                    if !external::relationships(&ref_facts)?.is_empty()
                        || !ref_facts.static_exclusions.is_empty()
                    {
                        return Err("reference chains/readiness/scripts are unsupported".into());
                    }
                    if metadata.observations.dom.is_some()
                        || metadata.observations.layout.is_some()
                        || metadata.observations.canvas.is_some()
                        || metadata.observations.styles.len() != 1
                        || metadata.observations.styles[0].property != "background-color"
                        || !matches!(
                            metadata.observations.styles[0].selection,
                            FamilyMetadata::Compare {}
                        )
                    {
                        return Err(
                            "relationship requires one present-target background-color observation"
                                .into(),
                        );
                    }
                }
                reference_document = Some(document_source(
                    &root,
                    path,
                    &ref_path,
                    &bytes,
                    rel.provenance.as_ref(),
                    Some(&ref_facts),
                    &files,
                    &mut references,
                )?);
                references.push(ref_path);
            } else if runnable
                && let Some(facts) = &facts
                && !external::relationships(facts)?.is_empty()
            {
                return Err("active relationship requires explicit relationship metadata".into());
            }
            let upstream = match &metadata.upstream_assertion {
                None => "None".into(),
                Some(u) => {
                    reason(&u.unsupported_reason)?;
                    if !matches!(u.form.as_str(), "visual_reftest" | "testharness") {
                        return Err("unsupported upstream assertion form declaration".into());
                    }
                    format!(
                        "Some(UpstreamAssertion {{ form: {}, unsupported_reason: {}, reference: {reference_provenance} }})",
                        literal(&u.form),
                        literal(&u.unsupported_reason)
                    )
                }
            };
            let kind = match &metadata.classification {
                ClassificationMetadata::Runnable {} => {
                    if metadata.requires_javascript {
                        return Err("requires_javascript: runnable fixtures must not require page JavaScript".into());
                    }
                    let independent = match (&metadata.expectation, &metadata.expected) {
                        (None, None) => None,
                        (Some(disposition), Some(expected)) => {
                            let disposition = match disposition {
                                ExpectationMetadata::Match {} => "Expectation::Match".into(),
                                ExpectationMetadata::KnownFailure { reason: text } => {
                                    reason(text)?;
                                    format!(
                                        "Expectation::KnownFailure {{ reason: {} }}",
                                        literal(text)
                                    )
                                }
                            };
                            let expected = match expected {
                                ExpectedMetadata::Canvas { rgb8 } => {
                                    format!("Expected::Canvas(CanvasColor({rgb8:?}))")
                                }
                                ExpectedMetadata::Projection {
                                    path: expected_path,
                                } => {
                                    let p = reference(path, expected_path, &files)?;
                                    if !p.ends_with(".expected.json") {
                                        return Err(
                                            "expected.path: expected an .expected.json file".into(),
                                        );
                                    }
                                    let bytes =
                                        fs::read(root.join(&p)).map_err(|e| e.to_string())?;
                                    std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
                                    references.push(p.clone());
                                    format!("Expected::Projection({})", embedded(&p, true))
                                }
                            };
                            Some(format!(
                                "IndependentExpectation {{ expected: {expected}, disposition: {disposition} }}"
                            ))
                        }
                        _ => {
                            return Err("expected and expectation must be declared together".into());
                        }
                    };
                    let checks = if let Some(reference) = reference_document {
                        let kind = if metadata.relationship.as_ref().unwrap().kind == "match" {
                            "Match"
                        } else {
                            "Mismatch"
                        };
                        format!(
                            "Checks::Reference {{ kind: RelationshipKind::{kind}, reference: {reference}, independent: {} }}",
                            independent.map_or("None".into(), |v| format!("Some({v})"))
                        )
                    } else {
                        format!(
                            "Checks::Independent({})",
                            independent.ok_or(
                                "expectation: required for runnable fixture without relationship"
                            )?
                        )
                    };
                    format!("FixtureKind::Runnable {{ checks: {checks} }}")
                }
                ClassificationMetadata::Unsupported { reason: text }
                | ClassificationMetadata::Skipped { reason: text } => {
                    if metadata.expectation.is_some() || metadata.expected.is_some() {
                        return Err("classification: excluded fixtures must not declare expectation or expected data".into());
                    }
                    reason(text)?;
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
                    "Fixture {{ id: TestId({}), document: {document}, upstream: {upstream}, metadata_source: {}, requires_javascript: {}, request: {request}, kind: {kind} }}",
                    literal(&metadata.id),
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
