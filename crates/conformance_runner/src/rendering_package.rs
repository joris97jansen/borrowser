use std::collections::BTreeSet;
use std::path::Path;

use conformance_test_support::{
    FixtureFormat, MAX_DESCRIPTOR_BYTES, MAX_EXECUTION_SUPPORT_PATHS_V2, ReferenceKind,
    ReferenceRelation, ValidatedFixture,
};
use rendering_test_support::{
    PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1, PairedRenderingFixtureLimitConfigurationError,
    PairedRenderingFixtureLimits, PairedRenderingFixtureLoadError, PairedRenderingFixturePackage,
    RENDERING_EXPECTATION_PAIR_COUNT_V1, RENDERING_STYLESHEET_COUNT_V1,
    RenderingFixtureLimitConfigurationError, RenderingFixtureLimits, RenderingFixtureLoadError,
    RenderingFixturePackage, RenderingObservationOwner, load_fixture_package,
    load_paired_fixture_package,
};

use crate::report::DEFAULT_REPORT_LIMITS;

#[derive(Debug)]
pub enum RenderingPackageReconciliationError {
    FixtureV2Required {
        test_id: String,
    },
    FixtureV3Required {
        test_id: String,
    },
    ReferenceRequired {
        test_id: String,
    },
    ExecutionPackageRequired {
        test_id: String,
    },
    LimitConfiguration(RenderingFixtureLimitConfigurationError),
    PairedLimitConfiguration(PairedRenderingFixtureLimitConfigurationError),
    DescriptorLimitDoesNotFitPlatform {
        configured: u64,
    },
    SupportPathCapacityIncompatible {
        required: usize,
        maximum: usize,
    },
    EntryMustBeFixtureToml {
        test_id: String,
    },
    Nested(RenderingFixtureLoadError),
    PairedNested(PairedRenderingFixtureLoadError),
    IdMismatch {
        outer: String,
        nested: String,
    },
    PackageRootOutsideRepository {
        test_id: String,
    },
    PrimaryInputMismatch {
        outer: String,
        nested: String,
    },
    ReferenceInputMismatch {
        outer: String,
        nested: String,
    },
    PairedSupportPathLimitExceeded {
        actual: usize,
        maximum: usize,
    },
    ReferencedFileSetMismatch {
        test_id: String,
        declared: Vec<String>,
        referenced: Vec<String>,
    },
}

impl RenderingPackageReconciliationError {
    pub const fn stable_label(&self) -> &'static str {
        match self {
            Self::FixtureV2Required { .. } => "fixture-v2-required",
            Self::FixtureV3Required { .. } => "fixture-v3-required",
            Self::ReferenceRequired { .. } => "reference-required",
            Self::ExecutionPackageRequired { .. } => "execution-package-required",
            Self::LimitConfiguration(_) => "limit-configuration",
            Self::PairedLimitConfiguration(_) => "paired-limit-configuration",
            Self::DescriptorLimitDoesNotFitPlatform { .. } => {
                "descriptor-limit-does-not-fit-platform"
            }
            Self::SupportPathCapacityIncompatible { .. } => "support-path-capacity-incompatible",
            Self::EntryMustBeFixtureToml { .. } => "entry-must-be-fixture-toml",
            Self::Nested(_) => "nested-fixture-load",
            Self::PairedNested(_) => "paired-nested-fixture-load",
            Self::IdMismatch { .. } => "id-mismatch",
            Self::PackageRootOutsideRepository { .. } => "package-root-outside-repository",
            Self::PrimaryInputMismatch { .. } => "primary-input-mismatch",
            Self::ReferenceInputMismatch { .. } => "reference-input-mismatch",
            Self::PairedSupportPathLimitExceeded { .. } => "paired-support-path-limit-exceeded",
            Self::ReferencedFileSetMismatch { .. } => "referenced-file-set-mismatch",
        }
    }
}

