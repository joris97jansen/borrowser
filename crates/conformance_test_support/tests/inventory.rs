mod support;

use std::fs;

use conformance_test_support::{
    InventoryDiagnosticKind, InventoryRepository, MAX_DESCRIPTOR_BYTES, ObservationSurface,
    PortablePathComponent, discover_inventory, generate_manifest_bytes,
};
use support::{TestRepository, descriptor, descriptor_v2, descriptor_v3};

#[test]
fn ag2_owned_parsers_are_available_without_reimplementing_their_vocabularies() {
    assert!(PortablePathComponent::is_valid(
        "capture.web-observable-dom-tree-v1.txt"
    ));
    assert!(!PortablePathComponent::is_valid("../capture.txt"));
    let component = PortablePathComponent::parse("capture.web-observable-dom-tree-v1.txt")
        .expect("portable AG2 component");
    assert_eq!(component.as_str(), "capture.web-observable-dom-tree-v1.txt");
    assert!(PortablePathComponent::parse("../capture.txt").is_none());

    for surface in [
        ObservationSurface::HtmlTokenizer,
        ObservationSurface::HtmlTreeConstruction,
        ObservationSurface::DomTree,
        ObservationSurface::CssParsing,
        ObservationSurface::CssSelectors,
        ObservationSurface::CssCascade,
        ObservationSurface::ComputedStyle,
        ObservationSurface::LayoutGeometry,
        ObservationSurface::PaintOperations,
        ObservationSurface::BrowserRuntimeSemantic,
    ] {
        assert_eq!(ObservationSurface::parse(surface.as_str()), Some(surface));
    }
}

#[test]
fn v3_reference_relation_and_package_containment_are_strict() {
    let valid = TestRepository::new();
    valid.bundle(
        "paired",
        &descriptor_v3(
            "paired-reference",
            "paint-operations",
            "rendering/test.html",
            ("semantic", "mismatch", "rendering/reference.html"),
            "rendering/fixture.toml",
            &["rendering/test.css", "rendering/reference.css"],
        ),
        &[
            ("rendering/test.html", b"test"),
            ("rendering/reference.html", b"reference"),
            ("rendering/fixture.toml", b"nested"),
            ("rendering/test.css", b"test css"),
            ("rendering/reference.css", b"reference css"),
        ],
    );
    let inventory = discover_inventory(&valid.repository()).expect("valid V3 inventory");
    assert_eq!(
        inventory.fixtures()[0]
            .reference()
            .expect("reference")
            .relation(),
        conformance_test_support::ReferenceRelation::Mismatch
    );

    for (name, test_path, reference_path) in [
        ("test-outside", "test.html", "rendering/reference.html"),
        ("reference-outside", "rendering/test.html", "reference.html"),
    ] {
        let repository = TestRepository::new();
        repository.bundle(
            name,
            &descriptor_v3(
                name,
                "paint-operations",
                test_path,
                ("semantic", "match", reference_path),
                "rendering/fixture.toml",
                &[],
            ),
            &[
                (test_path, b"test"),
                (reference_path, b"reference"),
                ("rendering/fixture.toml", b"nested"),
            ],
        );
        assert_kind(&repository, |kind| {
            matches!(
                kind,
                InventoryDiagnosticKind::ExecutionFileOutsidePackage { .. }
            )
        });
    }
}

#[test]
fn v3_test_and_reference_roles_cannot_be_support_paths() {
    for duplicate in ["rendering/test.html", "rendering/reference.html"] {
        let repository = TestRepository::new();
        repository.bundle(
            "duplicate-role",
            &descriptor_v3(
                "duplicate-role",
                "paint-operations",
                "rendering/test.html",
                ("semantic", "match", "rendering/reference.html"),
                "rendering/fixture.toml",
                &[duplicate],
            ),
            &[
                ("rendering/test.html", b"test"),
                ("rendering/reference.html", b"reference"),
                ("rendering/fixture.toml", b"nested"),
            ],
        );
        assert_kind(&repository, |kind| {
            matches!(kind, InventoryDiagnosticKind::DuplicateDeclaredPath { .. })
        });
    }
}

