//! No logger, arbitrary URL, redirect, proxy, or connection-reuse retry surface.
use crate::{approval::*, identity::*};
use crate::{canonical, orchestrator::*, provider::*};
use serde::Deserialize;
use std::io::Read;
use zeroize::Zeroizing;

const ORIGIN: &str = "https://robot-ws.your-server.de";
const BODY_BYTES: u64 = 16 * 1024 * 1024;
// ureq's default system resolver has no deadline. The worker owns only the
// public endpoint name, never credentials. A timed-out lookup cannot dispatch HTTP.
fn bounded_resolution(
    lookup: impl FnOnce() -> std::io::Result<Vec<std::net::SocketAddr>> + Send + 'static,
    timeout: std::time::Duration,
) -> std::io::Result<Vec<std::net::SocketAddr>> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("robot-dns".into())
        .spawn(move || {
            let _ = send.send(lookup());
        })?;
    receive
        .recv_timeout(timeout)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::TimedOut))?
}
fn resolve_host(netloc: &str) -> std::io::Result<Vec<std::net::SocketAddr>> {
    use std::net::ToSocketAddrs;
    let netloc = netloc.to_owned();
    bounded_resolution(
        move || {
            let addresses: Vec<_> = netloc.to_socket_addrs()?.take(17).collect();
            if addresses.len() > 16 {
                return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
            }
            Ok(addresses)
        },
        std::time::Duration::from_secs(5),
    )
}
fn transport_failure(kind: ureq::ErrorKind) -> ProviderFailure {
    // Audited against the pinned ureq 2.12.1 fresh-connection/TLS paths: these
    // errors precede handing a stream back for HTTP writes. No proxy/redirect/reuse.
    match kind {
        ureq::ErrorKind::Dns | ureq::ErrorKind::ConnectionFailed => {
            ProviderFailure::DefinitelyNotTransmitted
        }
        _ => ProviderFailure::TransmissionUncertain,
    }
}
pub(crate) struct RobotHttp {
    credentials: crate::linux::Credentials,
    approval: Option<ProductApproval>,
    account: AccountScopeId,
    authority: AuthorityId,
    #[cfg(test)]
    origin: Option<String>,
}
impl RobotHttp {
    pub(crate) fn new(
        credentials: crate::linux::Credentials,
        approval: ProductApproval,
        authority: AuthorityId,
    ) -> Self {
        Self {
            credentials,
            account: approval.account_scope.clone(),
            approval: Some(approval),
            authority,
            #[cfg(test)]
            origin: None,
        }
    }
    pub(crate) fn recovery(
        credentials: crate::linux::Credentials,
        account: AccountScopeId,
        authority: AuthorityId,
    ) -> Self {
        Self {
            credentials,
            approval: None,
            account,
            authority,
            #[cfg(test)]
            origin: None,
        }
    }
    fn check_dispatch(
        &self,
        binding: &crate::mutation::DispatchBinding,
        kind: crate::mutation::MutationKind,
    ) -> ProviderResult<()> {
        if binding.kind != kind
            || binding.account != self.account
            || binding.authority != self.authority
            || binding.descriptor.fingerprint().ok().as_ref() != Some(&binding.descriptor_sha256)
        {
            return Err(ProviderFailure::DefinitelyNotTransmitted);
        }
        Ok(())
    }
    fn call<T: serde::de::DeserializeOwned>(
        &mut self,
        path: &str,
        body: Option<&str>,
    ) -> ProviderResult<T> {
        self.exchange(if body.is_some() { "POST" } else { "GET" }, path, body)
    }
    fn dispatch<T: serde::de::DeserializeOwned>(
        &mut self,
        descriptor: &crate::mutation::MutationDescriptor,
    ) -> ProviderResult<T> {
        self.exchange(
            descriptor.method(),
            descriptor.endpoint(),
            Some(descriptor.body()),
        )
    }
    fn exchange<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> ProviderResult<T> {
        // ureq 2.12.1 retry branches require a recycled connection. A fresh agent
        // and a disabled pool rule out both its early-write and response retries.
        let agent = ureq::AgentBuilder::new()
            .resolver(resolve_host)
            .redirects(0)
            .try_proxy_from_env(false)
            .max_idle_connections(0)
            .max_idle_connections_per_host(0)
            .timeout_connect(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build();
        #[cfg(test)]
        let origin = self.origin.as_deref().unwrap_or(ORIGIN);
        #[cfg(not(test))]
        let origin = ORIGIN;
        let url = format!("{origin}{path}");
        let request = agent
            .request(method, &url)
            .set("Authorization", self.credentials.basic.as_str())
            .set("Accept", "application/json")
            .set("Accept-Encoding", "identity");
        let response = match body {
            Some(b) => request
                .set("Content-Type", "application/x-www-form-urlencoded")
                .send_string(b),
            None => request.call(),
        };
        let response = match response {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(ureq::Error::Transport(e)) => return Err(transport_failure(e.kind())),
        };
        let status = response.status();
        if status == 401 {
            return Err(ProviderFailure::Authentication);
        }
        let mut bytes = Zeroizing::new(Vec::new());
        response
            .into_reader()
            .take(BODY_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ProviderFailure::TransmissionUncertain)?;
        if bytes.len() as u64 > BODY_BYTES {
            return Err(ProviderFailure::Oversized);
        }
        if status == 503 {
            return Err(ProviderFailure::Maintenance);
        }
        // Reject credential echoes even when JSON uses escaped Unicode. This
        // tree is never persisted; DTO deserialization below still detects
        // duplicate known fields in the original bytes.
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| ProviderFailure::Malformed)?;
        if !self.credentials.response_safe(&value, 0) {
            return Err(ProviderFailure::Malformed);
        }
        // Bodies remain memory-only. Provider errors are mapped without formatting.
        if status == 403 {
            #[derive(Deserialize)]
            struct Wrapper {
                error: Rate,
            }
            #[derive(Deserialize)]
            struct Rate {
                code: String,
                max_request: u64,
                interval: u64,
            }
            let r: Wrapper =
                serde_json::from_slice(&bytes).map_err(|_| ProviderFailure::Malformed)?;
            if r.error.code == "RATE_LIMIT_EXCEEDED"
                && r.error.max_request > 0
                && r.error.interval > 0
            {
                return Err(ProviderFailure::RateLimited {
                    max_request: r.error.max_request,
                    interval_seconds: r.error.interval,
                });
            }
            return Err(ProviderFailure::Malformed);
        }
        if status == 409 {
            return Err(ProviderFailure::Conflict);
        }
        if status == 400 || status == 412 {
            #[derive(Deserialize)]
            struct Wrapper {
                error: Rejection,
            }
            #[derive(Deserialize)]
            struct Rejection {
                code: String,
            }
            let r: Wrapper =
                serde_json::from_slice(&bytes).map_err(|_| ProviderFailure::Malformed)?;
            if matches!(
                r.error.code.as_str(),
                "INVALID_INPUT" | "PRECONDITION_FAILED"
            ) {
                return Err(ProviderFailure::Rejected);
            }
        }
        if status == 404 && body.is_none() {
            #[derive(Deserialize)]
            struct MissingWrapper {
                error: Missing,
            }
            #[derive(Deserialize)]
            struct Missing {
                code: String,
            }
            let missing: MissingWrapper =
                serde_json::from_slice(&bytes).map_err(|_| ProviderFailure::Malformed)?;
            if (path == "/order/server/transaction" && missing.error.code == "NOT_FOUND")
                || (path == "/server" && missing.error.code == "SERVER_NOT_FOUND")
            {
                return serde_json::from_slice(b"[]").map_err(|_| ProviderFailure::Malformed);
            }
        }
        if status != 200 && status != 201 {
            return Err(ProviderFailure::TransmissionUncertain);
        }
        serde_json::from_slice(&bytes).map_err(|_| ProviderFailure::Malformed)
    }
}
#[derive(Deserialize)]
struct TransactionWrapper {
    transaction: Box<serde_json::value::RawValue>,
}
impl TransactionWrapper {
    fn normalized(self) -> TransactionResponse {
        #[derive(Deserialize)]
        struct Identity {
            id: RobotTransactionId,
        }
        let identity: Identity = match serde_json::from_str(self.transaction.get()) {
            Ok(id) => id,
            Err(_) => return TransactionResponse::NoIdentity(ProviderFailure::Malformed),
        };
        match serde_json::from_str::<WireTransaction>(self.transaction.get())
            .ok()
            .and_then(|t| t.normalized().ok())
        {
            Some(t) => TransactionResponse::Normalized(t),
            None => TransactionResponse::IdentityOnly {
                id: identity.id,
                failure: ProviderFailure::Malformed,
            },
        }
    }
}
#[derive(Deserialize)]
struct WireTransaction {
    id: RobotTransactionId,
    date: String,
    status: TransactionStatus,
    server_number: Option<ServerNumber>,
    product: WireProduct,
    addons: Vec<String>,
}
#[derive(Deserialize)]
struct WireProduct {
    id: ProductId,
    location: Option<String>,
}
impl WireTransaction {
    fn normalized(self) -> ProviderResult<Transaction> {
        let mut addons = self.addons;
        addons.sort();
        let t = Transaction {
            id: self.id,
            date: self.date,
            status: self.status,
            server_number: self.server_number,
            product_id: self.product.id,
            location: self.product.location,
            addons,
        };
        t.validate().map_err(|_| ProviderFailure::Malformed)?;
        Ok(t)
    }
}
#[derive(Deserialize)]
struct ServerWrapper {
    server: WireServer,
}
#[derive(Deserialize)]
struct WireServer {
    server_number: ServerNumber,
    product: String,
    dc: String,
    status: ServerStatus,
    cancelled: bool,
}
impl WireServer {
    fn normalized(self) -> ProviderResult<ServerObservation> {
        if self.server_number.get() == 0
            || canonical::text(&self.product, 1024).is_err()
            || canonical::text(&self.dc, 128).is_err()
        {
            return Err(ProviderFailure::Malformed);
        }
        Ok(ServerObservation {
            number: self.server_number,
            product: self.product,
            datacenter: self.dc,
            status: self.status,
            cancelled: self.cancelled,
        })
    }
}
#[derive(Deserialize)]
struct CancellationWrapper {
    cancellation: WireCancellation,
}
#[derive(Deserialize)]
struct WireCancellation {
    server_number: ServerNumber,
    cancelled: bool,
    reservation_possible: bool,
    reserved: bool,
    cancellation_date: Option<String>,
}
impl WireCancellation {
    fn normalized(self) -> CancellationObservation {
        CancellationObservation {
            server_number: self.server_number,
            cancelled: self.cancelled,
            reservation_possible: self.reservation_possible,
            reserved: self.reserved,
            cancellation_date: self.cancellation_date,
        }
    }
}
impl RobotReader for RobotHttp {
    fn catalogue(&mut self, request: &AllocationRequest) -> ProviderResult<CatalogueQuote> {
        let approval = self.approval.clone().ok_or(ProviderFailure::Rejected)?;
        #[derive(Deserialize, serde::Serialize)]
        struct Product {
            id: ProductId,
            name: String,
            description: Vec<String>,
            location: Vec<String>,
            prices: Vec<Price>,
            orderable_addons: Vec<Addon>,
        }
        #[derive(Deserialize, serde::Serialize)]
        struct Price {
            location: String,
            price: Money,
            price_setup: Money,
        }
        #[derive(Deserialize, serde::Serialize)]
        struct Money {
            gross: String,
        }
        #[derive(Deserialize, serde::Serialize)]
        struct Addon {
            id: String,
            min: u64,
            max: u64,
            location: Option<String>,
            prices: Vec<Price>,
        }
        #[derive(Deserialize)]
        struct Wrapper {
            product: Product,
        }
        let products: Vec<Wrapper> = self.call("/order/server/product", None)?;
        if products.len() > 1024 {
            return Err(ProviderFailure::Oversized);
        }
        let matches: Vec<_> = products
            .into_iter()
            .filter(|p| p.product.id == request.product_id)
            .collect();
        if matches.len() != 1 {
            return Err(ProviderFailure::Malformed);
        }
        let p = &matches[0].product;
        let price = |prices: &[Price]| -> ProviderResult<(u64, u64)> {
            let prices: Vec<_> = prices.iter().filter(|p| p.location == "FSN1").collect();
            if prices.len() != 1 {
                return Err(ProviderFailure::Malformed);
            }
            let p = prices[0];
            Ok((
                euro_units(&p.price.gross).map_err(|_| ProviderFailure::Malformed)?,
                euro_units(&p.price_setup.gross).map_err(|_| ProviderFailure::Malformed)?,
            ))
        };
        let addons: Vec<_> = p
            .orderable_addons
            .iter()
            .filter(|a| a.id == "primary_ipv4")
            .collect();
        if addons.len() != 1 {
            return Err(ProviderFailure::Malformed);
        }
        let (monthly, setup) = price(&p.prices)?;
        let (ipv4_monthly, ipv4_setup) = price(&addons[0].prices)?;
        let live_identity = CatalogueIdentity {
            product_id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone(),
        };
        let mut available_locations = p.location.clone();
        available_locations.sort();
        let ipv4 = Ipv4Capability {
            id: addons[0].id.clone(),
            minimum: addons[0].min,
            maximum: addons[0].max,
            location: addons[0].location.clone(),
            price_location: "FSN1".into(),
        };
        let quote = CatalogueQuote {
            approval: approval.clone(),
            catalogue_evidence_sha256: approval.digest().map_err(|_| ProviderFailure::Rejected)?,
            live_identity,
            available_locations,
            ipv4,
            product_id: p.id.clone(),
            name: p.name.clone(),
            location: "FSN1".into(),
            monthly_gross_units: monthly
                .checked_add(ipv4_monthly)
                .ok_or(ProviderFailure::Malformed)?,
            setup_gross_units: setup
                .checked_add(ipv4_setup)
                .ok_or(ProviderFailure::Malformed)?,
            approved_monthly_gross_units: approval.monthly_gross_ceiling,
            approved_setup_gross_units: approval.setup_gross_ceiling,
        };
        quote.validate().map_err(|_| ProviderFailure::Rejected)?;
        Ok(quote)
    }
    fn history(&mut self) -> ProviderResult<Vec<TransactionResponse>> {
        let list: Vec<TransactionWrapper> = self.call("/order/server/transaction", None)?;
        if list.len() > 1024 {
            return Err(ProviderFailure::Oversized);
        }
        Ok(list
            .into_iter()
            .map(TransactionWrapper::normalized)
            .collect())
    }
    fn servers(&mut self) -> ProviderResult<Vec<ServerObservation>> {
        let list: Vec<ServerWrapper> = self.call("/server", None)?;
        if list.len() > 256 {
            return Err(ProviderFailure::Oversized);
        }
        list.into_iter().map(|w| w.server.normalized()).collect()
    }
    fn transaction(&mut self, id: &RobotTransactionId) -> ProviderResult<TransactionResponse> {
        if !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(ProviderFailure::Malformed);
        }
        Ok(self
            .call::<TransactionWrapper>(&format!("/order/server/transaction/{id}"), None)?
            .normalized())
    }
    fn server(&mut self, n: ServerNumber) -> ProviderResult<ServerObservation> {
        self.call::<ServerWrapper>(&format!("/server/{n}"), None)?
            .server
            .normalized()
    }
    fn cancellation(&mut self, n: ServerNumber) -> ProviderResult<CancellationObservation> {
        Ok(self
            .call::<CancellationWrapper>(&format!("/server/{n}/cancellation"), None)?
            .cancellation
            .normalized())
    }
}
impl RobotMutator for RobotHttp {
    fn allocate(
        &mut self,
        dispatch: DurableAllocationDispatch,
    ) -> ProviderResult<TransactionResponse> {
        if !dispatch
            .deadline()
            .permits(&crate::linux::now().map_err(|_| ProviderFailure::DefinitelyNotTransmitted)?)
        {
            return Err(ProviderFailure::DefinitelyNotTransmitted);
        }
        let binding = dispatch.binding();
        self.check_dispatch(binding, crate::mutation::MutationKind::Allocation)?;
        Ok(self
            .dispatch::<TransactionWrapper>(&binding.descriptor)?
            .normalized())
    }
    fn cancel(
        &mut self,
        dispatch: DurableCancellationDispatch,
    ) -> ProviderResult<CancellationObservation> {
        let binding = dispatch.binding();
        self.check_dispatch(binding, crate::mutation::MutationKind::Cancellation)?;
        Ok(self
            .dispatch::<CancellationWrapper>(&binding.descriptor)?
            .cancellation
            .normalized())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener};
    fn fixture(response: &str, run: impl FnOnce(&mut RobotHttp)) -> String {
        let response = response.to_owned();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let child = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut b = [0u8; 1];
            while !bytes.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut b).unwrap();
                bytes.push(b[0]);
                assert!(bytes.len() < 16384);
            }
            let headers = String::from_utf8(bytes.clone()).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            bytes.extend(body);
            if !response.is_empty() {
                stream.write_all(response.as_bytes()).unwrap();
            }
            drop(stream);
            listener.set_nonblocking(true).unwrap();
            // A second attempted connection would remain in the backlog even if
            // the client returned after an error; no second request is serviced.
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(listener.accept().is_err(), "unexpected transport retry");
            String::from_utf8(bytes).unwrap()
        });
        let mut client = RobotHttp::new(
            crate::linux::Credentials::synthetic(),
            crate::orchestrator::command_tests::approval("reviewed-id", "a", "synthetic AX42-1"),
            "c".parse().unwrap(),
        );
        client.origin = Some(format!("http://{address}"));
        run(&mut client);
        child.join().unwrap()
    }
    #[test]
    fn dns_timeout_cannot_dispatch_and_transport_progress_is_conservative() {
        let (release, wait) = std::sync::mpsc::channel();
        let result = bounded_resolution(
            move || {
                wait.recv().unwrap();
                Ok(vec![])
            },
            std::time::Duration::from_millis(1),
        );
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::TimedOut);
        release.send(()).unwrap();
        assert_eq!(
            transport_failure(ureq::ErrorKind::Dns),
            ProviderFailure::DefinitelyNotTransmitted
        );
        assert_eq!(
            transport_failure(ureq::ErrorKind::ConnectionFailed),
            ProviderFailure::DefinitelyNotTransmitted
        );
        assert_eq!(
            transport_failure(ureq::ErrorKind::Io),
            ProviderFailure::TransmissionUncertain
        );
    }
    #[test]
    fn actual_transport_does_not_resend_after_lost_mutation_response() {
        let request = AllocationRequest {
            product_id: "reviewed-id".parse().unwrap(),
            location: "FSN1".into(),
            addons: vec!["primary_ipv4".into()],
        };
        let sent = fixture("", |c| {
            assert_eq!(
                c.allocate(synthetic_allocation_dispatch(&request)),
                Err(ProviderFailure::TransmissionUncertain)
            )
        });
        assert!(sent.starts_with("POST /order/server/transaction HTTP/1.1\r\n"));
        assert!(sent.ends_with("product_id=reviewed-id&location=FSN1&addon%5B%5D=primary_ipv4"));
    }
    #[test]
    fn redirects_are_not_followed() {
        fixture(
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/steal\r\nContent-Length: 2\r\n\r\n{}",
            |c| assert!(c.history().is_err()),
        );
    }
    #[test]
    fn maintenance_can_be_non_json_and_authentication_never_needs_body() {
        fixture(
            "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 4\r\n\r\ndown",
            |c| assert_eq!(c.history(), Err(ProviderFailure::Maintenance)),
        );
        fixture(
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
            |c| assert_eq!(c.history(), Err(ProviderFailure::Authentication)),
        );
    }
    #[test]
    fn credential_echo_with_unicode_escape_is_rejected() {
        let c = crate::linux::Credentials::synthetic();
        let v: serde_json::Value = serde_json::from_str(r#"{"x":"synthetic-\u0070ass"}"#).unwrap();
        assert!(!c.response_safe(&v, 0));
        assert_eq!(
            format!("{}", crate::Error("provider response rejected")),
            "provider response rejected"
        );
    }
    #[test]
    fn cancellation_wire_conflict_and_weak_absence_remain_incomplete() {
        let sent = fixture(
            "HTTP/1.1 409 Conflict\r\nContent-Length: 2\r\n\r\n{}",
            |c| {
                assert_eq!(
                    c.cancel(synthetic_cancellation_dispatch(123.try_into().unwrap())),
                    Err(ProviderFailure::Conflict)
                );
            },
        );
        assert!(sent.starts_with("POST /server/123/cancellation HTTP/1.1\r\n"));
        assert!(sent.ends_with("cancellation_date=now&reserve_location=false"));
        for _ in 0..2 {
            let body = r#"{"error":{"code":"SERVER_NOT_FOUND"}}"#;
            fixture(
                &format!(
                    "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
                |c| {
                    assert_eq!(
                        c.cancellation(123.try_into().unwrap()),
                        Err(ProviderFailure::TransmissionUncertain)
                    );
                },
            );
        }
    }
    #[test]
    fn semantic_cancellation_readback_never_sends_an_unreviewed_second_post() {
        for (cancelled, date, reserved) in [
            (true, "null", false),
            (false, "\"2026-09-20\"", false),
            (false, "null", true),
            (true, "\"2026-09-20\"", false),
            (false, "null", false),
        ] {
            let body = format!(
                r#"{{"cancellation":{{"server_number":123,"cancelled":{cancelled},"reservation_possible":true,"reserved":{reserved},"cancellation_date":{date}}}}}"#
            );
            let sent = fixture(
                &format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
                |client| {
                    crate::orchestrator::command_tests::assert_no_http_cancellation_retry(client);
                },
            );
            assert!(sent.starts_with("GET /server/123/cancellation HTTP/1.1\r\n"));
        }
    }
    #[test]
    fn malformed_oversized_and_rate_observations_are_typed() {
        fixture("HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n{", |c| {
            assert_eq!(c.history(), Err(ProviderFailure::Malformed));
        });
        let body = "x".repeat(BODY_BYTES as usize + 1);
        fixture(
            &format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
            |c| {
                assert_eq!(c.history(), Err(ProviderFailure::Oversized));
            },
        );
        let body = r#"{"error":{"code":"RATE_LIMIT_EXCEEDED","max_request":2,"interval":3600}}"#;
        fixture(
            &format!(
                "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
            |c| {
                assert_eq!(
                    c.history(),
                    Err(ProviderFailure::RateLimited {
                        max_request: 2,
                        interval_seconds: 3600
                    })
                );
            },
        );
    }
    #[test]
    fn transaction_identity_parsing_rejects_ambiguity_but_retains_supported_ids() {
        let parse = |body: &str| {
            serde_json::from_str::<TransactionWrapper>(body)
                .map(TransactionWrapper::normalized)
                .unwrap_or(TransactionResponse::NoIdentity(ProviderFailure::Malformed))
        };
        for body in [
            r#"{"transaction":{"id":"B123","status":"future-status","product":{"id":"reviewed-id"}}}"#,
            r#"{"transaction":{"id":"B123","status":"ready","product":17}}"#,
            r#"{"transaction":{"id":"B123","status":"ready","product":{"id":"bad&product"}}}"#,
        ] {
            assert_eq!(
                parse(body),
                TransactionResponse::IdentityOnly {
                    id: "B123".parse().unwrap(),
                    failure: ProviderFailure::Malformed
                }
            );
        }
        for body in [
            r#"{"transaction":{"id":"B123","id":"B456"}}"#,
            r#"{"transaction":{"id":"B123","id":"B123"}}"#,
            r#"{"transaction":{"id":"B123","i\u0064":"B123"}}"#,
            r#"{"transaction":{"id":"B123"},"transaction":{"id":"B456"}}"#,
            r#"{"transaction":{"status":"ready"}}"#,
            r#"{"transaction":{"id":"invalid/path"}}"#,
            r#"{"transaction":{"id":"B123"}"#,
        ] {
            assert!(matches!(parse(body), TransactionResponse::NoIdentity(_)));
        }
        let valid = r#"{"transaction":{"id":"B123","date":"2026-09-19","status":"ready","server_number":123,"product":{"id":"reviewed-id","location":"FSN1"},"addons":["primary_ipv4"]}}"#;
        assert!(matches!(parse(valid), TransactionResponse::Normalized(_)));
    }
    #[test]
    fn wire_catalogue_validates_reviewed_identity_and_ipv4_orderability() {
        // JSON fixture construction is test-only; production identity extraction never uses Value inference.
        let price = serde_json::json!({"location":"FSN1","price":{"gross":"0.0000"},"price_setup":{"gross":"0.0000"}});
        let good = serde_json::json!([{"product":{"id":"reviewed-id","name":"synthetic AX42-1","description":["synthetic reviewed hardware"],"location":["FSN1"],"prices":[price.clone()],"orderable_addons":[{"id":"primary_ipv4","min":0,"max":1,"prices":[price]}]}}]);
        for case in 0..6 {
            let mut value = good.clone();
            match case {
                1 => {
                    value[0]["product"]["description"] = serde_json::json!(["different processor"])
                }
                2 => value[0]["product"]["orderable_addons"] = serde_json::json!([]),
                3 => value[0]["product"]["orderable_addons"][0]["min"] = serde_json::json!(2),
                4 => value[0]["product"]["orderable_addons"][0]["max"] = serde_json::json!(0),
                5 => {
                    value[0]["product"]["orderable_addons"][0]["location"] =
                        serde_json::json!("NBG1")
                }
                _ => {}
            }
            let body = serde_json::to_string(&value).unwrap();
            fixture(
                &format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
                |c| {
                    let result = c.catalogue(&AllocationRequest {
                        product_id: "reviewed-id".parse().unwrap(),
                        location: "FSN1".into(),
                        addons: vec!["primary_ipv4".into()],
                    });
                    assert_eq!(result.is_ok(), case == 0);
                },
            );
        }
    }
    #[test]
    fn account_or_authority_mismatch_cannot_use_a_dispatch_capability() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let request = AllocationRequest {
            product_id: "reviewed-id".parse().unwrap(),
            location: "FSN1".into(),
            addons: vec!["primary_ipv4".into()],
        };
        for (account, authority) in [("wrong-account", "c"), ("a", "wrong-authority")] {
            let mut c = RobotHttp::recovery(
                crate::linux::Credentials::synthetic(),
                account.parse().unwrap(),
                authority.parse().unwrap(),
            );
            c.origin = Some(format!("http://{}", listener.local_addr().unwrap()));
            assert_eq!(
                c.allocate(synthetic_allocation_dispatch(&request)),
                Err(ProviderFailure::DefinitelyNotTransmitted)
            );
            assert_eq!(
                c.cancel(synthetic_cancellation_dispatch(123.try_into().unwrap())),
                Err(ProviderFailure::DefinitelyNotTransmitted)
            );
            assert!(listener.accept().is_err());
        }
    }
    #[test]
    fn expired_allocation_capability_cannot_transmit() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut c = RobotHttp::recovery(
            crate::linux::Credentials::synthetic(),
            "a".parse().unwrap(),
            "c".parse().unwrap(),
        );
        c.origin = Some(format!("http://{}", listener.local_addr().unwrap()));
        let request = AllocationRequest {
            product_id: "reviewed-id".parse().unwrap(),
            location: "FSN1".into(),
            addons: vec!["primary_ipv4".into()],
        };
        assert_eq!(
            c.allocate(synthetic_expired_allocation_dispatch(&request)),
            Err(ProviderFailure::DefinitelyNotTransmitted)
        );
        assert!(listener.accept().is_err());
    }
    #[test]
    fn unsupported_server_status_is_not_corroboration() {
        let body = r#"{"server":{"server_number":123,"product":"AX42-1","dc":"FSN1-DC1","status":"future-status","cancelled":false}}"#;
        fixture(
            &format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            ),
            |c| {
                assert!(matches!(
                    c.server(123.try_into().unwrap()),
                    Err(ProviderFailure::Malformed)
                ));
            },
        );
    }
    #[test]
    #[ignore = "explicit opt-in; requires controller credentials and BORROWSER_ROBOT_INTEGRATION=read-only"]
    fn provider_read_only() {
        assert_eq!(
            std::env::var("BORROWSER_ROBOT_INTEGRATION").as_deref(),
            Ok("read-only")
        );
        let d = crate::linux::deployment().unwrap();
        let mut c = RobotHttp::new(
            crate::linux::Credentials::load().unwrap(),
            crate::linux::product_approval(&d).unwrap(),
            d.authority_id,
        );
        c.history().unwrap();
    }
    #[test]
    #[ignore = "explicit opt-in; requires controller configuration and BORROWSER_ROBOT_INTEGRATION=test-order"]
    fn provider_nonprocessing_order_validation() {
        assert_eq!(
            std::env::var("BORROWSER_ROBOT_INTEGRATION").as_deref(),
            Ok("test-order")
        );
        let d = crate::linux::deployment().unwrap();
        let r = AllocationRequest {
            product_id: d.product_id.clone(),
            location: "FSN1".into(),
            addons: vec!["primary_ipv4".into()],
        };
        let mut c = RobotHttp::new(
            crate::linux::Credentials::load().unwrap(),
            crate::linux::product_approval(&d).unwrap(),
            d.authority_id.clone(),
        );
        c.catalogue(&r).unwrap();
        let form = format!("{}&test=true", r.form().unwrap());
        assert!(form.ends_with("&test=true"));
        // Separate test-only call; never falls back to RobotMutator::allocate.
        let _: TransactionWrapper = c.call("/order/server/transaction", Some(&form)).unwrap();
    }
}
