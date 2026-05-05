use std::sync::OnceLock;

use crate::{HttpClientPolicy, tls::build_rustls_client_config};

pub(crate) fn agent_for_policy(policy: &HttpClientPolicy) -> ureq::Agent {
    if *policy == HttpClientPolicy::default() {
        default_http_agent().clone()
    } else {
        build_http_agent(policy)
    }
}

fn default_http_agent() -> &'static ureq::Agent {
    static HTTP_AGENT: OnceLock<ureq::Agent> = OnceLock::new();

    HTTP_AGENT.get_or_init(|| build_http_agent(&HttpClientPolicy::default()))
}

fn build_http_agent(policy: &HttpClientPolicy) -> ureq::Agent {
    let tls_config = build_rustls_client_config(&policy.tls);

    ureq::AgentBuilder::new()
        .user_agent(&policy.user_agent)
        .timeout_connect(policy.timeouts.connect)
        .timeout_read(policy.timeouts.read)
        .timeout_write(policy.timeouts.write)
        .redirects(policy.redirects)
        .tls_config(tls_config)
        .build()
}