#[test]
fn filesystem_creation_order_does_not_change_discovery_or_diagnostics() {
    let first = TestRepository::new();
    first.bundle(
        "group/zeta",
        &descriptor("zeta-case", "dom-tree", "test.html"),
        &[("test.html", b"zeta")],
    );
    first.bundle(
        "group/alpha",
        &descriptor("alpha-case", "html-tokenizer", "test.html"),
        &[("test.html", b"alpha")],
    );

    let second = TestRepository::new();
    second.bundle(
        "group/alpha",
        &descriptor("alpha-case", "html-tokenizer", "test.html"),
        &[("test.html", b"alpha")],
    );
    second.bundle(
        "group/zeta",
        &descriptor("zeta-case", "dom-tree", "test.html"),
        &[("test.html", b"zeta")],
    );

    let first_inventory = discover_inventory(&first.repository()).expect("first inventory");
    let second_inventory = discover_inventory(&second.repository()).expect("second inventory");
    let first_paths = first_inventory
        .fixtures()
        .iter()
        .map(|fixture| fixture.fixture_path().as_str())
        .collect::<Vec<_>>();
    let second_paths = second_inventory
        .fixtures()
        .iter()
        .map(|fixture| fixture.fixture_path().as_str())
        .collect::<Vec<_>>();
    assert_eq!(first_paths, second_paths);
    assert_eq!(
        generate_manifest_bytes(&first.repository()).expect("first manifest"),
        generate_manifest_bytes(&second.repository()).expect("second manifest")
    );
}

#[test]
fn logical_id_is_stable_across_bundle_reorganization() {
    let repository = TestRepository::new();
    repository.bundle(
        "old/location",
        &descriptor("stable-logical-id", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let before = discover_inventory(&repository.repository()).expect("inventory before move");
    fs::create_dir_all(repository.fixture_root().join("new")).expect("new group");
    fs::rename(
        repository.fixture_root().join("old/location"),
        repository.fixture_root().join("new/location"),
    )
    .expect("move fixture bundle");
    let after = discover_inventory(&repository.repository()).expect("inventory after move");
    assert_eq!(before.fixtures()[0].id(), after.fixtures()[0].id());
    assert_ne!(
        before.fixtures()[0].fixture_path(),
        after.fixtures()[0].fixture_path()
    );
}

#[test]
fn exact_and_case_colliding_ids_are_diagnosed() {
    let exact = TestRepository::new();
    exact.bundle(
        "one",
        &descriptor("duplicate-id", "dom-tree", "test.html"),
        &[("test.html", b"one")],
    );
    exact.bundle(
        "two",
        &descriptor("duplicate-id", "dom-tree", "test.html"),
        &[("test.html", b"two")],
    );
    let errors = discover_inventory(&exact.repository()).expect_err("duplicate id");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::DuplicateTestId { .. }
    )));

    let case = TestRepository::new();
    case.bundle(
        "one",
        &descriptor("case-id", "dom-tree", "test.html"),
        &[("test.html", b"one")],
    );
    case.bundle(
        "two",
        &descriptor("Case-Id", "dom-tree", "test.html"),
        &[("test.html", b"two")],
    );
    let errors = discover_inventory(&case.repository()).expect_err("case collision");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::CaseUnsafeTestId { .. }
    )));
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::CaseCollidingTestId { .. }
    )));
}

#[test]
fn overlong_and_invalid_grammar_test_ids_are_distinct() {
    let overlong = TestRepository::new();
    overlong.bundle(
        "overlong",
        &descriptor(&"a".repeat(129), "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    assert_kind(&overlong, |kind| {
        matches!(kind, InventoryDiagnosticKind::TestIdTooLong { .. })
    });

    let invalid_grammar = TestRepository::new();
    invalid_grammar.bundle(
        "invalid-grammar",
        &descriptor("bad--id", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let errors = discover_inventory(&invalid_grammar.repository()).expect_err("invalid test id");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::InvalidTestId { .. }
    )));
    assert!(!errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::CaseUnsafeTestId { .. }
    )));
}

