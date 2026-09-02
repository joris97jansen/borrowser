use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

const FORBIDDEN_ENGINE_PACKAGES: &[&str] = &[
    "css",
    "layout",
    "gfx",
    "paint",
    "browser",
    "runtime",
    "runtime_net",
    "runtime_parse",
    "runtime_css",
    "js",
];

const FORBIDDEN_DIRECT_RUNNER_PACKAGES: &[&str] = &[
    "html",
    "css",
    "layout",
    "gfx",
    "paint",
    "browser",
    "runtime",
    "runtime_net",
    "runtime_parse",
    "runtime_css",
    "js",
];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DependencyKind {
    Normal,
    Build,
    Dev,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DeclaredDependency {
    package: String,
    alias: String,
    kind: DependencyKind,
    target: Option<String>,
}

fn cargo_metadata(manifest_path: &Path, locked: bool) -> serde_json::Value {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.args([
        "metadata",
        "--format-version",
        "1",
        "--offline",
        "--no-deps",
        "--manifest-path",
    ]);
    command.arg(manifest_path);
    if locked {
        command.arg("--locked");
    }
    let output = command.output().expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid cargo metadata JSON")
}

fn package<'a>(metadata: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    metadata["packages"]
        .as_array()
        .expect("metadata packages")
        .iter()
        .find(|package| package["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("metadata contains package {name}"))
}

fn package_dependency_declarations(
    metadata: &serde_json::Value,
    name: &str,
) -> BTreeSet<DeclaredDependency> {
    package(metadata, name)["dependencies"]
        .as_array()
        .expect("package dependencies")
        .iter()
        .map(|dependency| {
            let kind = match dependency["kind"].as_str() {
                None => DependencyKind::Normal,
                Some("build") => DependencyKind::Build,
                Some("dev") => DependencyKind::Dev,
                Some(other) => panic!("unknown Cargo dependency kind {other}"),
            };
            DeclaredDependency {
                package: dependency["name"]
                    .as_str()
                    .expect("resolved dependency package identity")
                    .to_owned(),
                alias: dependency_alias(dependency).to_owned(),
                kind,
                target: dependency["target"].as_str().map(str::to_owned),
            }
        })
        .collect()
}

fn package_dependencies(metadata: &serde_json::Value, name: &str) -> BTreeSet<String> {
    package_dependency_declarations(metadata, name)
        .into_iter()
        .map(|dependency| dependency.package)
        .collect()
}

fn forbidden_dependencies(packages: &BTreeSet<String>, forbidden: &[&str]) -> BTreeSet<String> {
    forbidden
        .iter()
        .copied()
        .filter(|package| packages.contains(*package))
        .map(str::to_owned)
        .collect()
}

fn assert_absent(packages: &BTreeSet<String>, forbidden: &[&str], owner: &str) {
    let present = forbidden_dependencies(packages, forbidden);
    assert!(
        present.is_empty(),
        "{owner} must not depend on forbidden package identities: {present:?}"
    );
}

fn feature_values<'a>(package: &'a serde_json::Value, feature: &str) -> &'a [serde_json::Value] {
    package["features"][feature]
        .as_array()
        .unwrap_or_else(|| panic!("package declares feature {feature}"))
}

fn dependency_alias(dependency: &serde_json::Value) -> &str {
    dependency["rename"]
        .as_str()
        .or_else(|| dependency["name"].as_str())
        .expect("dependency alias")
}

fn activate_feature_closure(
    package: &serde_json::Value,
    enabled: &mut BTreeSet<String>,
) -> BTreeSet<String> {
    let mut activated_dependencies = BTreeSet::new();
    loop {
        let before_features = enabled.len();
        let before_dependencies = activated_dependencies.len();
        for feature in enabled.clone() {
            let Some(values) = package["features"]
                .get(&feature)
                .and_then(|value| value.as_array())
            else {
                continue;
            };
            for value in values {
                let value = value.as_str().expect("feature value");
                if let Some(alias) = value.strip_prefix("dep:") {
                    activated_dependencies.insert(alias.to_owned());
                } else if let Some((alias, _forwarded)) = value.split_once('/') {
                    let weak = alias.strip_suffix('?');
                    if weak.is_none() || activated_dependencies.contains(weak.unwrap()) {
                        activated_dependencies.insert(weak.unwrap_or(alias).to_owned());
                    }
                } else {
                    enabled.insert(value.to_owned());
                }
            }
        }
        if before_features == enabled.len() && before_dependencies == activated_dependencies.len() {
            return activated_dependencies;
        }
    }
}

