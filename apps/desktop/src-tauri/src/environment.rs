use std::{io::Read, process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub struct ProxyObservation {
    pub state: &'static str,
    pub reason_code: &'static str,
}

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

pub fn observe_proxy() -> ProxyObservation {
    if !cfg!(target_os = "macos") { return unknown("UNSUPPORTED_OS"); }
    // Fixed system command, no shell, no user-supplied arguments or settings writes.
    let Ok(mut child) = Command::new("/usr/sbin/scutil").arg("--proxy")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() else {
        return unknown("PROXY_READ_FAILED");
    };
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() { return unknown("PROXY_READ_FAILED"); }
                let mut bytes = Vec::new();
                let Some(stdout) = child.stdout.take() else { return unknown("PROXY_READ_FAILED"); };
                if stdout.take(131_073).read_to_end(&mut bytes).is_err() || bytes.len() > 131_072 {
                    return unknown("PROXY_RESPONSE_TOO_LARGE");
                }
                return std::str::from_utf8(&bytes).map(parse_proxy_flags)
                    .unwrap_or_else(|_| unknown("PROXY_RESPONSE_UNRECOGNIZED"));
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return unknown("PROXY_READ_FAILED");
            }
        }
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
}