#[test]
fn descriptor_size_limit_accepts_exact_boundary_and_rejects_sentinel_byte() {
    let exact = TestRepository::new();
    exact.bundle(
        "exact-limit",
        &descriptor_padded_to("descriptor-exact-limit", MAX_DESCRIPTOR_BYTES as usize),
        &[("test.html", b"input")],
    );
    discover_inventory(&exact.repository()).expect("descriptor at exact size limit");

    let oversized = TestRepository::new();
    oversized.bundle(
        "oversized",
        &descriptor_padded_to("descriptor-over-limit", MAX_DESCRIPTOR_BYTES as usize + 1),
        &[("test.html", b"input")],
    );
    let errors = discover_inventory(&oversized.repository()).expect_err("oversized descriptor");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::DescriptorTooLarge { .. }
    )));
    assert!(
        !errors
            .diagnostics()
            .iter()
            .any(|diagnostic| matches!(diagnostic.kind, InventoryDiagnosticKind::MalformedToml))
    );
}

#[test]
fn deeply_nested_organizational_directories_are_discovered_iteratively() {
    let repository = TestRepository::new();
    let mut components = (0..128)
        .map(|index| format!("d{index:03}"))
        .collect::<Vec<_>>();
    components.push("bundle".to_owned());
    repository.bundle(
        &components.join("/"),
        &descriptor("deeply-nested-fixture", "dom-tree", "test.html"),
        &[("test.html", b"input")],
    );
    let inventory = discover_inventory(&repository.repository()).expect("deep inventory");
    assert_eq!(
        inventory.fixtures()[0].id().as_str(),
        "deeply-nested-fixture"
    );
}

#[test]
fn malformed_unknown_version_and_unknown_fields_are_typed() {
    let malformed = TestRepository::new();
    malformed.bundle("bad", "not = [valid", &[("test.html", b"input")]);
    assert_kind(&malformed, |kind| {
        matches!(kind, InventoryDiagnosticKind::MalformedToml)
    });

    let missing_metadata = TestRepository::new();
    missing_metadata.bundle(
        "bad",
        &descriptor("missing-metadata", "dom-tree", "test.html").replace(
            "\n[metadata]\ndescription = \"Temporary inventory fixture.\"\n",
            "\n",
        ),
        &[("test.html", b"input")],
    );
    assert_kind(&missing_metadata, |kind| {
        matches!(kind, InventoryDiagnosticKind::InvalidDescriptorShape)
    });

    let version = TestRepository::new();
    version.bundle(
        "bad",
        &descriptor("version-case", "dom-tree", "test.html").replace(
            "borrowser-conformance-fixture-v1",
            "borrowser-conformance-fixture-v9",
        ),
        &[("test.html", b"input")],
    );
    assert_kind(&version, |kind| {
        matches!(
            kind,
            InventoryDiagnosticKind::UnsupportedDescriptorVersion { .. }
        )
    });

    let unknown = TestRepository::new();
    unknown.bundle(
        "bad",
        &descriptor("unknown-field", "dom-tree", "test.html").replace(
            "kind = \"native\"",
            "kind = \"native\"\nsource_form = \"invented\"",
        ),
        &[("test.html", b"input")],
    );
    let errors = discover_inventory(&unknown.repository()).expect_err("unknown field");
    assert!(errors.diagnostics().iter().any(|diagnostic| {
        matches!(
            &diagnostic.kind,
            InventoryDiagnosticKind::UnknownDescriptorField { field }
                if field == "source.source_form"
        )
    }));
}

#[test]
fn fixture_root_outside_repository_is_rejected() {
    let repository = tempfile::tempdir().expect("repository root");
    let fixture_root = tempfile::tempdir().expect("outside fixture root");
    let errors = discover_inventory(&InventoryRepository::new(
        repository.path(),
        fixture_root.path(),
    ))
    .expect_err("outside fixture root");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::FixtureRootOutsideRepository
    )));
}

