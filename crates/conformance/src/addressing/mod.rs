//! Local AH9 conformance, consuming addressing and committed Browser results.
mod document;
pub(crate) mod input;
pub(crate) mod observations;
#[cfg(test)]
mod tests;
use crate::model::HarnessError;
use ::addressing::{BrowserUrl, ResourceAddress, ResourceKind};
use input::{AddressingCase, OriginOperand, Purpose, UrlCase};
use observations::*;

pub(crate) const COMPARISON_UNSUPPORTED: &str =
    "AH9 addressing observations are local-only; Chromium addressing capture is outside this issue";

#[derive(Clone, Copy, Debug)]
pub(crate) struct AddressingInput {
    pub declaration: &'static str,
    pub assets: &'static [(&'static str, &'static str)],
}
impl AddressingInput {
    pub(crate) fn case(self) -> Result<AddressingCase, HarnessError> {
        #[derive(serde::Deserialize)]
        struct Envelope {
            addressing: AddressingCase,
        }
        // The full envelope is strictly checked by AG's build-time declaration
        // reader. Reuse the same closed nested schema at runtime preflight.
        let case = serde_json::from_str::<Envelope>(self.declaration)
            .map_err(|e| HarnessError::new("addressing.input", e.to_string()))?
            .addressing;
        case.validate()
            .map_err(|e| HarnessError::new("addressing.input", e))?;
        for (name, _) in case.assets() {
            self.asset(name)?;
        }
        Ok(case)
    }
    pub(crate) fn asset(self, name: &str) -> Result<&'static str, HarnessError> {
        self.assets
            .iter()
            .find(|(path, _)| *path == name)
            .map(|(_, text)| *text)
            .ok_or_else(|| {
                HarnessError::new(
                    "addressing.asset",
                    format!("missing embedded asset {name:?}"),
                )
            })
    }
}
impl Purpose {
    fn resource_kind(self) -> ResourceKind {
        match self {
            Self::Document => ResourceKind::Document,
            Self::Stylesheet => ResourceKind::Stylesheet,
            Self::Image => ResourceKind::Image,
            Self::Script => ResourceKind::Script,
            Self::Font => ResourceKind::Font,
            Self::Media => ResourceKind::Media,
            Self::FormAction => ResourceKind::FormAction,
            Self::Hyperlink => ResourceKind::Hyperlink,
            Self::Unknown => ResourceKind::Unknown,
        }
    }
}
pub(crate) fn expected(
    json: &str,
    case: &AddressingCase,
) -> Result<AddressingObservation, HarnessError> {
    let value: AddressingObservation = serde_json::from_str(json)
        .map_err(|e| HarnessError::new("expectation.invalid", e.to_string()))?;
    value.validate(case)?;
    Ok(value)
}
fn url_case(input: &UrlCase) -> Result<(UrlObservation, Option<BrowserUrl>), HarnessError> {
    let base = input.base.as_deref().map(BrowserUrl::parse);
    let base_observation = match (&input.base, &base) {
        (Some(text), Some(result)) => Availability::Present(base_result(
            "explicit",
            Availability::Present(text.clone()),
            result,
        )?),
        _ => Availability::Inapplicable,
    };
    let result = match &base {
        None => BrowserUrl::parse(&input.reference),
        Some(Ok(base)) => base.url.resolve(&input.reference),
        Some(Err(_)) => {
            return Ok((
                UrlObservation {
                    input: input.reference.clone(),
                    base: base_observation,
                    outcome: UrlOutcome::UnresolvedBase {},
                    source: Availability::Inapplicable,
                },
                None,
            ));
        }
    };
    let (outcome, url) = match result {
        Ok(result) => {
            let resource = input.purpose.map_or(Availability::Inapplicable, |p| {
                Availability::Present(address(&ResourceAddress::new(
                    result.url.clone(),
                    p.resource_kind(),
                )))
            });
            (
                UrlOutcome::Parsed {
                    url: Box::new(parsed(&result.url)),
                    diagnostics: diagnostics(&result.diagnostics),
                    origin: origin(&result.url.url_origin()),
                    resource,
                },
                Some(result.url),
            )
        }
        Err(error) => (
            UrlOutcome::Failed {
                diagnostics: diagnostics(&error.diagnostics),
                failure: failure(&error)?,
            },
            None,
        ),
    };
    Ok((
        UrlObservation {
            input: input.reference.clone(),
            base: base_observation,
            outcome,
            source: Availability::Inapplicable,
        },
        url,
    ))
}
pub(crate) fn base_result(
    source: &str,
    input: Availability<String>,
    result: &Result<::addressing::UrlParseOutcome, ::addressing::UrlFailure>,
) -> Result<BaseObservation, HarnessError> {
    Ok(match result {
        Ok(result) => BaseObservation {
            source: source.into(),
            input,
            url: Availability::Present(parsed(&result.url)),
            diagnostics: diagnostics(&result.diagnostics),
            failure: Availability::Inapplicable,
        },
        Err(error) => BaseObservation {
            source: source.into(),
            input,
            url: Availability::Unavailable("base_parse_failed".into()),
            diagnostics: diagnostics(&error.diagnostics),
            failure: Availability::Present(failure(error)?),
        },
    })
}
pub(crate) fn execute(input: AddressingInput) -> Result<AddressingObservation, HarnessError> {
    let case = input.case()?;
    let result = match &case {
        AddressingCase::Url { rows } => AddressingObservation::Url(
            rows.iter()
                .map(|row| url_case(row).map(|v| v.0))
                .collect::<Result<_, _>>()?,
        ),
        AddressingCase::Origins { rows } => AddressingObservation::Origins(
            rows.iter()
                .map(|row| {
                    let (left, url) = url_case(&UrlCase {
                        reference: row.left.clone(),
                        base: None,
                        purpose: None,
                    })?;
                    let left_origin = url
                        .ok_or_else(|| {
                            HarnessError::new(
                                "addressing.required-data",
                                "origin operand did not parse",
                            )
                        })?
                        .url_origin();
                    let (right, right_origin, creation) = match &row.right {
                        OriginOperand::CloneLeft {} => {
                            (left.clone(), left_origin.clone(), "clone_left")
                        }
                        OriginOperand::Independent { url } => {
                            let (observation, parsed) = url_case(&UrlCase {
                                reference: url.clone(),
                                base: None,
                                purpose: None,
                            })?;
                            let origin = parsed
                                .ok_or_else(|| {
                                    HarnessError::new(
                                        "addressing.required-data",
                                        "origin operand did not parse",
                                    )
                                })?
                                .url_origin();
                            (observation, origin, "independent")
                        }
                    };
                    Ok(OriginPairObservation {
                        left,
                        right,
                        right_creation: creation.into(),
                        same_origin: left_origin.is_same_origin(&right_origin),
                    })
                })
                .collect::<Result<_, HarnessError>>()?,
        ),
        AddressingCase::Keys { rows } => AddressingObservation::Keys(
            rows.iter()
                .map(|row| {
                    let (left, left_url) = url_case(&row.left)?;
                    let (right, right_url) = url_case(&row.right)?;
                    let left_url = left_url.ok_or_else(|| {
                        HarnessError::new("addressing.required-data", "key operand did not parse")
                    })?;
                    let right_url = right_url.ok_or_else(|| {
                        HarnessError::new("addressing.required-data", "key operand did not parse")
                    })?;
                    let a = ResourceAddress::new(
                        left_url.clone(),
                        row.left
                            .purpose
                            .ok_or_else(|| {
                                HarnessError::new("addressing.input", "missing purpose")
                            })?
                            .resource_kind(),
                    );
                    let b = ResourceAddress::new(
                        right_url.clone(),
                        row.right
                            .purpose
                            .ok_or_else(|| {
                                HarnessError::new("addressing.input", "missing purpose")
                            })?
                            .resource_kind(),
                    );
                    Ok(KeyPairObservation {
                        left,
                        right,
                        equal_keys: a.retrieval_key() == b.retrieval_key(),
                        equal_urls: left_url == right_url,
                    })
                })
                .collect::<Result<_, HarnessError>>()?,
        ),
        AddressingCase::Documents { rows } => AddressingObservation::Documents(
            rows.iter()
                .map(|row| document::execute(input, row))
                .collect::<Result<_, _>>()?,
        ),
    };
    result.validate(&case)?;
    Ok(result)
}