impl std::fmt::Display for RenderingPackageReconciliationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FixtureV2Required { test_id } => {
                write!(formatter, "rendering case {test_id} requires AG fixture V2")
            }
            Self::FixtureV3Required { test_id } => {
                write!(
                    formatter,
                    "paired rendering case {test_id} requires AG fixture V3"
                )
            }
            Self::ReferenceRequired { test_id } => {
                write!(
                    formatter,
                    "paired rendering case {test_id} requires an outer reference"
                )
            }
            Self::ExecutionPackageRequired { test_id } => write!(
                formatter,
                "rendering case {test_id} requires exactly one AG execution package"
            ),
            Self::LimitConfiguration(error) => write!(formatter, "{error}"),
            Self::PairedLimitConfiguration(error) => write!(formatter, "{error}"),
            Self::DescriptorLimitDoesNotFitPlatform { configured } => write!(
                formatter,
                "AG descriptor limit {configured} does not fit this platform"
            ),
            Self::SupportPathCapacityIncompatible { required, maximum } => write!(
                formatter,
                "rendering V1 may require {required} support paths, exceeding AG2 maximum {maximum}"
            ),
            Self::EntryMustBeFixtureToml { test_id } => write!(
                formatter,
                "rendering case {test_id} execution entry must be fixture.toml"
            ),
            Self::Nested(error) => write!(formatter, "{error}"),
            Self::PairedNested(error) => write!(formatter, "{error}"),
            Self::IdMismatch { outer, nested } => write!(
                formatter,
                "outer AG id {outer} does not equal nested rendering id {nested}"
            ),
            Self::PackageRootOutsideRepository { test_id } => write!(
                formatter,
                "rendering case {test_id} package root is outside the repository"
            ),
            Self::PrimaryInputMismatch { outer, nested } => write!(
                formatter,
                "outer test_path {outer} does not equal nested primary input {nested}"
            ),
            Self::ReferenceInputMismatch { outer, nested } => write!(
                formatter,
                "outer reference.path {outer} does not equal nested reference input {nested}"
            ),
            Self::PairedSupportPathLimitExceeded { actual, maximum } => write!(
                formatter,
                "paired rendering package declares {actual} support paths, exceeding V1 maximum {maximum}"
            ),
            Self::ReferencedFileSetMismatch { test_id, .. } => write!(
                formatter,
                "rendering case {test_id} declared and nested referenced file sets differ"
            ),
        }
    }
}

#[derive(Debug)]
pub struct ReconciledPairedRenderingPackage {
    pub package: PairedRenderingFixturePackage,
    pub reference_kind: ReferenceKind,
    pub relation: ReferenceRelation,
}

impl std::error::Error for RenderingPackageReconciliationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LimitConfiguration(error) => Some(error),
            Self::PairedLimitConfiguration(error) => Some(error),
            Self::Nested(error) => Some(error),
            Self::PairedNested(error) => Some(error),
            _ => None,
        }
    }
}