#[test]
fn invalid_scope_observation_source_and_reference_are_typed() {
    for (replacement, predicate) in [
        (("static-html-css-no-js", "runnable"), 0_u8),
        (("dom-tree", "path-derived-category"), 1_u8),
        (("kind = \"native\"", "kind = \"wpt\""), 2_u8),
    ] {
        let repository = TestRepository::new();
        repository.bundle(
            "bad",
            &descriptor("typed-error", "dom-tree", "test.html")
                .replace(replacement.0, replacement.1),
            &[("test.html", b"input")],
        );
        let errors = discover_inventory(&repository.repository()).expect_err("typed error");
        assert!(
            errors
                .diagnostics()
                .iter()
                .any(|diagnostic| match predicate {
                    0 => matches!(
                        diagnostic.kind,
                        InventoryDiagnosticKind::InvalidScope { .. }
                    ),
                    1 => matches!(
                        diagnostic.kind,
                        InventoryDiagnosticKind::InvalidObservation { .. }
                    ),
                    2 => matches!(
                        diagnostic.kind,
                        InventoryDiagnosticKind::InvalidSourceKind { .. }
                    ),
                    _ => unreachable!(),
                })
        );
    }

    let reference = TestRepository::new();
    let descriptor = support::descriptor_with_reference(
        "bad-reference",
        "paint-operations",
        "test.html",
        "pixel",
        "reference.html",
    );
    reference.bundle(
        "bad",
        &descriptor,
        &[("test.html", b"test"), ("reference.html", b"reference")],
    );
    assert_kind(&reference, |kind| {
        matches!(kind, InventoryDiagnosticKind::InvalidReferenceKind { .. })
    });
}

#[test]
fn unsafe_missing_non_regular_and_undeclared_paths_are_rejected() {
    for unsafe_path in [
        "../outside",
        "nested/../outside",
        "/absolute",
        "C:/absolute",
        "nested//test.html",
        "fixture.toml",
        "Uppercase/test.html",
        "nested/trailing.",
        "nested/trailing ",
        "nested/con.txt",
        "nested/unicode-é",
    ] {
        let repository = TestRepository::new();
        repository.bundle(
            "bad",
            &descriptor("unsafe-path", "dom-tree", unsafe_path),
            &[],
        );
        assert_kind(&repository, |kind| {
            matches!(kind, InventoryDiagnosticKind::InvalidRelativePath { .. })
        });
    }

    let backslash = TestRepository::new();
    backslash.bundle(
        "bad",
        &descriptor("unsafe-path", "dom-tree", "test.html")
            .replace("test_path = \"test.html\"", "test_path = 'C:\\absolute'"),
        &[],
    );
    assert_kind(&backslash, |kind| {
        matches!(kind, InventoryDiagnosticKind::InvalidRelativePath { .. })
    });

    let control = TestRepository::new();
    control.bundle(
        "bad",
        &descriptor("unsafe-path", "dom-tree", "test.html").replace(
            "test_path = \"test.html\"",
            "test_path = \"control-\\tname\"",
        ),
        &[],
    );
    assert_kind(&control, |kind| {
        matches!(kind, InventoryDiagnosticKind::InvalidRelativePath { .. })
    });

    let missing = TestRepository::new();
    missing.bundle(
        "bad",
        &descriptor("missing-path", "dom-tree", "missing.html"),
        &[],
    );
    assert_kind(&missing, |kind| {
        matches!(kind, InventoryDiagnosticKind::MissingDeclaredFile { .. })
    });

    let directory = TestRepository::new();
    directory.bundle(
        "bad",
        &descriptor("directory-path", "dom-tree", "payload"),
        &[],
    );
    fs::create_dir(directory.fixture_root().join("bad/payload")).expect("payload directory");
    assert_kind(&directory, |kind| {
        matches!(
            kind,
            InventoryDiagnosticKind::DeclaredPathNotRegularFile { .. }
        )
    });

    let undeclared = TestRepository::new();
    undeclared.bundle(
        "bad",
        &descriptor("undeclared-path", "dom-tree", "test.html"),
        &[("test.html", b"input"), ("asset.css", b"undeclared")],
    );
    assert_kind(&undeclared, |kind| {
        matches!(kind, InventoryDiagnosticKind::UndeclaredBundleFile)
    });
}

