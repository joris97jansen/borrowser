use std::collections::BTreeSet;
use std::path::Path;

use conformance_test_support::{
    FixtureFormat, MAX_DESCRIPTOR_BYTES, MAX_EXECUTION_SUPPORT_PATHS_V2, ObservationSurface,
    ValidatedFixture,
};
use css_test_support::{
    CssExecutionProfile, CssFixtureLimitConfigurationError, CssFixtureLimits, CssFixtureLoadError,
    CssFixturePackage, load_fixture_package,
};

use crate::report::DEFAULT_REPORT_LIMITS;

#[derive(Debug)]
pub enum CssPackageReconciliationError {
    FixtureV2Required {
        test_id: String,
    },
    ExecutionPackageRequired {
        test_id: String,
    },
    LimitConfiguration(CssFixtureLimitConfigurationError),
    DescriptorLimitDoesNotFitPlatform {
        configured: u64,
    },
    EntryMustBeFixtureToml {
        test_id: String,
    },
    Nested(CssFixtureLoadError),
    IdMismatch {
        outer: String,
        nested: String,
    },
    ObservationProfileMismatch {
        test_id: String,
        observation: ObservationSurface,
        profile: CssExecutionProfile,
    },
    PackageRootOutsideRepository {
        test_id: String,
    },
    PrimaryInputMismatch {
        outer: String,
        nested: String,
    },
    ReferencedFileSetMismatch {
        test_id: String,
        declared: Vec<String>,
        referenced: Vec<String>,
    },
}

impl std::fmt::Display for CssPackageReconciliationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixtureV2Required { test_id } => {
                write!(formatter, "CSS case {test_id} requires AG fixture V2")
            }
            Self::ExecutionPackageRequired { test_id } => write!(
                formatter,
                "CSS case {test_id} requires exactly one AG execution package"
            ),
            Self::LimitConfiguration(error) => write!(formatter, "{error}"),
            Self::DescriptorLimitDoesNotFitPlatform { configured } => write!(
                formatter,
                "AG descriptor limit {configured} does not fit this platform"
            ),
            Self::EntryMustBeFixtureToml { test_id } => {
                write!(
                    formatter,
                    "CSS case {test_id} execution entry must be fixture.toml"
                )
            }
            Self::Nested(error) => write!(formatter, "{error}"),
            Self::IdMismatch { outer, nested } => {
                write!(
                    formatter,
                    "outer AG id {outer} does not equal nested CSS id {nested}"
                )
            }
            Self::ObservationProfileMismatch {
                test_id,
                observation,
                profile,
            } => write!(
                formatter,
                "CSS case {test_id} AG observation {} does not admit profile {}",
                observation.as_str(),
                profile.stable_label()
            ),
            Self::PackageRootOutsideRepository { test_id } => {
                write!(
                    formatter,
                    "CSS case {test_id} package root is outside the repository"
                )
            }
            Self::PrimaryInputMismatch { outer, nested } => write!(
                formatter,
                "outer test_path {outer} does not equal nested primary input {nested}"
            ),
            Self::ReferencedFileSetMismatch { test_id, .. } => write!(
                formatter,
                "CSS case {test_id} declared and nested referenced file sets differ"
            ),
        }
    }
}

impl std::error::Error for CssPackageReconciliationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Nested(error) => Some(error),
            Self::LimitConfiguration(error) => Some(error),
            _ => None,
        }
    }
}

