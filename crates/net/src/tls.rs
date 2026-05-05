use std::sync::Arc;

use rustls::pki_types::CertificateDer;

use crate::policy::TlsTrustStore;

pub(crate) fn build_rustls_client_config(policy: &TlsTrustStore) -> Arc<rustls::ClientConfig> {
    let root_store = build_root_cert_store(policy);
    // Borrowser standardizes its explicit rustls configuration on the ring
    // provider so the network subsystem does not inherit an accidental backend.
    let config = rustls::ClientConfig::builder_with_provider(
        rustls::crypto::ring::default_provider().into(),
    )
    .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
    .expect("browser TLS config should support TLS 1.2 and 1.3")
    .with_root_certificates(root_store)
    .with_no_client_auth();

    Arc::new(config)
}

fn build_root_cert_store(policy: &TlsTrustStore) -> rustls::RootCertStore {
    let mut store = rustls::RootCertStore::empty();

    match policy {
        TlsTrustStore::NativeRoots => add_native_root_certs(&mut store),
        TlsTrustStore::NativeRootsWithAdditional(extra) => {
            add_native_root_certs(&mut store);
            add_explicit_root_certs(&mut store, extra);
        }
        TlsTrustStore::CustomRoots(roots) => {
            add_explicit_root_certs(&mut store, roots);
        }
    }

    store
}

fn add_native_root_certs(store: &mut rustls::RootCertStore) {
    let result = rustls_native_certs::load_native_certs();
    let (valid, invalid) = store.add_parsable_certificates(result.certs);
    if invalid > 0 {
        eprintln!("[net][tls][native-roots] ignored {invalid} unparsable native certificate(s)");
    }
    for err in result.errors {
        eprintln!("[net][tls][native-roots] failed to load platform root: {err}");
    }
    if valid == 0 {
        eprintln!(
            "[net][tls][native-roots] no valid native root certificates loaded; HTTPS validation will fail"
        );
    }
}

fn add_explicit_root_certs(store: &mut rustls::RootCertStore, roots: &[Vec<u8>]) {
    for der in roots {
        if let Err(err) = store.add(CertificateDer::from(der.clone())) {
            eprintln!("[net][tls][explicit-roots] rejected root certificate: {err}");
        }
    }

    if roots.is_empty() && store.is_empty() {
        eprintln!("[net][tls][explicit-roots] configured with an empty trust store");
    }
}