#[test]
fn files_outside_bundles_and_nested_bundles_are_rejected() {
    let missing = TestRepository::new();
    let undescribed = missing.fixture_root().join("group/undescribed");
    fs::create_dir_all(&undescribed).expect("undescribed directory");
    fs::write(undescribed.join("test.html"), b"input").expect("undescribed input");
    assert_kind(&missing, |kind| {
        matches!(kind, InventoryDiagnosticKind::MissingFixtureDescriptor)
    });

    let nested = TestRepository::new();
    nested.bundle(
        "outer",
        &descriptor("outer-case", "dom-tree", "test.html"),
        &[("test.html", b"outer")],
    );
    let nested_root = nested.fixture_root().join("outer/nested");
    fs::create_dir(&nested_root).expect("nested root");
    fs::write(
        nested_root.join("fixture.toml"),
        descriptor("nested-case", "dom-tree", "test.html"),
    )
    .expect("nested descriptor");
    fs::write(nested_root.join("test.html"), b"nested").expect("nested input");
    let errors = discover_inventory(&nested.repository()).expect_err("nested fixture");
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.kind,
        InventoryDiagnosticKind::NestedFixtureBundle
    )));
}

#[test]
fn exact_payload_bytes_are_not_decoded_or_normalized() {
    let repository = TestRepository::new();
    let bytes = b"line one\r\nline two\rlast\xff";
    repository.bundle(
        "bytes",
        &descriptor("exact-byte-payload", "html-tokenizer", "test.bin"),
        &[("test.bin", bytes)],
    );
    let path = repository.fixture_root().join("bytes/test.bin");
    discover_inventory(&repository.repository()).expect("byte fixture inventory");
    generate_manifest_bytes(&repository.repository()).expect("byte fixture manifest");
    assert_eq!(fs::read(path).expect("exact payload"), bytes);
}

#[test]
fn competing_diagnostics_have_creation_order_independent_order() {
    let build = |reverse: bool| {
        let repository = TestRepository::new();
        let bundles = if reverse {
            ["zeta", "alpha"]
        } else {
            ["alpha", "zeta"]
        };
        for bundle in bundles {
            repository.bundle(bundle, "malformed = [", &[("input.bin", b"input")]);
        }
        let errors = discover_inventory(&repository.repository()).expect_err("invalid inventory");
        errors.to_string()
    };
    assert_eq!(build(false), build(true));
}

#[cfg(unix)]
#[test]
fn symlinks_are_rejected() {
    use std::os::unix::fs::symlink;

    let linked = TestRepository::new();
    linked.bundle(
        "linked",
        &descriptor("linked-payload", "dom-tree", "test.html"),
        &[],
    );
    let outside = linked.root().join("outside.html");
    fs::write(&outside, b"outside").expect("outside target");
    symlink(&outside, linked.fixture_root().join("linked/test.html")).expect("payload symlink");
    assert_kind(&linked, |kind| {
        matches!(kind, InventoryDiagnosticKind::SymlinkNotAllowed)
    });

    let repository = tempfile::tempdir().expect("repository root");
    let outside = tempfile::tempdir().expect("outside root");
    fs::create_dir_all(outside.path().join("conformance/fixtures")).expect("outside fixture root");
    symlink(outside.path(), repository.path().join("tests"))
        .expect("intermediate fixture-root symlink");
    let inventory_repository = InventoryRepository::new(
        repository.path(),
        repository.path().join("tests/conformance/fixtures"),
    );
    let errors = discover_inventory(&inventory_repository).expect_err("symlinked root chain");
    assert!(
        errors.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic.kind,
            InventoryDiagnosticKind::SymlinkNotAllowed
        ))
    );

    let descriptor_link = TestRepository::new();
    let bundle = descriptor_link.fixture_root().join("descriptor-link");
    fs::create_dir_all(&bundle).expect("descriptor-link bundle");
    fs::write(bundle.join("test.html"), b"input").expect("descriptor-link payload");
    let outside_descriptor = descriptor_link.root().join("outside-fixture.toml");
    fs::write(
        &outside_descriptor,
        descriptor("descriptor-link", "dom-tree", "test.html"),
    )
    .expect("outside descriptor");
    symlink(&outside_descriptor, bundle.join("fixture.toml")).expect("descriptor symlink");
    assert_kind(&descriptor_link, |kind| {
        matches!(kind, InventoryDiagnosticKind::SymlinkNotAllowed)
    });
}