pub fn reconcile_css_package(
    repository_root: &Path,
    outer: &ValidatedFixture,
) -> Result<CssFixturePackage, CssPackageReconciliationError> {
    let test_id = outer.id().as_str().to_owned();
    if outer.format() != FixtureFormat::V2 {
        return Err(CssPackageReconciliationError::FixtureV2Required { test_id });
    }
    let execution = outer.execution_package().ok_or_else(|| {
        CssPackageReconciliationError::ExecutionPackageRequired {
            test_id: test_id.clone(),
        }
    })?;
    if Path::new(execution.entry_path().as_str())
        .file_name()
        .and_then(|name| name.to_str())
        != Some("fixture.toml")
    {
        return Err(CssPackageReconciliationError::EntryMustBeFixtureToml { test_id });
    }
    let entry_path = repository_root.join(execution.entry_path().as_str());
    let limits = ag_css_fixture_limits()?;
    let package =
        load_fixture_package(&entry_path, limits).map_err(CssPackageReconciliationError::Nested)?;
    if package.id() != outer.id().as_str() {
        return Err(CssPackageReconciliationError::IdMismatch {
            outer: outer.id().as_str().to_owned(),
            nested: package.id().to_owned(),
        });
    }
    if !profile_matches_observation(package.profile(), outer.observation()) {
        return Err(CssPackageReconciliationError::ObservationProfileMismatch {
            test_id,
            observation: outer.observation(),
            profile: package.profile(),
        });
    }
    let package_root = entry_path.parent().ok_or_else(|| {
        CssPackageReconciliationError::EntryMustBeFixtureToml {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let package_relative = package_root.strip_prefix(repository_root).map_err(|_| {
        CssPackageReconciliationError::PackageRootOutsideRepository {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let nested_primary = portable_display(&package_relative.join(package.primary_input_path()));
    if outer.test_path().as_str() != nested_primary {
        return Err(CssPackageReconciliationError::PrimaryInputMismatch {
            outer: outer.test_path().as_str().to_owned(),
            nested: nested_primary,
        });
    }
    let declared: BTreeSet<_> = std::iter::once(outer.test_path().as_str().to_owned())
        .chain(
            execution
                .support_paths()
                .iter()
                .map(|path| path.as_str().to_owned()),
        )
        .collect();
    let referenced: BTreeSet<_> = package
        .referenced_paths()
        .map(|path| portable_display(&package_relative.join(path)))
        .collect();
    if declared != referenced {
        return Err(CssPackageReconciliationError::ReferencedFileSetMismatch {
            test_id: outer.id().as_str().to_owned(),
            declared: declared.into_iter().collect(),
            referenced: referenced.into_iter().collect(),
        });
    }
    Ok(package)
}

fn ag_css_fixture_limits() -> Result<CssFixtureLimits, CssPackageReconciliationError> {
    let max_descriptor_bytes = usize::try_from(MAX_DESCRIPTOR_BYTES).map_err(|_| {
        CssPackageReconciliationError::DescriptorLimitDoesNotFitPlatform {
            configured: MAX_DESCRIPTOR_BYTES,
        }
    })?;
    // A combined fixture uses one stylesheet as `test_path`; every additional
    // stylesheet plus the required HTML and expected snapshot are support
    // paths. Therefore S + 1 must fit AG2's support-path capacity.
    let package_stylesheet_maximum = MAX_EXECUTION_SUPPORT_PATHS_V2.saturating_sub(1);
    let max_stylesheets =
        package_stylesheet_maximum.min(CssFixtureLimits::production_stylesheet_maximum());
    CssFixtureLimits::try_new(
        max_descriptor_bytes,
        DEFAULT_REPORT_LIMITS.observation_bytes,
        max_stylesheets,
    )
    .map_err(CssPackageReconciliationError::LimitConfiguration)
}

pub const fn profile_matches_observation(
    profile: CssExecutionProfile,
    observation: ObservationSurface,
) -> bool {
    matches!(
        (profile, observation),
        (
            CssExecutionProfile::PropertyValue,
            ObservationSurface::CssParsing
        ) | (
            CssExecutionProfile::SelectorParsing
                | CssExecutionProfile::SelectorSpecificity
                | CssExecutionProfile::SelectorMatching,
            ObservationSurface::CssSelectors
        ) | (
            CssExecutionProfile::CascadeWinner | CssExecutionProfile::InheritanceCssWide,
            ObservationSurface::CssCascade
        ) | (
            CssExecutionProfile::ComputedStyle,
            ObservationSurface::ComputedStyle
        )
    )
}

fn portable_display(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use conformance_test_support::{InventoryRepository, discover_inventory};

    use super::*;

    #[test]
    fn runner_configuration_reconciles_ag_transport_with_subsystem_bounds() {
        let limits = ag_css_fixture_limits().expect("compatible AG5 limits");
        assert_eq!(
            limits.max_descriptor_bytes(),
            usize::try_from(MAX_DESCRIPTOR_BYTES).expect("AG descriptor limit fits test platform")
        );
        assert_eq!(
            limits.max_expected_bytes(),
            DEFAULT_REPORT_LIMITS.observation_bytes
        );
        assert_eq!(
            limits.max_target_depth(),
            CssFixtureLimits::production_target_depth_maximum()
        );
        assert_eq!(
            limits.max_html_input_bytes(),
            css_test_support::CSS_NESTED_MAX_HTML_INPUT_BYTES
        );
        assert_eq!(
            limits.max_targets(),
            css_test_support::CSS_NESTED_MAX_TARGETS
        );
        assert!(
            limits.max_html_input_bytes() <= limits.max_expected_bytes(),
            "CSS-owned authored HTML capacity must fit below retained AG observation capacity"
        );
        assert!(
            limits
                .max_targets()
                .checked_mul(css_test_support::CSS_NESTED_MAX_TARGET_LABEL_BYTES)
                .is_some_and(|label_bytes| label_bytes <= limits.max_expected_bytes()),
            "maximum retained target-label evidence must fit AG observation capacity"
        );
        assert!(
            limits.max_stylesheets() < MAX_EXECUTION_SUPPORT_PATHS_V2,
            "stylesheet primary/support accounting must fit AG2 V2"
        );
        assert!(
            limits.max_stylesheets() <= CssFixtureLimits::production_stylesheet_maximum(),
            "fixture capacity must not exceed the production style pass"
        );
    }

    fn write_case(
        observation: &str,
        test_path: &str,
        support_paths: &[&str],
        nested_id: &str,
        include_extra: bool,
    ) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary repository");
        let bundle = root.path().join("tests/conformance/fixtures/case");
        let nested = bundle.join("css");
        fs::create_dir_all(&nested).expect("fixture directories");
        let supports = support_paths
            .iter()
            .map(|path| format!("\"{path}\""))
            .collect::<Vec<_>>()
            .join(", ");
        fs::write(
            bundle.join("fixture.toml"),
            format!(
                r#"format = "borrowser-conformance-fixture-v2"
id = "case"
scope = "static-html-css-no-js"
observation = "{observation}"
test_path = "{test_path}"
[source]
kind = "native"
[metadata]
description = "CSS reconciliation test."
[execution_package]
entry_path = "css/fixture.toml"
support_paths = [{supports}]
"#
            ),
        )
        .expect("outer descriptor");
        fs::write(
            nested.join("fixture.toml"),
            format!(
                r#"format = "borrowser-css-fixture-v1"
id = "{nested_id}"
profile = "selector-parsing"
[input]
selector_list = "selectors.txt"
[expectations]
snapshot = "expected.txt"
"#
            ),
        )
        .expect("nested descriptor");
        fs::write(nested.join("selectors.txt"), "div").expect("selector source");
        fs::write(nested.join("expected.txt"), "expected").expect("expected snapshot");
        if include_extra {
            fs::write(nested.join("extra.txt"), "declared but unreferenced")
                .expect("extra support");
        }
        root
    }

    fn reconcile(
        root: &tempfile::TempDir,
    ) -> Result<CssFixturePackage, CssPackageReconciliationError> {
        let fixture_root = root.path().join("tests/conformance/fixtures");
        let inventory = discover_inventory(&InventoryRepository::new(root.path(), fixture_root))
            .expect("valid AG2 inventory");
        reconcile_css_package(root.path(), &inventory.fixtures()[0])
    }

    #[test]
    fn outer_and_nested_css_contracts_reconcile_only_when_all_semantic_keys_match() {
        let valid = write_case(
            "css-selectors",
            "css/selectors.txt",
            &["css/expected.txt"],
            "case",
            false,
        );
        assert!(reconcile(&valid).is_ok());

        let id = write_case(
            "css-selectors",
            "css/selectors.txt",
            &["css/expected.txt"],
            "different",
            false,
        );
        assert!(matches!(
            reconcile(&id),
            Err(CssPackageReconciliationError::IdMismatch { .. })
        ));

        let surface = write_case(
            "css-parsing",
            "css/selectors.txt",
            &["css/expected.txt"],
            "case",
            false,
        );
        assert!(matches!(
            reconcile(&surface),
            Err(CssPackageReconciliationError::ObservationProfileMismatch { .. })
        ));

        let primary = write_case(
            "css-selectors",
            "css/expected.txt",
            &["css/selectors.txt"],
            "case",
            false,
        );
        assert!(matches!(
            reconcile(&primary),
            Err(CssPackageReconciliationError::PrimaryInputMismatch { .. })
        ));

        let files = write_case(
            "css-selectors",
            "css/selectors.txt",
            &["css/expected.txt", "css/extra.txt"],
            "case",
            true,
        );
        assert!(matches!(
            reconcile(&files),
            Err(CssPackageReconciliationError::ReferencedFileSetMismatch { .. })
        ));
    }
}
