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

    pub(super) fn validated() -> Result<&'static [Fixture], HarnessError> {
        super::validate_catalogue(GENERATED_FIXTURES)?;
        Ok(GENERATED_FIXTURES)
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
    generated::validated()
}

pub(crate) fn select(id: Option<&str>) -> Result<Vec<&'static Fixture>, HarnessError> {
    let fixtures = catalogue()?;
    let selected: Vec<_> = fixtures
        .iter()
        .filter(|f| id.is_none_or(|id| f.id.0 == id))
        .collect();
    if selected.is_empty() {
        return Err(HarnessError::new(
            "fixture.selection",
            match id {
                Some(id) => format!("unknown test identity {id:?}"),
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