#[cfg(unix)]
#[test]
fn nonportable_discovered_path_components_are_rejected_deterministically() {
    let build = |reverse: bool| {
        let repository = TestRepository::new();
        let mut names = vec![
            "Uppercase".to_owned(),
            "contains\\backslash".to_owned(),
            "contains:colon".to_owned(),
            "control-\n".to_owned(),
            "trailing.".to_owned(),
            "trailing ".to_owned(),
            "unicode-é".to_owned(),
            "con".to_owned(),
            "a".repeat(129),
        ];
        if reverse {
            names.reverse();
        }
        for name in names {
            fs::create_dir(repository.fixture_root().join(name))
                .expect("host supports representative invalid component");
        }
        let errors = discover_inventory(&repository.repository()).expect_err("portable paths");
        assert_eq!(
            errors
                .diagnostics()
                .iter()
                .filter(|diagnostic| matches!(
                    diagnostic.kind,
                    InventoryDiagnosticKind::NonPortablePathComponent { .. }
                ))
                .count(),
            9
        );
        errors.to_string()
    };
    assert_eq!(build(false), build(true));
}

// APFS rejects byte sequences that Linux filesystems can preserve as non-UTF-8 names.
#[cfg(target_os = "linux")]
#[test]
fn non_utf8_paths_are_rejected_where_the_host_can_create_them() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let non_utf8 = TestRepository::new();
    let invalid = OsString::from_vec(vec![b'b', b'a', b'd', 0xff]);
    fs::create_dir(non_utf8.fixture_root().join(invalid)).expect("non-UTF-8 path");
    assert_kind(&non_utf8, |kind| {
        matches!(kind, InventoryDiagnosticKind::NonUtf8Path)
    });
}

#[test]
fn every_v1_observation_value_is_explicitly_supported() {
    let values = [
        ("html-tokenizer", ObservationSurface::HtmlTokenizer),
        (
            "html-tree-construction",
            ObservationSurface::HtmlTreeConstruction,
        ),
        ("dom-tree", ObservationSurface::DomTree),
        ("css-parsing", ObservationSurface::CssParsing),
        ("css-selectors", ObservationSurface::CssSelectors),
        ("css-cascade", ObservationSurface::CssCascade),
        ("computed-style", ObservationSurface::ComputedStyle),
        ("layout-geometry", ObservationSurface::LayoutGeometry),
        ("paint-operations", ObservationSurface::PaintOperations),
        (
            "browser-runtime-semantic",
            ObservationSurface::BrowserRuntimeSemantic,
        ),
    ];
    for (index, (value, expected)) in values.into_iter().enumerate() {
        let repository = TestRepository::new();
        repository.bundle(
            "case",
            &descriptor(&format!("observation-{index}"), value, "test.bin"),
            &[("test.bin", b"input")],
        );
        let inventory = discover_inventory(&repository.repository()).expect("valid observation");
        assert_eq!(inventory.fixtures()[0].observation(), expected);
    }
}

#[test]
fn v2_accepts_one_explicit_default_deny_execution_package() {
    let repository = TestRepository::new();
    repository.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-tokenizer",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &["parser/tokens.txt", "parser/parse-errors.txt"],
        ),
        &[
            ("parser/fixture.toml", b"canonical subsystem declaration"),
            ("parser/input.html", b"<p>input"),
            ("parser/tokens.txt", b"tokens"),
            ("parser/parse-errors.txt", b"errors"),
        ],
    );
    let inventory = discover_inventory(&repository.repository()).expect("valid V2 package");
    let fixture = &inventory.fixtures()[0];
    let package = fixture.execution_package().expect("execution package");
    assert_eq!(
        package.entry_path().as_str(),
        "tests/conformance/fixtures/packaged/parser/fixture.toml"
    );
    assert_eq!(
        package
            .support_paths()
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        [
            "tests/conformance/fixtures/packaged/parser/parse-errors.txt",
            "tests/conformance/fixtures/packaged/parser/tokens.txt",
        ]
    );
}

