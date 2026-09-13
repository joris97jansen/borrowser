//! Pure checks shared by the kernel adapter and deterministic negative tests.
use crate::{CaptureError as E, Result};

pub(crate) fn namespace_binding(
    parent: u64,
    isolated: u64,
    expected_owner: u64,
    actual_owner: u64,
    inner_pid: &str,
) -> Result<()> {
    if parent == isolated || expected_owner != actual_owner || inner_pid != "1" {
        Err(E::Isolation)
    } else {
        Ok(())
    }
}
/// Four exact kernel capability masks, never a boolean "has capabilities".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Capabilities {
    effective: u64,
    permitted: u64,
    inheritable: u64,
    ambient: u64,
}
const SYS_ADMIN: u64 = 1 << 21;
impl Capabilities {
    const ZERO: Self = Self {
        effective: 0,
        permitted: 0,
        inheritable: 0,
        ambient: 0,
    };
    const ZYGOTE: Self = Self {
        effective: SYS_ADMIN,
        permitted: SYS_ADMIN,
        inheritable: 0,
        ambient: 0,
    };
    pub(crate) fn parse(status: &str) -> Result<Self> {
        if status.len() > 65536 {
            return Err(E::Limit);
        }
        let mut masks = [0; 4];
        for (index, field) in ["CapEff", "CapPrm", "CapInh", "CapAmb"].iter().enumerate() {
            let mut values = status
                .lines()
                .filter_map(|line| line.split_once(':'))
                .filter_map(|(key, value)| (key == *field).then_some(value));
            let value = values
                .next()
                .ok_or(E::Sandbox)?
                .strip_prefix('\t')
                .ok_or(E::Sandbox)?;
            if values.next().is_some()
                || value.len() != 16
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(E::Sandbox);
            }
            masks[index] = u64::from_str_radix(value, 16).map_err(|_| E::Sandbox)?;
        }
        Ok(Self {
            effective: masks[0],
            permitted: masks[1],
            inheritable: masks[2],
            ambient: masks[3],
        })
    }
}
pub(crate) fn cleared_capabilities(status: &str) -> Result<()> {
    if Capabilities::parse(status)? != Capabilities::ZERO {
        return Err(E::Sandbox);
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChromiumRole {
    Main,
    Renderer,
    NamespaceZygote,
    UnsandboxedZygote,
    Gpu,
    NetworkServiceUtility,
    NetworkServiceBroker,
    GpuBroker,
}
/// Attribution from Chromium's rewritten Linux argv-memory display title.
/// This is deliberately not an invocation argument vector or executable path.
#[derive(Debug)]
pub(crate) struct ChromiumProcessTitle<'a>(&'a str);
impl<'a> ChromiumProcessTitle<'a> {
    pub(crate) fn parse(observable: &'a str) -> Result<Self> {
        if observable.len() > 65536 {
            return Err(E::Limit);
        }
        if !observable.ends_with('\0') {
            return Err(E::Sandbox);
        }
        let title = observable.trim_end_matches('\0');
        if title.is_empty()
            || title
                .bytes()
                .any(|b| !(0x20..=0x7e).contains(&b) || matches!(b, b'\'' | b'"' | b'\\'))
        {
            return Err(E::Sandbox);
        }
        let mut tokens = title.split(' ');
        let label = tokens.next().ok_or(E::Sandbox)?;
        if label.is_empty() || label.starts_with('-') || label.len() > 4096 {
            return Err(E::Sandbox);
        }
        for (index, token) in tokens.enumerate() {
            if index >= 256 || token.len() > 4096 || !token.starts_with("--") || token == "--" {
                return Err(E::Sandbox);
            }
        }
        Ok(Self(title))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreliminaryRole {
    Process(ChromiumRole),
    BrokerCandidate,
}
/// Child-only attribution: no title can yield Main.
pub(crate) fn chromium_child_role(title: &ChromiumProcessTitle<'_>) -> Result<PreliminaryRole> {
    let mut process_type = None;
    let mut unsandboxed_zygote = false;
    let mut utility_subtype = None;
    let mut service_sandbox = None;
    let mut count = 0;
    let mut tokens = title.0.split(' ');
    let display_label = tokens.next().ok_or(E::Sandbox)?;
    if display_label.is_empty() || display_label.len() > 4096 {
        return Err(E::Sandbox);
    }
    for arg in tokens {
        count += 1;
        if count > 256 || arg.len() > 4096 || arg.is_empty() {
            return Err(E::Sandbox);
        }
        let key = arg.split('=').next().ok_or(E::Sandbox)?;
        if matches!(
            key,
            "--no-sandbox"
                | "--disable-namespace-sandbox"
                | "--disable-seccomp-filter-sandbox"
                | "--disable-gpu-sandbox"
                | "--disable-setuid-sandbox"
                | "--no-zygote"
                | "--no-unsandboxed-zygote"
                | "--single-process"
                | "--in-process-gpu"
                | "--allow-sandbox-debugging"
                | "--gpu-sandbox-failures-fatal"
        ) {
            return Err(E::Sandbox);
        }
        if key == "--no-zygote-sandbox" {
            if arg != key || unsandboxed_zygote {
                return Err(E::Sandbox);
            }
            unsandboxed_zygote = true;
        }
        for (switch, slot) in [
            ("--utility-sub-type", &mut utility_subtype),
            ("--service-sandbox-type", &mut service_sandbox),
        ] {
            if key == switch {
                let (_, value) = arg.split_once('=').ok_or(E::Sandbox)?;
                if value.is_empty() || slot.replace(value).is_some() {
                    return Err(E::Sandbox);
                }
            }
        }
        if arg == "--type" {
            return Err(E::Sandbox);
        }
        if let Some(value) = arg.strip_prefix("--type=")
            && process_type.replace(value).is_some()
        {
            return Err(E::Sandbox);
        }
    }
    if unsandboxed_zygote && process_type != Some("zygote") {
        return Err(E::Sandbox);
    }
    if (utility_subtype.is_some() || service_sandbox.is_some())
        && !matches!(process_type, Some("utility"))
    {
        return Err(E::Sandbox);
    }
    let role = match process_type {
        Some("renderer") => ChromiumRole::Renderer,
        Some("zygote") if unsandboxed_zygote => ChromiumRole::UnsandboxedZygote,
        Some("zygote") => ChromiumRole::NamespaceZygote,
        Some("gpu-process") => ChromiumRole::Gpu,
        Some("broker") if count == 1 => return Ok(PreliminaryRole::BrokerCandidate),
        Some("utility")
            if utility_subtype == Some("network.mojom.NetworkService")
                && service_sandbox == Some("network") =>
        {
            ChromiumRole::NetworkServiceUtility
        }
        _ => return Err(E::Sandbox),
    };
    Ok(PreliminaryRole::Process(role))
}
/// The caller supplies the launch-owned PID, never a title-derived identity.
/// Deferred child reads ensure Main has no proc-title parsing dependency.
pub(crate) fn attribute_role(
    pid: i32,
    launched_main_pid: i32,
    read_child_title: impl FnOnce() -> Result<String>,
) -> Result<PreliminaryRole> {
    if pid <= 0 || launched_main_pid <= 0 {
        return Err(E::ProcessIdentity);
    }
    if pid == launched_main_pid {
        return Ok(PreliminaryRole::Process(ChromiumRole::Main));
    }
    let observable = read_child_title()?;
    chromium_child_role(&ChromiumProcessTitle::parse(&observable)?)
}
/// Compared only after retained pidfd/start/stopped-object validation. Namespace
/// IDs come from retained kernel objects, not strings supplied by the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MainIdentity {
    pub pid: i32,
    pub executable: (u64, u64),
    pub namespaces: [u64; 3], // user, PID, network, all exactly outer
}
pub(crate) fn verify_main(
    actual: MainIdentity,
    expected: MainIdentity,
    status: &str,
) -> Result<()> {
    if actual.pid <= 0 || actual.pid != expected.pid || actual.executable != expected.executable {
        return Err(E::ProcessIdentity);
    }
    if actual.namespaces != expected.namespaces {
        return Err(E::Isolation);
    }
    cleared_capabilities(status)
}
/// Ancestry is checked from retained namespace objects BEFORE this pure rule.
/// Distinct namespace IDs alone never establish ancestry.
pub(crate) fn process_capabilities(
    role: ChromiumRole,
    status: &str,
    strict_user_descendant: bool,
    strict_pid_descendant: bool,
) -> Result<()> {
    let expected = if role == ChromiumRole::NamespaceZygote {
        let mut pids = status.lines().filter_map(|l| l.strip_prefix("NSpid:\t"));
        let pids_value = pids.next().ok_or(E::Sandbox)?;
        let mut count = 0;
        let mut last = 0;
        for pid in pids_value.split_whitespace() {
            count += 1;
            last = pid.parse::<u32>().map_err(|_| E::Sandbox)?;
            if count > 32 || last == 0 {
                return Err(E::Sandbox);
            }
        }
        if pids.next().is_some()
            || count < 2
            || last != 1
            || !strict_user_descendant
            || !strict_pid_descendant
        {
            return Err(E::Sandbox);
        }
        Capabilities::ZYGOTE
    } else {
        if matches!(
            role,
            ChromiumRole::Main
                | ChromiumRole::UnsandboxedZygote
                | ChromiumRole::Gpu
                | ChromiumRole::NetworkServiceUtility
                | ChromiumRole::NetworkServiceBroker
                | ChromiumRole::GpuBroker
        ) && (strict_user_descendant || strict_pid_descendant)
        {
            return Err(E::Sandbox);
        }
        Capabilities::ZERO
    };
    if Capabilities::parse(status)? != expected {
        return Err(E::Sandbox);
    }
    Ok(())
}
pub(crate) fn renderer_sandbox(
    status: &str,
    outer_pid_namespace: u64,
    renderer_pid_namespace: u64,
    outer_user_namespace: u64,
    renderer_user_namespace: u64,
) -> Result<()> {
    if !status.lines().any(|l| l == "Seccomp:\t2")
        || !status.lines().any(|l| l == "NoNewPrivs:\t1")
        || outer_pid_namespace == renderer_pid_namespace
        || outer_user_namespace == renderer_user_namespace
    {
        Err(E::Sandbox)
    } else {
        Ok(())
    }
}
/// Specialized Linux GPU policy: seccomp without the renderer's PID/user
/// namespace sandbox. Exact ancestry and executable identity are checked by
/// the stopped-population verifier, before calling this predicate.
pub(crate) fn gpu_sandbox(
    status: &str,
    strict_user_descendant: bool,
    strict_pid_descendant: bool,
) -> Result<()> {
    process_capabilities(
        ChromiumRole::Gpu,
        status,
        strict_user_descendant,
        strict_pid_descendant,
    )?;
    for (field, expected) in [("Seccomp", "\t2"), ("NoNewPrivs", "\t1")] {
        let mut values = status
            .lines()
            .filter_map(|l| l.split_once(':'))
            .filter_map(|(key, value)| (key == field).then_some(value));
        if values.next() != Some(expected) || values.next().is_some() {
            return Err(E::Sandbox);
        }
    }
    Ok(())
}
/// kNetwork uses direct exec and its own seccomp policy, not a zygote
/// namespace sandbox. Matching GPU scalar values do not grant GPU authority.
pub(crate) fn network_service_sandbox(
    status: &str,
    strict_user_descendant: bool,
    strict_pid_descendant: bool,
) -> Result<()> {
    process_capabilities(
        ChromiumRole::NetworkServiceUtility,
        status,
        strict_user_descendant,
        strict_pid_descendant,
    )?;
    for (field, expected) in [("Seccomp", "\t2"), ("NoNewPrivs", "\t1")] {
        let mut values = status
            .lines()
            .filter_map(|l| l.split_once(':'))
            .filter_map(|(key, value)| (key == field).then_some(value));
        if values.next() != Some(expected) || values.next().is_some() {
            return Err(E::Sandbox);
        }
    }
    Ok(())
}
/// Dedicated BrokerProcessPolicy scalar evidence. This does not attest the
/// filter contents or grant the client/zygote's role authority.
pub(crate) fn broker_sandbox(
    status: &str,
    strict_user_descendant: bool,
    strict_pid_descendant: bool,
) -> Result<()> {
    if strict_user_descendant || strict_pid_descendant {
        return Err(E::Sandbox);
    }
    cleared_capabilities(status)?;
    for (field, expected) in [("Seccomp", "\t2"), ("NoNewPrivs", "\t1")] {
        let mut values = status
            .lines()
            .filter_map(|l| l.split_once(':'))
            .filter_map(|(key, value)| (key == field).then_some(value));
        if values.next() != Some(expected) || values.next().is_some() {
            return Err(E::Sandbox);
        }
    }
    Ok(())
}

/// Relational check over the same retained stopped population. The kernel
/// adapter inserts members only after ALL individual role checks succeed.
/// This map is not an input format or an independent admission authority.
pub(crate) fn broker_relationships(
    members: &std::collections::BTreeMap<i32, (PreliminaryRole, i32)>,
) -> Result<std::collections::BTreeMap<i32, ChromiumRole>> {
    use ChromiumRole::{Gpu, GpuBroker, NetworkServiceBroker, NetworkServiceUtility};
    if members.len() > 256 {
        return Err(E::Limit);
    }
    let mut children = std::collections::BTreeMap::new();
    let mut resolved = std::collections::BTreeMap::new();
    for (&pid, &(role, parent)) in members {
        if pid <= 0 || parent <= 0 || pid == parent {
            return Err(E::ProcessIdentity);
        }
        if let PreliminaryRole::Process(role) = role {
            if matches!(role, GpuBroker | NetworkServiceBroker) {
                return Err(E::Sandbox);
            }
            resolved.insert(pid, role);
            continue;
        }
        let broker = match members.get(&parent).map(|p| p.0) {
            Some(PreliminaryRole::Process(NetworkServiceUtility)) => NetworkServiceBroker,
            Some(PreliminaryRole::Process(Gpu)) => GpuBroker,
            _ => return Err(E::ProcessIdentity),
        };
        resolved.insert(pid, broker);
        if children.insert(parent, pid).is_some() {
            return Err(E::Sandbox);
        }
    }
    for (&pid, &(role, _)) in members {
        if matches!(role, PreliminaryRole::Process(Gpu | NetworkServiceUtility))
            && !children.contains_key(&pid)
        {
            return Err(E::Sandbox);
        }
    }
    Ok(resolved)
}

/// Object identity is checked before process-title attribution for every member.
pub(crate) fn executable_identity(
    actual: (u64, u64),
    main: (u64, u64),
    allowed: bool,
) -> Result<()> {
    if !allowed || actual != main {
        Err(E::ProcessIdentity)
    } else {
        Ok(())
    }
}
/// V9 checks the final live singleton, not historical Network Service PIDs.
pub(crate) fn network_service_population(live: usize) -> Result<()> {
    if live == 1 { Ok(()) } else { Err(E::Sandbox) }
}
pub(crate) fn parent_pid(status: &str) -> Result<i32> {
    let mut values = status.lines().filter_map(|l| l.strip_prefix("PPid:\t"));
    let value = values.next().ok_or(E::ProcessIdentity)?;
    if values.next().is_some() || value.len() > 10 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(E::ProcessIdentity);
    }
    let pid = value.parse::<i32>().map_err(|_| E::ProcessIdentity)?;
    if pid <= 0 {
        return Err(E::ProcessIdentity);
    }
    Ok(pid)
}
#[cfg(test)]
mod tests {
    fn title_role(text: &str) -> super::Result<super::ChromiumRole> {
        match super::chromium_child_role(&super::ChromiumProcessTitle::parse(text)?)? {
            super::PreliminaryRole::Process(role) => Ok(role),
            super::PreliminaryRole::BrokerCandidate => Err(super::E::Sandbox),
        }
    }
    use super::*;
    #[test]
    fn namespace_owner_identity_and_pid_one_are_all_required() {
        assert!(namespace_binding(1, 2, 3, 3, "1").is_ok());
        for args in [(1, 1, 3, 3, "1"), (1, 2, 3, 4, "1"), (1, 2, 3, 3, "2")] {
            assert_eq!(
                namespace_binding(args.0, args.1, args.2, args.3, args.4),
                Err(E::Isolation)
            );
        }
    }
    #[test]
    fn sandbox_is_not_inferred_from_launch_flags() {
        assert_eq!(
            renderer_sandbox("Seccomp:\t0\nNoNewPrivs:\t1\n", 1, 2, 3, 4),
            Err(E::Sandbox)
        );
        assert_eq!(
            renderer_sandbox("Seccomp:\t2\nNoNewPrivs:\t1\n", 1, 1, 3, 4),
            Err(E::Sandbox)
        );
        assert_eq!(
            renderer_sandbox("Seccomp:\t2\nNoNewPrivs:\t1\n", 1, 2, 3, 3),
            Err(E::Sandbox)
        );
        assert!(renderer_sandbox("Seccomp:\t2\nNoNewPrivs:\t1\n", 1, 2, 3, 4).is_ok());
    }
    #[test]
    fn all_execution_capability_sets_must_be_zero_and_unique() {
        let valid = "CapEff:\t0000000000000000\nCapPrm:\t0000000000000000\nCapInh:\t0000000000000000\nCapAmb:\t0000000000000000\n";
        assert!(cleared_capabilities(valid).is_ok());
        for field in ["CapEff", "CapPrm", "CapInh", "CapAmb"] {
            let line = format!("{field}:\t0000000000000000\n");
            assert!(cleared_capabilities(&valid.replace(&line, "")).is_err());
            assert!(cleared_capabilities(&format!("{valid}{line}")).is_err());
            assert!(
                cleared_capabilities(
                    &valid.replace(&line, &format!("{field}:\t0000000000000001\n"))
                )
                .is_err()
            );
        }
    }
    fn status(effective: u64, permitted: u64, inheritable: u64, ambient: u64) -> String {
        format!(
            "CapEff:\t{effective:016x}\nCapPrm:\t{permitted:016x}\nCapInh:\t{inheritable:016x}\nCapAmb:\t{ambient:016x}\nNSpid:\t20\t1\nSeccomp:\t2\nNoNewPrivs:\t1\n"
        )
    }
    #[test]
    fn exact_namespace_zygote_exception_is_closed() {
        let zero = status(0, 0, 0, 0);
        let zygote = status(SYS_ADMIN, SYS_ADMIN, 0, 0);
        assert!(process_capabilities(ChromiumRole::Main, &zero, false, false).is_ok());
        assert!(process_capabilities(ChromiumRole::Renderer, &zero, true, true).is_ok());
        assert!(renderer_sandbox(&zero, 1, 2, 3, 4).is_ok());
        assert!(process_capabilities(ChromiumRole::NamespaceZygote, &zygote, true, true).is_ok());
        for role in [
            ChromiumRole::Main,
            ChromiumRole::Renderer,
            ChromiumRole::Gpu,
            ChromiumRole::NetworkServiceUtility,
        ] {
            assert!(process_capabilities(role, &zygote, true, true).is_err());
        }
        for (user, pid) in [(false, true), (true, false), (false, false)] {
            assert!(
                process_capabilities(ChromiumRole::NamespaceZygote, &zygote, user, pid).is_err()
            );
        }
        for bad in [
            status(SYS_ADMIN | 1, SYS_ADMIN, 0, 0),
            status(SYS_ADMIN, SYS_ADMIN | 1, 0, 0),
            status(SYS_ADMIN, SYS_ADMIN, 1, 0),
            status(SYS_ADMIN, SYS_ADMIN, 0, 1),
            zero,
            zygote.replace("20\t1", "20\t2"),
            zygote.replace("NSpid:\t20\t1\n", ""),
            format!("{zygote}NSpid:\t20\t1\n"),
        ] {
            assert!(process_capabilities(ChromiumRole::NamespaceZygote, &bad, true, true).is_err());
        }
    }
    #[test]
    fn capability_parser_and_role_metadata_fail_closed() {
        let valid = status(SYS_ADMIN, SYS_ADMIN, 0, 0);
        for field in ["CapEff", "CapPrm", "CapInh", "CapAmb"] {
            let prefix = format!("{field}:");
            let missing = valid
                .lines()
                .filter(|l| !l.starts_with(&prefix))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(Capabilities::parse(&missing).is_err());
            assert!(Capabilities::parse(&format!("{valid}{field}:bad\n")).is_err());
        }
        assert!(Capabilities::parse(&valid.replace("0000000000200000", "garbage")).is_err());
        assert!(title_role("chrome\0").is_err());
        assert_eq!(
            title_role("chrome --type=zygote\0"),
            Ok(ChromiumRole::NamespaceZygote)
        );
        for args in [
            "chrome\0",
            "chrome --type=unknown\0",
            "chrome --type=zygote --type=zygote\0",
            "chrome --type zygote\0",
            "chrome --type=zygote",
        ] {
            assert!(title_role(args).is_err());
        }
    }
    #[test]
    fn dual_zygote_switches_do_not_confer_each_others_authority() {
        let generic = title_role("chrome --type=zygote\0").unwrap();
        let bootstrap = title_role("chrome --type=zygote --no-zygote-sandbox\0").unwrap();
        assert_eq!(generic, ChromiumRole::NamespaceZygote);
        assert_eq!(bootstrap, ChromiumRole::UnsandboxedZygote);
        let zero = status(0, 0, 0, 0);
        let privileged = status(SYS_ADMIN, SYS_ADMIN, 0, 0);
        assert!(process_capabilities(generic, &privileged, true, true).is_ok());
        assert!(process_capabilities(generic, &zero, false, false).is_err());
        assert!(process_capabilities(bootstrap, &zero, false, false).is_ok());
        assert!(process_capabilities(bootstrap, &privileged, false, false).is_err());
        for (user, pid) in [(true, false), (false, true), (true, true)] {
            assert!(process_capabilities(bootstrap, &zero, user, pid).is_err());
        }
        for role in ["renderer", "gpu-process", "utility", "unrelated"] {
            assert!(title_role(&format!("chrome --type={role} --no-zygote-sandbox\0")).is_err());
        }
        for args in [
            "chrome --no-zygote-sandbox\0",
            "chrome --type=zygote --no-zygote-sandbox --no-zygote-sandbox\0",
            "chrome --type=zygote --no-zygote-sandbox=false\0",
        ] {
            assert!(title_role(args).is_err());
        }
    }
    #[test]
    fn forbidden_sandbox_switches_fail_for_every_role() {
        for flag in [
            "--no-sandbox",
            "--disable-namespace-sandbox",
            "--disable-seccomp-filter-sandbox",
            "--disable-gpu-sandbox",
            "--disable-setuid-sandbox",
            "--no-zygote",
            "--no-unsandboxed-zygote",
            "--single-process",
            "--in-process-gpu",
            "--allow-sandbox-debugging",
            "--gpu-sandbox-failures-fatal",
        ] {
            for suffix in ["", "=false"] {
                assert!(
                    title_role(&format!(
                        "chrome --type=zygote --no-zygote-sandbox {flag}{suffix}\0"
                    ))
                    .is_err()
                );
                assert!(title_role(&format!("chrome {flag}{suffix}\0")).is_err());
            }
        }
    }
    #[test]
    fn gpu_policy_is_specialized() {
        let good = status(0, 0, 0, 0);
        assert!(gpu_sandbox(&good, false, false).is_ok());
        assert!(renderer_sandbox(&good, 1, 1, 2, 2).is_err());
        assert!(renderer_sandbox(&good, 1, 3, 2, 4).is_ok());
        for (user, pid) in [(true, false), (false, true), (true, true)] {
            assert!(gpu_sandbox(&good, user, pid).is_err());
        }
        for bad in [
            good.replace("Seccomp:\t2", "Seccomp:\t0"),
            good.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
            good.replace("Seccomp:\t2\n", ""),
            format!("{good}Seccomp:\t2\n"),
            status(SYS_ADMIN, SYS_ADMIN, 0, 0),
        ] {
            assert!(gpu_sandbox(&bad, false, false).is_err());
        }

        assert_eq!(parent_pid("PPid:\t23\n"), Ok(23));
        for bad in [
            "",
            "PPid:\t0\n",
            "PPid:\t23\nPPid:\t23\n",
            "PPid:\t99999999999\n",
        ] {
            assert!(parent_pid(bad).is_err());
        }
    }

    const NETWORK: &str = "chrome --type=utility --utility-sub-type=network.mojom.NetworkService --service-sandbox-type=network\0";
    #[test]
    fn only_exact_network_utility_metadata_is_reviewed() {
        let network_text = NETWORK.trim_end_matches('\0');
        assert_eq!(title_role(NETWORK), Ok(ChromiumRole::NetworkServiceUtility));
        for bad in [
            "chrome --type=utility\0".to_owned(),
            NETWORK.replace(" --utility-sub-type=network.mojom.NetworkService", ""),
            NETWORK.replace(
                "network.mojom.NetworkService",
                "storage.mojom.StorageService",
            ),
            NETWORK.replace(" --service-sandbox-type=network", ""),
            NETWORK.replace(
                "--service-sandbox-type=network",
                "--service-sandbox-type=none",
            ),
            NETWORK.replace(
                "--service-sandbox-type=network",
                "--service-sandbox-type=utility",
            ),
            format!("{network_text} --utility-sub-type=network.mojom.NetworkService\0"),
            format!("{network_text} --service-sandbox-type=network\0"),
            format!("{network_text} --service-sandbox-type=none\0"),
            format!("{network_text} --utility-sub-type network.mojom.NetworkService\0"),
            format!("{network_text} --no-zygote-sandbox\0"),
        ] {
            assert!(title_role(&bad).is_err(), "{bad:?}");
        }
        for role in ["zygote", "renderer", "gpu-process", "unknown"] {
            assert!(
                title_role(&NETWORK.replace("--type=utility", &format!("--type={role}"))).is_err()
            );
        }

        for flag in [
            "--no-sandbox",
            "--disable-namespace-sandbox",
            "--disable-seccomp-filter-sandbox",
            "--single-process",
        ] {
            assert!(title_role(&format!("{network_text} {flag}\0")).is_err());
        }
    }
    #[test]
    fn network_sandbox_requires_objects_namespaces_and_exact_unprivileged_state() {
        let good = status(0, 0, 0, 0);
        assert!(network_service_sandbox(&good, false, false).is_ok());
        for (user, pid) in [(true, false), (false, true), (true, true)] {
            assert!(network_service_sandbox(&good, user, pid).is_err());
        }
        for bad in [
            status(SYS_ADMIN, SYS_ADMIN, 0, 0),
            status(0, 0, 1, 0),
            status(0, 0, 0, 1),
            good.replace("Seccomp:\t2", "Seccomp:\t0"),
            good.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
            good.replace("NoNewPrivs:\t1\n", ""),
            format!("{good}Seccomp:\t2\n"),
        ] {
            assert!(network_service_sandbox(&bad, false, false).is_err());
        }
        // Same predicate called by the retained, stopped population before role parsing.
        assert!(executable_identity((1, 2), (1, 2), true).is_ok());
        assert_eq!(
            executable_identity((1, 3), (1, 2), true),
            Err(E::ProcessIdentity)
        );
        assert_eq!(
            executable_identity((1, 2), (1, 2), false),
            Err(E::ProcessIdentity)
        );
        assert!(process_capabilities(ChromiumRole::NamespaceZygote, &good, false, false).is_err());
    }

    #[test]
    fn network_service_population_is_one_live_reviewed_role() {
        assert!(network_service_population(1).is_ok());
        for count in [0, 2, 256] {
            assert_eq!(network_service_population(count), Err(E::Sandbox));
        }
    }

    #[test]
    fn flattened_titles_and_broker_candidates_are_distinct_evidence() {
        for (text, role) in [
            ("chrome --type=renderer\0", ChromiumRole::Renderer),
            ("chrome --type=gpu-process\0", ChromiumRole::Gpu),
        ] {
            assert_eq!(title_role(text), Ok(role));
        }
        assert_eq!(
            chromium_child_role(&ChromiumProcessTitle::parse("chrome --type=broker\0\0").unwrap()),
            Ok(PreliminaryRole::BrokerCandidate)
        );
        for bad in [
            "chrome\0--type=broker\0",
            "chrome  --type=broker\0",
            "chrome --type=broker \0",
            "chrome --type=broker",
            "chrome --type=broker\t\0",
            "chrome --note='x --type=broker'\0",
            "chrome --type=broker --type=broker\0",
            "chrome --type=utility-broker\0",
            "chrome --type=gpu-process-broker\0",
            "chrome --type=broker --utility-sub-type=network.mojom.NetworkService\0",
            "chrome --type=broker --no-sandbox\0",
        ] {
            assert!(
                ChromiumProcessTitle::parse(bad)
                    .and_then(|t| chromium_child_role(&t))
                    .is_err(),
                "{bad:?}"
            );
        }
        for text in [
            "chrome --note=--type=broker\0",
            "chrome --not-type=broker\0",
        ] {
            assert!(title_role(text).is_err());
            assert!(title_role(text).is_err());
        }
    }
    #[test]
    fn main_is_owned_and_never_reads_child_title() {
        let config = crate::configuration::specimen();
        assert_eq!(
            config.invocation_arguments.last().map(String::as_str),
            Some("about:blank")
        );
        let full_title = format!("chrome {}\0", config.invocation_arguments.join(" "));
        assert!(ChromiumProcessTitle::parse(&full_title).is_err());
        assert_eq!(
            attribute_role(7, 7, || panic!("Main must not read a child title")),
            Ok(PreliminaryRole::Process(ChromiumRole::Main))
        );
        assert!(attribute_role(8, 7, || Ok(full_title)).is_err());
        for child in [
            "chrome --headless=new\0",
            "chrome --type=renderer about:blank\0",
            "chrome --type=broker other\0",
        ] {
            assert!(attribute_role(8, 7, || Ok(child.into())).is_err());
        }
        let expected = MainIdentity {
            pid: 7,
            executable: (10, 20),
            namespaces: [30, 40, 50],
        };
        let zero = status(0, 0, 0, 0);
        verify_main(expected, expected, &zero).unwrap();
        let mut bad = expected;
        bad.pid = 8;
        assert!(verify_main(bad, expected, &zero).is_err());
        bad = expected;
        bad.executable.1 += 1;
        assert!(verify_main(bad, expected, &zero).is_err());
        for i in 0..3 {
            bad = expected;
            bad.namespaces[i] += 1;
            assert!(verify_main(bad, expected, &zero).is_err());
        }
        for masks in [(1, 0, 0, 0), (0, 1, 0, 0), (0, 0, 1, 0), (0, 0, 0, 1)] {
            assert!(
                verify_main(
                    expected,
                    expected,
                    &status(masks.0, masks.1, masks.2, masks.3)
                )
                .is_err()
            );
        }
    }
    #[test]
    fn process_title_bounds_and_padding_are_explicit() {
        assert!(ChromiumProcessTitle::parse(&format!("chrome {}\0", "--x ".repeat(257))).is_err());
        assert!(
            ChromiumProcessTitle::parse(&format!("chrome --x={}\0", "x".repeat(4096))).is_err()
        );
        assert!(ChromiumProcessTitle::parse(&format!("{}\0", "x".repeat(65536))).is_err());
        for bad in [
            "\0",
            " chrome --type=broker\0",
            "chrome --type=broker\0hidden\0",
            "chrome --label=é\0",
            "chrome --note=x\\y\0",
            "chrome --type=broker --\0",
        ] {
            assert!(ChromiumProcessTitle::parse(bad).is_err());
        }
    }
    #[test]
    fn broker_state_cannot_be_repaired_by_correct_title() {
        let good = status(0, 0, 0, 0);
        assert!(broker_sandbox(&good, false, false).is_ok());
        for bad in [
            status(SYS_ADMIN, SYS_ADMIN, 0, 0),
            status(0, 0, 1, 0),
            status(0, 0, 0, 1),
            good.replace("NoNewPrivs:\t1", "NoNewPrivs:\t0"),
            good.replace("Seccomp:\t2", "Seccomp:\t0"),
        ] {
            assert!(broker_sandbox(&bad, false, false).is_err());
        }
        for field in ["NoNewPrivs:\t1\n", "Seccomp:\t2\n"] {
            assert!(broker_sandbox(&good.replace(field, ""), false, false).is_err());
            assert!(broker_sandbox(&format!("{good}{field}"), false, false).is_err());
        }
        assert!(broker_sandbox(&good, true, false).is_err());
        assert!(broker_sandbox(&good, false, true).is_err());
        assert_eq!(
            executable_identity((1, 2), (1, 3), true),
            Err(E::ProcessIdentity)
        );
    }
    #[test]
    fn stopped_population_resolves_only_two_closed_broker_relations() {
        use ChromiumRole::*;
        use PreliminaryRole::{BrokerCandidate as B, Process as P};
        let good = std::collections::BTreeMap::from([
            (2, (P(Main), 1)),
            (3, (P(UnsandboxedZygote), 2)),
            (4, (P(Gpu), 3)),
            (5, (P(NetworkServiceUtility), 2)),
            (6, (B, 4)),
            (7, (B, 5)),
        ]);
        let resolved = broker_relationships(&good).unwrap();
        assert_eq!(resolved[&6], GpuBroker);
        assert_eq!(resolved[&7], NetworkServiceBroker);
        for parent in [1, 2, 3, 6, 7, 99] {
            let mut bad = good.clone();
            bad.insert(6, (B, parent));
            assert!(broker_relationships(&bad).is_err());
        }
        for role in [
            Main,
            NamespaceZygote,
            UnsandboxedZygote,
            Renderer,
            NetworkServiceBroker,
            GpuBroker,
        ] {
            let mut bad = good.clone();
            bad.insert(4, (P(role), 3));
            assert!(broker_relationships(&bad).is_err());
        }
        let mut duplicate = good.clone();
        duplicate.insert(8, (B, 5));
        assert!(broker_relationships(&duplicate).is_err());
        for pid in [4, 5, 6, 7] {
            let mut bad = good.clone();
            bad.remove(&pid);
            assert!(broker_relationships(&bad).is_err());
        }
    }
}
