//! One embedded catalogue, validated before selection or artifact publication.
use crate::model::{Fixture, FixtureKind, HarnessError};
use crate::projection::{self, Request};

mod generated {
    // The metadata corpus determines which variants the generated source uses.
    #[allow(unused_imports)]
    use crate::model::{
        CanvasColor, Checks, DocumentInput, Expectation, Expected, Fixture, FixtureKind,
        HarnessError, IndependentExpectation, RelationshipKind, TestId, UpstreamAssertion,
        UpstreamSource,
    };
    #[allow(unused_imports)]
    use crate::projection::{ObservationRequest, Request, StyleField, StyleProperty};
    include!(concat!(env!("OUT_DIR"), "/fixtures_generated.rs"));

    pub(super) fn fixtures() -> &'static [Fixture] {
        GENERATED_FIXTURES
    }
}

fn validate_catalogue(fixtures: &[Fixture]) -> Result<(), HarnessError> {
    if fixtures.is_empty() {
        return Err(HarnessError::new(
            "fixture.configuration",
            "empty embedded fixture catalogue",
        ));
    }
    for (index, fixture) in fixtures.iter().enumerate() {
        let validate = || {
            if index > 0 && fixtures[index - 1].id.0 >= fixture.id.0 {
                return Err(HarnessError::new(
                    "fixture.configuration",
                    "fixture identities must be unique and sorted",
                ));
            }
            projection::validate_request(fixture.request)?;
            if matches!(fixture.kind, FixtureKind::Runnable { .. })
                && matches!(fixture.request.dom, Request::NotRequested)
                && fixture.request.styles.is_empty()
                && matches!(fixture.request.layout, Request::NotRequested)
                && matches!(fixture.request.canvas, Request::NotRequested)
            {
                return Err(HarnessError::new(
                    "fixture.configuration",
                    "no observation requested",
                ));
            }
            if let FixtureKind::Runnable { .. } = fixture.kind {
                if fixture.requires_javascript {
                    return Err(HarnessError::new(
                        "fixture.configuration",
                        "runnable fixture requires JavaScript",
                    ));
                }
                if let Some(independent) = fixture.independent() {
                    independent
                        .expected
                        .observations()?
                        .validate(fixture.request)?;
                }
                let mut documents = vec![fixture.document];
                if let Some((_, reference)) = fixture.reference() {
                    crate::relationship::validate_scope(fixture.request)?;
                    documents.push(reference);
                }
                for doc in documents {
                    use sha2::{Digest, Sha256};
                    if doc.sha256 != format!("{:x}", Sha256::digest(doc.html)) {
                        return Err(HarnessError::new(
                            "fixture.source",
                            "embedded byte hash mismatch",
                        ));
                    }
                    crate::metadata_links::validate(&doc.links()?)
                        .map_err(|e| HarnessError::new("fixture.links", e))?;
                }
            }
            Ok(())
        };
        validate().map_err(|error: HarnessError| {
            HarnessError::new(
                error.code,
                format!(
                    "{} ({}): {}",
                    fixture.metadata_source, fixture.id.0, error.detail
                ),
            )
        })?;
    }
    Ok(())
}

pub(crate) fn catalogue() -> Result<&'static [Fixture], HarnessError> {
    let fixtures = generated::fixtures();
    validate_catalogue(fixtures)?;
    Ok(fixtures)
}

pub(crate) fn select(id: Option<&str>) -> Result<Vec<&'static Fixture>, HarnessError> {
    select_suite(None, id)
}

/// Harness selection only; execution backends receive selected documents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Suite {
    Ci,
    Extended,
}

const CI: &[&str] = &[
    "canvas/cascade",
    "canvas/root",
    "dom/structure",
    "dom/template-namespaces",
    "layout/block-boxes",
    "style/cascade-inheritance",
    "wpt-style/background-color-mismatch",
];
const EXTENDED: &[&str] = &[
    "canvas/cascade",
    "canvas/root",
    "dom/structure",
    "dom/template-namespaces",
    "layout/block-boxes",
    "style/cascade-inheritance",
    "wpt-style/background-color-mismatch",
    "wpt/rgb-alias-p6",
    "wpt/rgb-comments-p6",
    "wpt/unsupported/first-child-js",
    "wpt/unsupported/testdriver",
];

impl Suite {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ci => "ci",
            Self::Extended => "extended",
        }
    }

    fn members(self) -> &'static [&'static str] {
        match self {
            Self::Ci => CI,
            Self::Extended => EXTENDED,
        }
    }
}

pub(crate) fn select_suite(
    suite: Option<Suite>,
    id: Option<&str>,
) -> Result<Vec<&'static Fixture>, HarnessError> {
    select_from(generated::fixtures(), suite, id)
}

fn select_from<'a>(
    fixtures: &'a [Fixture],
    suite: Option<Suite>,
    id: Option<&str>,
) -> Result<Vec<&'a Fixture>, HarnessError> {
    // Validate the entire catalogue even when selecting one member. Validate
    // named membership only on that path: legacy standalone catalogues remain valid.
    validate_catalogue(fixtures)?;
    if let Some(suite) = suite {
        let members = suite.members();
        if members.windows(2).any(|pair| pair[0] >= pair[1])
            || members
                .iter()
                .any(|id| !fixtures.iter().any(|f| f.id.0 == *id))
        {
            return Err(HarnessError::new(
                "fixture.configuration",
                format!(
                    "{} suite members must exist, be unique and sorted",
                    suite.name()
                ),
            ));
        }
    }
    let selected: Vec<_> = fixtures
        .iter()
        .filter(|f| suite.is_none_or(|suite| suite.members().contains(&f.id.0)))
        .filter(|f| id.is_none_or(|id| f.id.0 == id))
        .collect();
    if selected.is_empty() {
        return Err(HarnessError::new(
            "fixture.selection",
            match id {
                Some(id) => match suite {
                    Some(suite) => format!("test identity {id:?} is not in suite {}", suite.name()),
                    None => format!("unknown test identity {id:?}"),
                },
                None => "empty fixture selection".into(),
            },
        ));
    }
    Ok(selected)
}

#[cfg(test)]
pub(crate) fn fixture(id: &str) -> &'static Fixture {
    catalogue()
        .expect("test catalogue")
        .iter()
        .find(|f| f.id.0 == id)
        .expect("named baseline fixture")
}

// Independent original-fixture acceptance set, never a discovery registry.
#[cfg(test)]
pub(crate) fn baseline() -> &'static [Fixture] {
    static BASELINE: std::sync::OnceLock<[Fixture; 6]> = std::sync::OnceLock::new();
    BASELINE.get_or_init(|| {
        [
            "canvas/root",
            "canvas/cascade",
            "dom/structure",
            "dom/template-namespaces",
            "style/cascade-inheritance",
            "layout/block-boxes",
        ]
        .map(|id| *fixture(id))
    })
}

#[cfg(test)]
mod tests;