#[test]
fn v2_rejects_undeclared_and_extra_nested_descriptors() {
    let undeclared = TestRepository::new();
    undeclared.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-tokenizer",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &["parser/tokens.txt"],
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/input.html", b"input"),
            ("parser/tokens.txt", b"tokens"),
            ("parser/undeclared.txt", b"not declared"),
        ],
    );
    assert_kind(&undeclared, |kind| {
        matches!(kind, InventoryDiagnosticKind::UndeclaredBundleFile)
    });

    let nested = TestRepository::new();
    nested.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-tokenizer",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &["parser/tokens.txt", "parser/nested/fixture.toml"],
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/input.html", b"input"),
            ("parser/tokens.txt", b"tokens"),
            ("parser/nested/fixture.toml", b"nested"),
        ],
    );
    assert_kind(&nested, |kind| {
        matches!(kind, InventoryDiagnosticKind::NestedFixtureBundle)
    });
}

#[test]
fn v2_rejects_duplicate_and_out_of_package_paths() {
    let duplicate = TestRepository::new();
    duplicate.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-tokenizer",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &["parser/input.html"],
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/input.html", b"input"),
        ],
    );
    assert_kind(&duplicate, |kind| {
        matches!(kind, InventoryDiagnosticKind::DuplicateDeclaredPath { .. })
    });

    let outside = TestRepository::new();
    outside.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-tokenizer",
            "html-tokenizer",
            "input.html",
            "parser/fixture.toml",
            &["parser/tokens.txt"],
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/tokens.txt", b"tokens"),
            ("input.html", b"input"),
        ],
    );
    assert_kind(&outside, |kind| {
        matches!(
            kind,
            InventoryDiagnosticKind::ExecutionFileOutsidePackage { .. }
        )
    });
}

#[test]
fn v2_execution_support_count_accepts_exact_boundary_and_rejects_plus_one() {
    let support = (0..conformance_test_support::MAX_EXECUTION_SUPPORT_PATHS_V2)
        .map(|index| format!("parser/support-{index:03}.txt"))
        .collect::<Vec<_>>();
    let support_refs = support.iter().map(String::as_str).collect::<Vec<_>>();
    let repository = TestRepository::new();
    repository.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-boundary",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &support_refs,
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/input.html", b"input"),
        ],
    );
    for path in &support {
        fs::write(
            repository.fixture_root().join("packaged").join(path),
            b"support",
        )
        .expect("support file");
    }
    discover_inventory(&repository.repository()).expect("exact support-path boundary");

    let too_many = support
        .iter()
        .map(String::as_str)
        .chain(std::iter::once("parser/support-extra.txt"))
        .collect::<Vec<_>>();
    let repository = TestRepository::new();
    repository.bundle(
        "packaged",
        &descriptor_v2(
            "packaged-boundary-plus-one",
            "html-tokenizer",
            "parser/input.html",
            "parser/fixture.toml",
            &too_many,
        ),
        &[
            ("parser/fixture.toml", b"entry"),
            ("parser/input.html", b"input"),
        ],
    );
    assert_kind(&repository, |kind| {
        matches!(
            kind,
            InventoryDiagnosticKind::TooManyExecutionSupportPaths {
                declared: 257,
                maximum: 256
            }
        )
    });
}

fn assert_kind(repository: &TestRepository, predicate: impl Fn(&InventoryDiagnosticKind) -> bool) {
    let errors = discover_inventory(&repository.repository()).expect_err("invalid inventory");
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| predicate(&diagnostic.kind)),
        "diagnostics: {:?}",
        errors.diagnostics()
    );
}

fn descriptor_padded_to(id: &str, size: usize) -> String {
    let mut value = descriptor(id, "dom-tree", "test.html");
    let padding = size
        .checked_sub(value.len())
        .expect("descriptor target size");
    assert!(padding >= 2, "padding must fit a TOML comment");
    value.push('#');
    value.push_str(&"x".repeat(padding - 2));
    value.push('\n');
    assert_eq!(value.len(), size);
    value
}
