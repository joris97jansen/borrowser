//! AG-local networking observations consume production Fetch facts exclusively.
pub(crate) mod bounds;
mod execute;
pub(crate) mod input;
pub(crate) mod observations;
#[cfg(test)]
mod tests;
use crate::{addressing::input as purpose, model::HarnessError};
use addressing::{BrowserUrl, ResourceAddress};
pub(crate) use execute::{Failure, ProducerClosure};
use input::*;
use observations::*;
use std::sync::{Arc, atomic::AtomicBool};

pub(crate) const COMPARISON_UNSUPPORTED: &str =
    "Networking observations are local-only; Chromium networking capture is unavailable";
#[derive(Clone, Copy, Debug)]
pub(crate) struct NetworkingInput {
    pub declaration: &'static str,
}
impl NetworkingInput {
    pub(crate) fn case(self) -> Result<NetworkingCase, HarnessError> {
        if self.declaration.len() > bounds::DECLARATION {
            return Err(error("declaration byte budget exceeded"));
        }
        #[derive(serde::Deserialize)]
        struct Envelope {
            networking: NetworkingCase,
        }
        let case = serde_json::from_str::<Envelope>(self.declaration)
            .map_err(|e| error(e.to_string()))?
            .networking;
        case.validate().map_err(error)?;
        validate_configuration(&case)?;
        Ok(case)
    }
}
impl From<core_types::ResourceKind> for Route {
    fn from(r: core_types::ResourceKind) -> Self {
        match r {
            core_types::ResourceKind::Html => Self::Html,
            core_types::ResourceKind::Css => Self::Css,
            core_types::ResourceKind::Image => Self::Image,
        }
    }
}
impl From<Route> for core_types::ResourceKind {
    fn from(r: Route) -> Self {
        match r {
            Route::Html => Self::Html,
            Route::Css => Self::Css,
            Route::Image => Self::Image,
        }
    }
}
impl From<AcquisitionFailure> for net::NetworkErrorKind {
    fn from(e: AcquisitionFailure) -> Self {
        match e {
            AcquisitionFailure::DnsFailure => Self::DnsFailure,
            AcquisitionFailure::ConnectionFailure => Self::ConnectionFailure,
            AcquisitionFailure::TlsFailure => Self::TlsFailure,
            AcquisitionFailure::Timeout => Self::Timeout,
            AcquisitionFailure::Backend => Self::Backend,
        }
    }
}
fn headers(input: &[Header]) -> Result<net::HttpHeaders, HarnessError> {
    let mut out = net::HttpHeaders::new();
    for Header(n, v) in input {
        out.append(n, v).map_err(|e| error(e.to_string()))?;
    }
    Ok(out)
}
fn network(row: &FetchRow, materialize: bool) -> Result<net::fake::FakeNetwork, HarnessError> {
    let mut builder = net::fake::FakeNetworkBuilder::new();
    for route in &row.routes {
        let url = BrowserUrl::parse(&route.url)
            .map_err(|_| error("invalid registered route URL"))?
            .url;
        let method = net::RequestMethod::try_from(route.method.as_str())
            .map_err(|_| error("invalid registered method"))?;
        match &route.outcome {
            RouteOutcome::NetworkError { error: kind } => {
                builder.register_network_error(&url, method, (*kind).into())
            }
            RouteOutcome::Response {
                status,
                headers: h,
                body,
                reported_response_url,
                read_failure_after,
            } => {
                let status = net::HttpStatus::try_from(*status)
                    .map_err(|_| error("invalid fixture status"))?;
                let mut response = net::fake::FakeResponse::new(
                    status,
                    headers(h)?,
                    if materialize {
                        body.materialize()
                    } else {
                        vec![]
                    },
                    reported_response_url.clone(),
                );
                if materialize && let Some(n) = read_failure_after {
                    response = response
                        .with_read_failure_after(*n)
                        .map_err(|e| error(e.to_string()))?;
                }
                builder.register_response(&url, method, response)
            }
        }
        .map_err(|e| error(e.to_string()))?;
    }
    Ok(builder.build())
}
fn validate_configuration(case: &NetworkingCase) -> Result<(), HarnessError> {
    if let NetworkingCase::Fetch { rows } = case {
        for row in rows {
            let _ = headers(&row.request.headers)?;
            let _ = network(row, false)?;
            for u in [
                &row.request.initiator_url,
                &row.request.requesting_origin_url,
            ]
            .into_iter()
            .flatten()
            {
                BrowserUrl::parse(u).map_err(|_| error("invalid issuance context URL"))?;
            }
        }
    }
    Ok(())
}
fn rejected(terminal: Terminal) -> FetchObservation {
    FetchObservation {
        request: Available::Unavailable("request_not_constructed".into()),
        acquisitions: vec![],
        start: Available::Inapplicable,
        terminal,
        selected_final_url: Available::Inapplicable,
        body: Body::Bytes { bytes: vec![] },
    }
}
fn row(row: &FetchRow, expected_storage: usize) -> Result<FetchObservation, Failure> {
    let url = match BrowserUrl::parse(&row.request.url) {
        Ok(p) => p.url,
        Err(e) => {
            return Ok(rejected(Terminal::UrlRejected {
                failure: crate::addressing::observations::failure(&e)?,
                diagnostics: crate::addressing::observations::diagnostics(&e.diagnostics),
            }));
        }
    };
    let method = match net::RequestMethod::try_from(row.request.method.as_str()) {
        Ok(m) => m,
        Err(e) => return Ok(rejected(Terminal::ConstructionRejected { error: e.into() })),
    };
    let context = |u: &Option<String>| -> Result<_, HarnessError> {
        u.as_ref()
            .map(|u| {
                BrowserUrl::parse(u)
                    .map(|p| p.url)
                    .map_err(|_| error("invalid issuance context"))
            })
            .transpose()
    };
    let request = net::FetchRequest::try_new(
        1,
        ResourceAddress::new(url, row.request.purpose.resource_kind()),
        method,
        headers(&row.request.headers)?,
        context(&row.request.initiator_url)?,
        context(&row.request.requesting_origin_url)?.map(|u| u.url_origin()),
    );
    let request = match request {
        Ok(r) => r,
        Err(e) => return Ok(rejected(Terminal::ConstructionRejected { error: e.into() })),
    };
    execute::run(
        network(row, true)?,
        request,
        row.request.route,
        Arc::new(AtomicBool::new(false)),
        expected_storage,
    )
}
pub(crate) fn execute(
    input: NetworkingInput,
    expectation_json: &str,
) -> Result<NetworkingObservation, Failure> {
    let case = input.case()?;
    let expected = expected(expectation_json, &case)?;
    let NetworkingCase::Fetch { rows } = case else {
        return Err(error("deferred networking input cannot execute").into());
    };
    let mut observations = Vec::with_capacity(rows.len());
    let mut storage = expected.storage();
    for (input, expected_row) in rows.into_iter().zip(&expected.rows) {
        let observed = row(&input, expected_row.storage())?;
        observed.validate()?;
        storage = storage
            .checked_add(observed.storage())
            .ok_or_else(|| error("fixture storage overflow"))?;
        if storage > bounds::FIXTURE_STORAGE {
            return Err(error("fixture storage budget exceeded").into());
        }
        observations.push(observed);
    }
    Ok(NetworkingObservation { rows: observations })
}
pub(crate) fn expected(
    json: &str,
    case: &NetworkingCase,
) -> Result<NetworkingObservation, HarnessError> {
    if json.len() > bounds::EXPECTATION {
        return Err(error("expectation byte budget exceeded"));
    }
    let NetworkingCase::Fetch { rows } = case else {
        return Err(error("deferred fixture cannot contain expectations"));
    };
    let value: NetworkingObservation =
        serde_json::from_str(json).map_err(|e| error(e.to_string()))?;
    value.validate(rows.len())?;
    Ok(value)
}