pub fn reconcile_rendering_package(
    repository_root: &Path,
    outer: &ValidatedFixture,
    owner: RenderingObservationOwner,
) -> Result<RenderingFixturePackage, RenderingPackageReconciliationError> {
    let test_id = outer.id().as_str().to_owned();
    if outer.format() != FixtureFormat::V2 {
        return Err(RenderingPackageReconciliationError::FixtureV2Required { test_id });
    }
    let execution = outer.execution_package().ok_or_else(|| {
        RenderingPackageReconciliationError::ExecutionPackageRequired {
            test_id: test_id.clone(),
        }
    })?;
    if Path::new(execution.entry_path().as_str())
        .file_name()
        .and_then(|name| name.to_str())
        != Some("fixture.toml")
    {
        return Err(RenderingPackageReconciliationError::EntryMustBeFixtureToml { test_id });
    }
    let entry_path = repository_root.join(execution.entry_path().as_str());
    let package = load_fixture_package(&entry_path, owner, ag_rendering_fixture_limits()?)
        .map_err(RenderingPackageReconciliationError::Nested)?;
    if package.id() != outer.id().as_str() {
        return Err(RenderingPackageReconciliationError::IdMismatch {
            outer: outer.id().as_str().to_owned(),
            nested: package.id().to_owned(),
        });
    }
    let package_root = entry_path.parent().ok_or_else(|| {
        RenderingPackageReconciliationError::EntryMustBeFixtureToml {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let package_relative = package_root.strip_prefix(repository_root).map_err(|_| {
        RenderingPackageReconciliationError::PackageRootOutsideRepository {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let nested_primary = portable_display(&package_relative.join(package.primary_input_path()));
    if outer.test_path().as_str() != nested_primary {
        return Err(RenderingPackageReconciliationError::PrimaryInputMismatch {
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
        return Err(
            RenderingPackageReconciliationError::ReferencedFileSetMismatch {
                test_id: outer.id().as_str().to_owned(),
                declared: declared.into_iter().collect(),
                referenced: referenced.into_iter().collect(),
            },
        );
    }
    Ok(package)
}

pub fn reconcile_paired_rendering_package(
    repository_root: &Path,
    outer: &ValidatedFixture,
    owner: RenderingObservationOwner,
) -> Result<ReconciledPairedRenderingPackage, RenderingPackageReconciliationError> {
    let test_id = outer.id().as_str().to_owned();
    if !matches!(outer.format(), FixtureFormat::V3 | FixtureFormat::V4) {
        return Err(RenderingPackageReconciliationError::FixtureV3Required { test_id });
    }
    let reference = outer.reference().ok_or_else(|| {
        RenderingPackageReconciliationError::ReferenceRequired {
            test_id: test_id.clone(),
        }
    })?;
    let execution = outer.execution_package().ok_or_else(|| {
        RenderingPackageReconciliationError::ExecutionPackageRequired {
            test_id: test_id.clone(),
        }
    })?;
    if execution.support_paths().len() > PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1 {
        return Err(
            RenderingPackageReconciliationError::PairedSupportPathLimitExceeded {
                actual: execution.support_paths().len(),
                maximum: PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1,
            },
        );
    }
    if Path::new(execution.entry_path().as_str())
        .file_name()
        .and_then(|name| name.to_str())
        != Some("fixture.toml")
    {
        return Err(RenderingPackageReconciliationError::EntryMustBeFixtureToml { test_id });
    }
    let entry_path = repository_root.join(execution.entry_path().as_str());
    let package = load_paired_fixture_package(&entry_path, owner, ag_paired_rendering_limits()?)
        .map_err(RenderingPackageReconciliationError::PairedNested)?;
    if package.id() != outer.id().as_str() {
        return Err(RenderingPackageReconciliationError::IdMismatch {
            outer: outer.id().as_str().to_owned(),
            nested: package.id().to_owned(),
        });
    }
    let package_root = entry_path.parent().ok_or_else(|| {
        RenderingPackageReconciliationError::EntryMustBeFixtureToml {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let package_relative = package_root.strip_prefix(repository_root).map_err(|_| {
        RenderingPackageReconciliationError::PackageRootOutsideRepository {
            test_id: outer.id().as_str().to_owned(),
        }
    })?;
    let nested_test = portable_display(&package_relative.join(package.test_input_path()));
    if outer.test_path().as_str() != nested_test {
        return Err(RenderingPackageReconciliationError::PrimaryInputMismatch {
            outer: outer.test_path().as_str().to_owned(),
            nested: nested_test,
        });
    }
    let nested_reference = portable_display(&package_relative.join(package.reference_input_path()));
    if reference.path().as_str() != nested_reference {
        return Err(
            RenderingPackageReconciliationError::ReferenceInputMismatch {
                outer: reference.path().as_str().to_owned(),
                nested: nested_reference,
            },
        );
    }
    let declared: BTreeSet<_> = [
        outer.test_path().as_str().to_owned(),
        reference.path().as_str().to_owned(),
    ]
    .into_iter()
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
        return Err(
            RenderingPackageReconciliationError::ReferencedFileSetMismatch {
                test_id: outer.id().as_str().to_owned(),
                declared: declared.into_iter().collect(),
                referenced: referenced.into_iter().collect(),
            },
        );
    }
    Ok(ReconciledPairedRenderingPackage {
        package,
        reference_kind: reference.kind(),
        relation: reference.relation(),
    })
}

pub(crate) fn ag_rendering_fixture_limits()
-> Result<RenderingFixtureLimits, RenderingPackageReconciliationError> {
    let descriptor_bytes = usize::try_from(MAX_DESCRIPTOR_BYTES).map_err(|_| {
        RenderingPackageReconciliationError::DescriptorLimitDoesNotFitPlatform {
            configured: MAX_DESCRIPTOR_BYTES,
        }
    })?;
    let required_support_paths = RENDERING_STYLESHEET_COUNT_V1
        .checked_add(RENDERING_EXPECTATION_PAIR_COUNT_V1)
        .ok_or(
            RenderingPackageReconciliationError::SupportPathCapacityIncompatible {
                required: usize::MAX,
                maximum: MAX_EXECUTION_SUPPORT_PATHS_V2,
            },
        )?;
    if required_support_paths > MAX_EXECUTION_SUPPORT_PATHS_V2 {
        return Err(
            RenderingPackageReconciliationError::SupportPathCapacityIncompatible {
                required: required_support_paths,
                maximum: MAX_EXECUTION_SUPPORT_PATHS_V2,
            },
        );
    }
    RenderingFixtureLimits::try_new(descriptor_bytes, DEFAULT_REPORT_LIMITS.observation_bytes)
        .map_err(RenderingPackageReconciliationError::LimitConfiguration)
}

pub(crate) fn ag_paired_rendering_limits()
-> Result<PairedRenderingFixtureLimits, RenderingPackageReconciliationError> {
    let descriptor_bytes = usize::try_from(MAX_DESCRIPTOR_BYTES).map_err(|_| {
        RenderingPackageReconciliationError::DescriptorLimitDoesNotFitPlatform {
            configured: MAX_DESCRIPTOR_BYTES,
        }
    })?;
    if PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1 > MAX_EXECUTION_SUPPORT_PATHS_V2 {
        return Err(
            RenderingPackageReconciliationError::SupportPathCapacityIncompatible {
                required: PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1,
                maximum: MAX_EXECUTION_SUPPORT_PATHS_V2,
            },
        );
    }
    PairedRenderingFixtureLimits::try_new(descriptor_bytes, DEFAULT_REPORT_LIMITS.observation_bytes)
        .map_err(RenderingPackageReconciliationError::PairedLimitConfiguration)
}

fn portable_display(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ag_transport_and_report_limits_reconcile_with_rendering_v1() {
        let limits = ag_rendering_fixture_limits().unwrap();
        assert_eq!(limits.descriptor_bytes() as u64, MAX_DESCRIPTOR_BYTES);
        assert_eq!(
            limits.expected_snapshot_bytes(),
            DEFAULT_REPORT_LIMITS.observation_bytes
        );
        assert_eq!(
            RENDERING_STYLESHEET_COUNT_V1 + RENDERING_EXPECTATION_PAIR_COUNT_V1,
            144
        );
        assert!(
            limits.stylesheet_count() + limits.expectation_pair_count()
                <= MAX_EXECUTION_SUPPORT_PATHS_V2
        );
    }

    #[test]
    fn ag7_paired_support_sublimit_is_beneath_the_ag2_transport_ceiling() {
        let limits = ag_paired_rendering_limits().unwrap();
        assert_eq!(
            limits.observation_bytes(),
            DEFAULT_REPORT_LIMITS.observation_bytes
        );
        assert_eq!(PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1, 64);
        assert!(PAIRED_RENDERING_EXECUTION_SUPPORT_PATHS_V1 < MAX_EXECUTION_SUPPORT_PATHS_V2);
    }

    #[test]
    fn reconciliation_errors_have_closed_stable_identity_and_typed_sources() {
        let mismatch = RenderingPackageReconciliationError::IdMismatch {
            outer: "outer".to_owned(),
            nested: "nested".to_owned(),
        };
        assert_eq!(mismatch.stable_label(), "id-mismatch");
        assert_eq!(
            mismatch.to_string(),
            "outer AG id outer does not equal nested rendering id nested"
        );

        let nested =
            RenderingPackageReconciliationError::Nested(RenderingFixtureLoadError::Invalid(
                rendering_test_support::RenderingFixtureProblem::EmptyProfiles,
            ));
        assert_eq!(nested.stable_label(), "nested-fixture-load");
        assert!(std::error::Error::source(&nested).is_some());

        let paired = RenderingPackageReconciliationError::PairedNested(
            PairedRenderingFixtureLoadError::Invalid(
                rendering_test_support::PairedRenderingFixtureProblem::EmptyProfiles,
            ),
        );
        assert_eq!(paired.stable_label(), "paired-nested-fixture-load");
        assert!(std::error::Error::source(&paired).is_some());
    }
}
