use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use once_cell::sync::Lazy;

use crate::ErrorKind;
use crate::{
    stream::{ReadWrite, TlsConnector},
    Error,
};

struct RustlsStream(rustls::StreamOwned<rustls::ClientConnection, Box<dyn ReadWrite>>);

impl ReadWrite for RustlsStream {
    fn socket(&self) -> Option<&TcpStream> {
        self.0.get_ref().socket()
    }

    fn is_idle_for_reuse(&mut self) -> io::Result<bool> {
        if self.0.conn.is_handshaking() || self.0.conn.wants_write() {
            return Ok(false);
        }
        let state = self
            .0
            .conn
            .process_new_packets()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if state.plaintext_bytes_to_read() != 0
            || state.peer_has_closed()
            || self.0.conn.is_handshaking()
            || self.0.conn.wants_write()
        {
            return Ok(false);
        }
        match self.0.conn.reader().read(&mut [0]) {
            // An open rustls reader with no plaintext reports WouldBlock.
            // EOF and other errors instead report clean/unclean closure or
            // failure. Retained incomplete encrypted records are legitimate;
            // no private deframer-buffer inspection is needed or performed.
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                self.0.sock.is_idle_for_reuse()
            }
            Ok(_) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl Read for RustlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
}

impl Write for RustlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

#[cfg(feature = "native-certs")]
fn root_certs() -> rustls::RootCertStore {
    use log::error;

    let mut root_cert_store = rustls::RootCertStore::empty();
    let native_certs = rustls_native_certs::load_native_certs().unwrap_or_else(|_| {
        error!("native certificate loading failed");
        vec![]
    });
    let (valid_count, invalid_count) =
        root_cert_store.add_parsable_certificates(native_certs.into_iter().map(|c| c.into()));
    if valid_count == 0 && invalid_count > 0 {
        error!(
            "no valid certificates loaded by rustls-native-certs. all HTTPS requests will fail."
        );
    }
    root_cert_store
}

#[cfg(not(feature = "native-certs"))]
fn root_certs() -> rustls::RootCertStore {
    rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}

impl TlsConnector for Arc<rustls::ClientConfig> {
    fn connect(
        &self,
        dns_name: &str,
        mut io: Box<dyn ReadWrite>,
    ) -> Result<Box<dyn ReadWrite>, Error> {
        let dns_name = if dns_name.starts_with('[') && dns_name.ends_with(']') {
            // rustls doesn't like ipv6 addresses with brackets
            &dns_name[1..dns_name.len() - 1]
        } else {
            dns_name
        };

        let sni = rustls_pki_types::ServerName::try_from(dns_name)
            .map_err(|e| ErrorKind::Dns.msg(format!("parsing '{}'", dns_name)).src(e))?
            .to_owned();

        let mut sess = rustls::ClientConnection::new(self.clone(), sni)
            .map_err(|e| ErrorKind::Io.msg("tls connection creation failed").src(e))?;

        sess.complete_io(&mut io).map_err(|e| {
            ErrorKind::ConnectionFailed
                .msg("tls connection init failed")
                .src(e)
        })?;
        let stream = rustls::StreamOwned::new(sess, io);

        Ok(Box::new(RustlsStream(stream)))
    }
}

pub fn default_tls_config() -> Arc<dyn TlsConnector> {
    static TLS_CONF: Lazy<Arc<dyn TlsConnector>> = Lazy::new(|| {
        // `ureq` prefers to use `*ring*` by default. It unconditionally activates the Rustls
        // feature for this backend, so we know `rustls::crypto::ring::default_provider()` is
        // available. We use `builder_with_provider()` instead of `builder()` here to avoid the
        // risk that the rustls features are ambiguous and no process wide default crypto provider
        // has been set yet.
        let config = rustls::ClientConfig::builder_with_provider(
            rustls::crypto::ring::default_provider().into(),
        )
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .unwrap() // Safety: the *ring* default provider always configures ciphersuites compatible w/ both TLS 1.2 & TLS 1.3
        .with_root_certificates(root_certs())
        .with_no_client_auth();
        Arc::new(Arc::new(config))
    });
    TLS_CONF.clone()
}

impl fmt::Debug for RustlsStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RustlsStream").finish()
    }
}
