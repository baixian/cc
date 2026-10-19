use std::{io::Read, process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub struct ProxyObservation {
    pub state: &'static str,
    pub reason_code: &'static str,
}

#[derive(Clone, Copy)]
enum ReadFailure { Failed, TooLarge }

fn unknown(code: &'static str) -> ProxyObservation {
    ProxyObservation { state: "UNKNOWN", reason_code: code }
}

pub fn parse_proxy_flags(output: &str) -> ProxyObservation {
    const FLAGS: [&str; 8] = ["HTTPEnable", "HTTPSEnable", "SOCKSEnable", "FTPEnable",
        "GopherEnable", "RTSPEnable", "ProxyAutoConfigEnable", "ProxyAutoDiscoveryEnable"];
    let mut seen = false;
    let mut invalid = false;
    for line in output.lines() {
        let Some((key, value)) = line.trim().split_once(':') else { continue };
        if !FLAGS.contains(&key.trim()) { continue; }
        match value.trim() {
            "1" => return ProxyObservation { state: "ENABLED_FLAG_OBSERVED", reason_code: "PROXY_FLAG_ENABLED" },
            "0" => seen = true,
            _ => invalid = true,
        }
    }
    if invalid { unknown("PROXY_RESPONSE_UNRECOGNIZED") }
    else if seen { ProxyObservation { state: "NO_ENABLED_FLAG_OBSERVED", reason_code: "PROXY_NO_ENABLED_FLAG" } }
    else { unknown("PROXY_RESPONSE_UNRECOGNIZED") }
}

fn read_fixed_command(path: &str, args: &[&str]) -> Result<String, ReadFailure> {
    // Fixed system command, no shell, no user-supplied arguments or settings writes.
    let Ok(mut child) = Command::new(path).args(args)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
        return Err(ReadFailure::Failed);
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() { return Err(ReadFailure::Failed); }
                let mut bytes = Vec::new();
                let Some(stdout) = child.stdout.take() else { return Err(ReadFailure::Failed); };
                if stdout.take(131_073).read_to_end(&mut bytes).is_err() || bytes.len() > 131_072 {
                    return Err(ReadFailure::TooLarge);
                }
                return String::from_utf8(bytes).map_err(|_| ReadFailure::Failed);
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ReadFailure::Failed);
            }
        }
    }
}

pub fn observe_proxy() -> ProxyObservation {
    if !cfg!(target_os = "macos") { return unknown("UNSUPPORTED_OS"); }
    match read_fixed_command("/usr/sbin/scutil", &["--proxy"]) {
        Ok(output) => parse_proxy_flags(&output),
        Err(ReadFailure::TooLarge) => unknown("PROXY_RESPONSE_TOO_LARGE"),
        Err(ReadFailure::Failed) => unknown("PROXY_READ_FAILED"),
    }
}

pub fn parse_dns_summary(output: &str) -> ProxyObservation {
    if output.lines().any(|line| line.trim().starts_with("nameserver[")
        && line.split_once(':').is_some_and(|(_, value)| !value.trim().is_empty())) {
        ProxyObservation { state: "DNS_ENTRY_OBSERVED", reason_code: "DNS_NAMESERVER_ENTRY" }
    } else { unknown("DNS_ENTRY_NOT_OBSERVED") }
}

pub fn observe_dns() -> ProxyObservation {
    if !cfg!(target_os = "macos") { return unknown("UNSUPPORTED_OS"); }
    match read_fixed_command("/usr/sbin/scutil", &["--dns"]) {
        Ok(output) => parse_dns_summary(&output),
        Err(ReadFailure::TooLarge) => unknown("DNS_RESPONSE_TOO_LARGE"),
        Err(ReadFailure::Failed) => unknown("DNS_READ_FAILED"),
    }
}

pub fn parse_default_route(output: &str) -> ProxyObservation {
    let destination = output.lines().any(|line| line.trim().split_once(':')
        .is_some_and(|(key, value)| key == "destination" && value.trim() == "default"));
    let interface = output.lines().any(|line| line.trim().split_once(':')
        .is_some_and(|(key, value)| key == "interface" && !value.trim().is_empty()));
    if destination && interface {
        ProxyObservation { state: "DEFAULT_ROUTE_OBSERVED", reason_code: "DEFAULT_ROUTE_ENTRY" }
    } else { unknown("DEFAULT_ROUTE_NOT_OBSERVED") }
}

pub fn observe_default_route() -> ProxyObservation {
    if !cfg!(target_os = "macos") { return unknown("UNSUPPORTED_OS"); }
    match read_fixed_command("/sbin/route", &["-n", "get", "default"]) {
        Ok(output) => parse_default_route(&output),
        Err(ReadFailure::TooLarge) => unknown("ROUTE_RESPONSE_TOO_LARGE"),
        Err(ReadFailure::Failed) => unknown("ROUTE_READ_FAILED"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observes_flags_without_exposing_proxy_addresses() {
        let enabled = parse_proxy_flags("<dictionary> {\n HTTPProxy : 127.0.0.1\n HTTPEnable : 1\n HTTPSProxy : private.example\n }");
        assert_eq!(enabled.state, "ENABLED_FLAG_OBSERVED");
        let disabled = parse_proxy_flags("<dictionary> {\n HTTPEnable : 0\n SOCKSEnable : 0\n }");
        assert_eq!(disabled.state, "NO_ENABLED_FLAG_OBSERVED");
        assert_eq!(parse_proxy_flags("<dictionary> { } ").state, "UNKNOWN");
        assert_eq!(parse_proxy_flags("HTTPEnable : maybe").state, "UNKNOWN");
    }
    #[test]
    fn dns_and_route_summaries_do_not_copy_addresses() {
        let dns = parse_dns_summary("DNS configuration\nresolver #1\n nameserver[0] : 192.0.2.53\n");
        assert_eq!(dns.state, "DNS_ENTRY_OBSERVED");
        assert!(!dns.reason_code.contains("192.0.2.53"));
        assert_eq!(parse_dns_summary("DNS configuration\n").state, "UNKNOWN");
        let route = parse_default_route("route to: default\ndestination: default\ngateway: 192.0.2.1\ninterface: en0\n");
        assert_eq!(route.state, "DEFAULT_ROUTE_OBSERVED");
        assert!(!route.reason_code.contains("192.0.2.1"));
        assert_eq!(parse_default_route("destination: default\n").state, "UNKNOWN");
    }
}