fn forwarded_features(
    package: &serde_json::Value,
    enabled: &BTreeSet<String>,
    dependency_alias: &str,
) -> BTreeSet<String> {
    enabled
        .iter()
        .flat_map(|feature| {
            package["features"]
                .get(feature)
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(serde_json::Value::as_str)
        .filter_map(|value| value.split_once('/'))
        .filter_map(|(alias, feature)| {
            (alias.trim_end_matches('?') == dependency_alias).then_some(feature.to_owned())
        })
        .collect()
}

fn workspace_graph(
    metadata: &serde_json::Value,
    root: &str,
    root_features: &[&str],
) -> BTreeSet<String> {
    let workspace_packages = metadata["packages"]
        .as_array()
        .expect("metadata packages")
        .iter()
        .map(|package| {
            (
                package["name"].as_str().expect("package name").to_owned(),
                package,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut graph = BTreeSet::from([root.to_owned()]);
    let mut enabled = BTreeMap::from([(
        root.to_owned(),
        root_features
            .iter()
            .map(|value| (*value).to_owned())
            .collect(),
    )]);

    loop {
        let mut changed = false;
        for package_name in graph.clone() {
            let package = workspace_packages[&package_name];
            let (activated_dependencies, package_features) = {
                let package_features = enabled.entry(package_name.clone()).or_default();
                let activated_dependencies = activate_feature_closure(package, package_features);
                (activated_dependencies, package_features.clone())
            };
            for dependency in package["dependencies"]
                .as_array()
                .expect("package dependencies")
            {
                if dependency["kind"].as_str() == Some("dev") {
                    continue;
                }
                let alias = dependency_alias(dependency);
                let optional = dependency["optional"].as_bool().expect("optional flag");
                if optional && !activated_dependencies.contains(alias) {
                    continue;
                }
                let dependency_name = dependency["name"]
                    .as_str()
                    .expect("dependency package identity");
                if !workspace_packages.contains_key(dependency_name) {
                    continue;
                }
                changed |= graph.insert(dependency_name.to_owned());
                let dependency_features = enabled.entry(dependency_name.to_owned()).or_default();
                if dependency["uses_default_features"]
                    .as_bool()
                    .expect("default-features flag")
                {
                    changed |= dependency_features.insert("default".to_owned());
                }
                for feature in dependency["features"]
                    .as_array()
                    .expect("dependency features")
                {
                    changed |= dependency_features
                        .insert(feature.as_str().expect("dependency feature").to_owned());
                }
                for feature in forwarded_features(package, &package_features, alias) {
                    changed |= dependency_features.insert(feature);
                }
            }
        }
        if !changed {
            return graph;
        }
    }
}

fn synthetic_workspace_metadata(
    workspace_dependencies: &str,
    runner_dependencies: &str,
    package_names: &[&str],
) -> serde_json::Value {
    let workspace = tempfile::tempdir().unwrap();
    fs::create_dir_all(workspace.path().join("runner/src")).unwrap();
    let mut members = vec!["\"runner\"".to_owned()];
    for package_name in package_names {
        members.push(format!("\"{package_name}\""));
        fs::create_dir_all(workspace.path().join(package_name).join("src")).unwrap();
        fs::write(
            workspace.path().join(package_name).join("Cargo.toml"),
            format!(
                "[package]\nname = \"{package_name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n"
            ),
        )
        .unwrap();
        fs::write(workspace.path().join(package_name).join("src/lib.rs"), "").unwrap();
    }
    fs::write(
        workspace.path().join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [{}]\nresolver = \"2\"\n{workspace_dependencies}",
            members.join(", ")
        ),
    )
    .unwrap();
    fs::write(
        workspace.path().join("runner/Cargo.toml"),
        format!(
            "[package]\nname = \"synthetic-runner\"\nversion = \"0.0.0\"\nedition = \"2024\"\n{runner_dependencies}"
        ),
    )
    .unwrap();
    fs::write(workspace.path().join("runner/src/lib.rs"), "").unwrap();
    cargo_metadata(&workspace.path().join("Cargo.toml"), false)
}

#[test]
fn parser_execution_dependency_graph_preserves_ownership_boundaries() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let metadata = cargo_metadata(&workspace_root.join("Cargo.toml"), true);
    let runner = package(&metadata, "conformance-runner");
    assert!(
        feature_values(runner, "default").is_empty(),
        "subsystem-neutral orchestration has no adapter enabled by default"
    );
    assert_eq!(
        feature_values(runner, "html-parser"),
        &[serde_json::Value::String(
            "dep:html-test-support".to_owned()
        )]
    );
    let html_dependency = runner["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|dependency| dependency["name"].as_str() == Some("html-test-support"))
        .expect("runner declares the HTML adapter dependency");
    assert_eq!(html_dependency["optional"].as_bool(), Some(true));

    let runner_dependencies = package_dependencies(&metadata, "conformance-runner");
    assert!(runner_dependencies.contains("conformance-test-support"));
    assert!(runner_dependencies.contains("html-test-support"));
    assert_absent(
        &runner_dependencies,
        FORBIDDEN_DIRECT_RUNNER_PACKAGES,
        "direct conformance-runner declarations across every dependency kind and target",
    );

    let default_graph = workspace_graph(&metadata, "conformance-runner", &["default"]);
    assert!(default_graph.contains("conformance-test-support"));
    assert!(!default_graph.contains("html-test-support"));
    assert!(!default_graph.contains("html"));
    assert_absent(
        &default_graph,
        FORBIDDEN_ENGINE_PACKAGES,
        "default conformance-runner graph",
    );

    let html_graph = workspace_graph(&metadata, "conformance-runner", &["default", "html-parser"]);
    assert!(html_graph.contains("conformance-test-support"));
    assert!(html_graph.contains("html-test-support"));
    assert!(html_graph.contains("html"));
    assert_absent(
        &html_graph,
        FORBIDDEN_ENGINE_PACKAGES,
        "html-parser conformance-runner graph",
    );

    let generic = package_dependencies(&metadata, "conformance-test-support");
    assert_absent(
        &generic,
        &[
            "html-test-support",
            "html",
            "css",
            "layout",
            "gfx",
            "paint",
            "browser",
            "runtime",
            "runtime_net",
            "runtime_parse",
            "runtime_css",
            "js",
            "conformance-runner",
        ],
        "conformance-test-support",
    );

    let html_support = package_dependencies(&metadata, "html-test-support");
    assert!(
        html_support.contains("html"),
        "the adapter directly owns the production HTML dependency"
    );
    assert_absent(
        &html_support,
        &["conformance-test-support", "conformance-runner"],
        "html-test-support",
    );

    let production_html = package_dependencies(&metadata, "html");
    assert_absent(
        &production_html,
        &["conformance-test-support", "conformance-runner"],
        "html",
    );
}

#[test]
fn css_execution_dependency_graph_is_explicit_and_stops_before_rendering() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let metadata = cargo_metadata(&workspace_root.join("Cargo.toml"), true);
    let runner = package(&metadata, "conformance-runner");
    assert_eq!(
        feature_values(runner, "css"),
        &[serde_json::Value::String("dep:css-test-support".to_owned())]
    );
    let runner_dependencies = package_dependencies(&metadata, "conformance-runner");
    assert_absent(
        &runner_dependencies,
        FORBIDDEN_DIRECT_RUNNER_PACKAGES,
        "direct conformance-runner declarations",
    );

    let graph = workspace_graph(&metadata, "conformance-runner", &["default", "css"]);
    for required in [
        "conformance-test-support",
        "css-test-support",
        "css",
        "html",
    ] {
        assert!(
            graph.contains(required),
            "CSS adapter graph misses {required}"
        );
    }
    assert_absent(
        &graph,
        &[
            "layout",
            "gfx",
            "paint",
            "browser",
            "runtime",
            "runtime_net",
            "runtime_parse",
            "runtime_css",
            "js",
            "net",
            "platform",
            "app_api",
        ],
        "CSS conformance graph",
    );

    let support = package_dependencies(&metadata, "css-test-support");
    for required in ["css", "html"] {
        assert!(
            support.contains(required),
            "css-test-support misses {required}"
        );
    }
    assert_absent(
        &support,
        &[
            "layout",
            "gfx",
            "paint",
            "browser",
            "runtime",
            "runtime_net",
            "runtime_parse",
            "runtime_css",
            "js",
            "net",
            "platform",
            "app_api",
            "conformance-test-support",
            "conformance-runner",
        ],
        "css-test-support",
    );
}

#[test]
fn rendering_execution_dependency_graph_stops_before_browser_runtime_and_backends() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let metadata = cargo_metadata(&workspace_root.join("Cargo.toml"), true);
    let runner = package(&metadata, "conformance-runner");
    assert_eq!(
        feature_values(runner, "rendering"),
        &[serde_json::Value::String(
            "dep:rendering-test-support".to_owned()
        )]
    );
    for (label, features) in [
        ("featureless", Vec::new()),
        ("parser-only", vec!["html-parser"]),
        ("CSS-only", vec!["css"]),
        ("parser-and-CSS", vec!["html-parser", "css"]),
    ] {
        let graph = workspace_graph(&metadata, "conformance-runner", &features);
        assert!(
            !graph.contains("rendering-test-support"),
            "{label} conformance-runner graph must not activate rendering-test-support"
        );
    }
    let graph = workspace_graph(&metadata, "conformance-runner", &["default", "rendering"]);
    for required in [
        "conformance-test-support",
        "rendering-test-support",
        "wpt-test-support",
        "external-test-provenance",
        "html",
        "css",
        "layout",
        "gfx",
    ] {
        assert!(
            graph.contains(required),
            "rendering graph misses {required}"
        );
    }
    assert_absent(
        &graph,
        &[
            "browser",
            "runtime",
            "runtime_net",
            "runtime_parse",
            "runtime_css",
            "js",
            "net",
            "platform",
            "app_api",
        ],
        "rendering conformance graph",
    );
    let support = package_dependencies(&metadata, "rendering-test-support");
    for required in [
        "html",
        "css",
        "layout",
        "gfx",
        "conformance-test-support",
        "wpt-test-support",
    ] {
        assert!(
            support.contains(required),
            "rendering-test-support misses {required}"
        );
    }
    assert_absent(
        &support,
        &[
            "browser",
            "runtime",
            "runtime_net",
            "runtime_parse",
            "runtime_css",
            "js",
            "net",
            "platform",
            "app_api",
            "conformance-runner",
        ],
        "rendering-test-support",
    );
    let html_dependency = package(&metadata, "rendering-test-support")["dependencies"]
        .as_array()
        .expect("rendering support dependencies")
        .iter()
        .find(|dependency| dependency["name"].as_str() == Some("html"))
        .expect("rendering support HTML dependency");
    let html_features: BTreeSet<_> = html_dependency["features"]
        .as_array()
        .expect("HTML dependency features")
        .iter()
        .map(|value| value.as_str().expect("feature name"))
        .collect();
    assert_eq!(html_features, BTreeSet::from(["html5"]));
}

#[test]
fn ag8_external_source_dependency_dag_is_one_way_and_test_tooling_only() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let metadata = cargo_metadata(&workspace_root.join("Cargo.toml"), true);

    let provenance = package_dependencies(&metadata, "external-test-provenance");
    for repository_package in [
        "html",
        "css",
        "layout",
        "gfx",
        "conformance-test-support",
        "wpt-test-support",
        "html-test-support",
    ] {
        assert!(!provenance.contains(repository_package));
    }

    let generic = package_dependencies(&metadata, "conformance-test-support");
    assert!(generic.contains("external-test-provenance"));
    assert!(!generic.contains("wpt-test-support"));

    let wpt = package_dependencies(&metadata, "wpt-test-support");
    for required in [
        "external-test-provenance",
        "conformance-test-support",
        "html5ever",
    ] {
        assert!(wpt.contains(required), "WPT support misses {required}");
    }

    let html_support = package_dependencies(&metadata, "html-test-support");
    assert!(html_support.contains("external-test-provenance"));
    assert!(!html_support.contains("conformance-test-support"));
    assert!(!html_support.contains("wpt-test-support"));

    let runner = package_dependencies(&metadata, "conformance-runner");
    assert!(!runner.contains("wpt-test-support"));

    for production in [
        "html",
        "css",
        "layout",
        "gfx",
        "browser",
        "runtime_net",
        "runtime_parse",
        "runtime_css",
        "js",
        "net",
        "platform",
        "app_api",
        "borrowser",
    ] {
        let dependencies = package_dependencies(&metadata, production);
        for tooling in [
            "external-test-provenance",
            "conformance-test-support",
            "wpt-test-support",
            "rendering-test-support",
        ] {
            assert!(
                !dependencies.contains(tooling),
                "production package {production} must not depend on {tooling}"
            );
        }
    }
}

#[test]
fn direct_aliased_html_is_a_forbidden_runner_declaration() {
    let metadata = synthetic_workspace_metadata(
        "",
        "\n[dependencies]\ndirect_html_alias = { package = \"html\", path = \"../html\" }\n",
        &["html"],
    );
    let dependencies = package_dependencies(&metadata, "synthetic-runner");
    assert_eq!(
        forbidden_dependencies(&dependencies, FORBIDDEN_DIRECT_RUNNER_PACKAGES),
        BTreeSet::from(["html".to_owned()])
    );
    assert!(
        package_dependency_declarations(&metadata, "synthetic-runner").contains(
            &DeclaredDependency {
                package: "html".to_owned(),
                alias: "direct_html_alias".to_owned(),
                kind: DependencyKind::Normal,
                target: None,
            }
        )
    );
}

#[test]
fn workspace_inherited_html_alias_is_a_forbidden_runner_declaration() {
    let metadata = synthetic_workspace_metadata(
        "\n[workspace.dependencies]\ninherited_html_alias = { package = \"html\", path = \"html\" }\n",
        "\n[dependencies]\ninherited_html_alias = { workspace = true }\n",
        &["html"],
    );
    let dependencies = package_dependencies(&metadata, "synthetic-runner");
    assert_eq!(
        forbidden_dependencies(&dependencies, FORBIDDEN_DIRECT_RUNNER_PACKAGES),
        BTreeSet::from(["html".to_owned()])
    );
    assert!(
        package_dependency_declarations(&metadata, "synthetic-runner").contains(
            &DeclaredDependency {
                package: "html".to_owned(),
                alias: "inherited_html_alias".to_owned(),
                kind: DependencyKind::Normal,
                target: None,
            }
        )
    );
}

#[test]
fn forbidden_dev_build_and_target_dependencies_are_all_visible_to_the_guard() {
    let metadata = synthetic_workspace_metadata(
        "\n[workspace.dependencies]\ndev_browser_alias = { package = \"browser\", path = \"browser\" }\n",
        "\n[dev-dependencies]\ndev_browser_alias = { workspace = true }\n\n[build-dependencies]\nbuild_css_alias = { package = \"css\", path = \"../css\" }\n\n[target.'cfg(unix)'.dependencies]\ntarget_layout_alias = { package = \"layout\", path = \"../layout\" }\n",
        &["browser", "css", "layout"],
    );
    let declarations = package_dependency_declarations(&metadata, "synthetic-runner");
    assert!(declarations.contains(&DeclaredDependency {
        package: "browser".to_owned(),
        alias: "dev_browser_alias".to_owned(),
        kind: DependencyKind::Dev,
        target: None,
    }));
    assert!(declarations.contains(&DeclaredDependency {
        package: "css".to_owned(),
        alias: "build_css_alias".to_owned(),
        kind: DependencyKind::Build,
        target: None,
    }));
    assert!(declarations.contains(&DeclaredDependency {
        package: "layout".to_owned(),
        alias: "target_layout_alias".to_owned(),
        kind: DependencyKind::Normal,
        target: Some("cfg(unix)".to_owned()),
    }));
    assert_eq!(
        forbidden_dependencies(
            &package_dependencies(&metadata, "synthetic-runner"),
            FORBIDDEN_DIRECT_RUNNER_PACKAGES,
        ),
        BTreeSet::from(["browser".to_owned(), "css".to_owned(), "layout".to_owned(),])
    );
}
