use crate::models::{ConnectionDto, ConnectionPageDto, CoreSnapshotDto, DiagnosticProbeDto, DiagnosticReportDto, ErrorDto, ProviderSummaryDto, ProxyCatalogDto, ProxyGroupDto, ProxyNodeDto, RouteEvidenceDto, RunningRuleDto, RunningRulesDto, RuleSimulationDto};
use serde_json::Value;
use std::{collections::BTreeMap, net::{Ipv4Addr, Ipv6Addr}, time::{Duration, Instant}};
use url::{Host, Url};

#[derive(Clone)]
pub struct ExternalCore {
    pub endpoint: Url,
    pub version: String,
}

pub fn parse_loopback_endpoint(input: &str) -> Result<Url, ErrorDto> {
    let url = Url::parse(input).map_err(|_| ErrorDto::new("INVALID_ENDPOINT", "控制器地址格式错误", false))?;
    let loopback = match url.host() {
        Some(Host::Ipv4(ip)) => ip == Ipv4Addr::LOCALHOST,
        Some(Host::Ipv6(ip)) => ip == Ipv6Addr::LOCALHOST,
        _ => false,
    };
    if url.scheme() != "http" || !loopback
        || url.port().is_none() || url.username() != "" || url.password().is_some()
        || url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(ErrorDto::new("INVALID_ENDPOINT", "仅允许 http://127.0.0.1:端口 或 http://[::1]:端口", false));
    }
    Ok(url)
}

fn client() -> Result<reqwest::Client, ErrorDto> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ErrorDto::new("CORE_CLIENT_ERROR", "无法创建本机控制器连接", false))
}

async fn get_json(endpoint: &Url, route: &str) -> Result<Value, ErrorDto> {
    let mut url = endpoint.clone();
    url.set_path(route);
    let response = client()?.get(url).send().await.map_err(|_| ErrorDto::new("CORE_UNAVAILABLE", "本机控制器未响应", true))?;
    match response.status().as_u16() {
        401 | 403 => return Err(ErrorDto::new("CORE_AUTH_REQUIRED", "控制器需要 Secret；当前版本暂不支持安全录入", false)),
        200..=299 => {},
        _ => return Err(ErrorDto::new("CORE_API_ERROR", "控制器返回异常状态", true)),
    }
    if response.content_length().is_some_and(|n| n > 8 * 1024 * 1024) {
        return Err(ErrorDto::new("CORE_RESPONSE_TOO_LARGE", "控制器数据超过读取上限", false));
    }
    let mut response = response;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| ErrorDto::new("CORE_UNAVAILABLE", "读取控制器数据失败", true))? {
        if chunk.len() > 8 * 1024 * 1024 - bytes.len() {
            return Err(ErrorDto::new("CORE_RESPONSE_TOO_LARGE", "控制器数据超过读取上限", false));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice::<Value>(&bytes).map_err(|_| ErrorDto::new("CORE_RESPONSE_INVALID", "控制器数据格式错误", false))
}

pub async fn connect(endpoint_input: &str) -> Result<ExternalCore, ErrorDto> {
    let endpoint = parse_loopback_endpoint(endpoint_input)?;
    let version = get_json(&endpoint, "/version").await?;
    let version = version.get("version").and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "内核版本字段缺失", false))?;
    Ok(ExternalCore { endpoint, version: version.to_owned() })
}

pub async fn snapshot(core: &ExternalCore) -> Result<CoreSnapshotDto, ErrorDto> {
    let value = get_json(&core.endpoint, "/connections").await?;
    parse_snapshot(&value, &core.version)
}

pub fn parse_snapshot(value: &Value, version: &str) -> Result<CoreSnapshotDto, ErrorDto> {
    let connections = value.get("connections").and_then(Value::as_array)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "连接列表字段缺失", false))?;
    let mut chains = BTreeMap::<String, u64>::new();
    let mut unknown_route_count = 0;
    let mut identified_process_count = 0;
    for entry in connections {
        let chain = entry.get("chains").and_then(Value::as_array)
            .map(|items| items.iter().take(12).filter_map(|item| bounded_string(Some(item))).collect::<Vec<_>>())
            .filter(|items| !items.is_empty()).map(|items| items.join(" → "));
        if let Some(chain) = chain { *chains.entry(chain).or_default() += 1; }
        else { unknown_route_count += 1; }
        if entry.get("metadata").and_then(|m| m.get("processName").or_else(|| m.get("process")))
            .and_then(Value::as_str).is_some_and(|s| !s.is_empty()) { identified_process_count += 1; }
    }
    let mut route_evidence: Vec<_> = chains.into_iter().map(|(chain, connection_count)| RouteEvidenceDto { chain, connection_count }).collect();
    route_evidence.sort_by(|a, b| b.connection_count.cmp(&a.connection_count).then_with(|| a.chain.cmp(&b.chain)));
    route_evidence.truncate(6);
    Ok(CoreSnapshotDto {
        state: "EXTERNAL_READONLY".into(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        core_version: version.into(),
        connection_count: connections.len() as u64,
        upload_bytes: safe_byte_count(value.get("uploadTotal")),
        download_bytes: safe_byte_count(value.get("downloadTotal")),
        route_evidence,
        unknown_route_count,
        identified_process_count,
    })
}

fn bounded_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.chars().take(256).collect())
}

fn safe_byte_count(value: Option<&Value>) -> Option<u64> {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    value.and_then(Value::as_u64).filter(|n| *n <= MAX_SAFE_INTEGER)
}

pub fn parse_connections(value: &Value, filter: &str, offset: usize, limit: usize) -> Result<ConnectionPageDto, ErrorDto> {
    let raw = value.get("connections").and_then(Value::as_array)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "连接列表字段缺失", false))?;
    let mut skipped_count = 0;
    let needle = filter.to_lowercase();
    let mut items = Vec::new();
    for entry in raw {
        let Some(id) = bounded_string(entry.get("id")) else { skipped_count += 1; continue };
        let metadata = entry.get("metadata");
        let host = bounded_string(metadata.and_then(|m| m.get("host")));
        let destination_ip = bounded_string(metadata.and_then(|m| m.get("destinationIP")));
        let process_name = bounded_string(metadata.and_then(|m| m.get("processName").or_else(|| m.get("process"))));
        let rule = bounded_string(entry.get("rule"));
        if !needle.is_empty() && ![host.as_deref(), destination_ip.as_deref(), process_name.as_deref(), rule.as_deref()]
            .iter().flatten().any(|s| s.to_lowercase().contains(&needle)) { continue; }
        let chains = entry.get("chains").and_then(Value::as_array).map(|a| a.iter().take(12)
            .filter_map(|v| bounded_string(Some(v))).collect()).unwrap_or_default();
        items.push(ConnectionDto { id, host, destination_ip, process_name, rule, chains,
            upload_bytes: safe_byte_count(entry.get("upload")),
            download_bytes: safe_byte_count(entry.get("download")) });
    }
    items.sort_by(|a,b| a.id.cmp(&b.id));
    let total = items.len();
    let page = items.into_iter().skip(offset).take(limit).collect();
    let next = offset.saturating_add(limit);
    Ok(ConnectionPageDto { items: page, next_cursor: (next < total).then(|| next.to_string()),
        has_more: next < total, total, skipped_count, observed_at: chrono::Utc::now().to_rfc3339() })
}

pub async fn list_connections(core: &ExternalCore, filter: &str, offset: usize, limit: usize) -> Result<ConnectionPageDto, ErrorDto> {
    let value = get_json(&core.endpoint, "/connections").await?;
    parse_connections(&value, filter, offset, limit)
}

fn diagnostic_probe(name: &str, result: Result<Value, ErrorDto>, elapsed: Duration) -> DiagnosticProbeDto {
    let reason_code = match result {
        Ok(value) if name == "version" && value.get("version").and_then(Value::as_str).is_some_and(|s| !s.is_empty()) => None,
        Ok(value) if name == "connections" && value.get("connections").and_then(Value::as_array).is_some() => None,
        Ok(_) => Some("CORE_RESPONSE_INVALID".into()),
        Err(error) => Some(error.code),
    };
    DiagnosticProbeDto { name: name.into(), status: if reason_code.is_some() { "UNAVAILABLE" } else { "AVAILABLE" }.into(),
        latency_ms: elapsed.as_millis().min(u64::MAX as u128) as u64, reason_code }
}

pub async fn diagnose(core: &ExternalCore) -> DiagnosticReportDto {
    let start = Instant::now();
    let version = diagnostic_probe("version", get_json(&core.endpoint, "/version").await, start.elapsed());
    let start = Instant::now();
    let connections = diagnostic_probe("connections", get_json(&core.endpoint, "/connections").await, start.elapsed());
    DiagnosticReportDto { observed_at: chrono::Utc::now().to_rfc3339(), scope: "LOCAL_CONTROLLER_ONLY".into(), probes: vec![version, connections] }
}

fn parse_catalog(value: &Value, providers: Result<Value, ErrorDto>) -> Result<ProxyCatalogDto, ErrorDto> {
    let raw = value.get("proxies").and_then(Value::as_object)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "代理目录字段缺失", false))?;
    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let mut omitted_count = raw.len().saturating_sub(1000) as u64;
    for (key, entry) in raw.iter().take(1000) {
        let name = bounded_string(entry.get("name")).unwrap_or_else(|| key.chars().take(256).collect());
        let kind = bounded_string(entry.get("type")).unwrap_or_else(|| "UNKNOWN".into());
        if let Some(all) = entry.get("all").and_then(Value::as_array) {
            let candidates = all.iter().take(200).filter_map(|item| bounded_string(Some(item))).collect();
            groups.push(ProxyGroupDto { name, kind, selected: bounded_string(entry.get("now")), candidates });
        } else {
            let last_delay_ms = entry.get("history").and_then(Value::as_array).and_then(|a| a.last())
                .and_then(|h| h.get("delay")).and_then(Value::as_u64).filter(|n| *n > 0 && *n <= 60_000);
            nodes.push(ProxyNodeDto { name, kind, alive: entry.get("alive").and_then(Value::as_bool), last_delay_ms });
        }
    }
    let (providers, provider_error_code) = match providers {
        Ok(value) => match value.get("providers").and_then(Value::as_object) {
            Some(raw) => {
                omitted_count += raw.len().saturating_sub(100) as u64;
                (raw.iter().take(100).map(|(name, data)| ProviderSummaryDto {
                    name: name.chars().take(256).collect(),
                    node_count: data.get("proxies").and_then(Value::as_array).map_or(0, |a| a.len() as u64),
                }).collect(), None)
            }
            None => (Vec::new(), Some("CORE_RESPONSE_INVALID".into())),
        },
        Err(error) => (Vec::new(), Some(error.code)),
    };
    Ok(ProxyCatalogDto { observed_at: chrono::Utc::now().to_rfc3339(), nodes, groups, providers, provider_error_code, omitted_count })
}

pub async fn catalog(core: &ExternalCore) -> Result<ProxyCatalogDto, ErrorDto> {
    let proxies = get_json(&core.endpoint, "/proxies").await?;
    let providers = get_json(&core.endpoint, "/providers/proxies").await;
    parse_catalog(&proxies, providers)
}

pub fn parse_rules(value: &Value) -> Result<RunningRulesDto, ErrorDto> {
    let raw = value.get("rules").and_then(Value::as_array)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "规则列表字段缺失", false))?;
    let rules = raw.iter().take(2000).enumerate().map(|(index, entry)| RunningRuleDto {
        index: entry.get("index").and_then(Value::as_u64).unwrap_or(index as u64),
        kind: if entry.get("payload").and_then(Value::as_str).is_some_and(|s| s.chars().count() > 256) {
            "UNKNOWN".into()
        } else { bounded_string(entry.get("type")).unwrap_or_else(|| "UNKNOWN".into()) },
        payload: bounded_string(entry.get("payload")).unwrap_or_default(),
        proxy: bounded_string(entry.get("proxy")).unwrap_or_else(|| "UNKNOWN".into()),
        disabled: entry.get("extra").and_then(|extra| extra.get("disabled")).and_then(Value::as_bool).unwrap_or(false),
    }).collect();
    Ok(RunningRulesDto { observed_at: chrono::Utc::now().to_rfc3339(), rules, omitted_count: raw.len().saturating_sub(2000) as u64 })
}

pub async fn running_rules(core: &ExternalCore) -> Result<RunningRulesDto, ErrorDto> {
    parse_rules(&get_json(&core.endpoint, "/rules").await?)
}

fn valid_hostname(host: &str) -> bool {
    host.len() <= 253 && host.len() >= 3 && host.contains('.') && host.split('.').all(|label|
        !label.is_empty() && label.len() <= 63 && !label.starts_with('-') && !label.ends_with('-')
            && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
}

pub fn simulate_domain(rules: &RunningRulesDto, host: &str) -> RuleSimulationDto {
    let host = host.to_ascii_lowercase();
    let make = |state: &str, rule: Option<RunningRuleDto>, reason: Option<&str>| RuleSimulationDto {
        observed_at: rules.observed_at.clone(), state: state.into(), rule,
        reason_code: reason.map(str::to_owned), scope: "STATIC_DOMAIN_ONLY".into(),
    };
    for rule in &rules.rules {
        if rule.disabled { continue; }
        let value = rule.payload.to_ascii_lowercase();
        let matched = match rule.kind.as_str() {
            "DOMAIN" if valid_hostname(&value) => host == value,
            "DOMAIN-SUFFIX" if valid_hostname(&value) => host == value || host.ends_with(&format!(".{value}")),
            "DOMAIN-KEYWORD" if !value.is_empty() => host.contains(&value),
            "MATCH" => true,
            _ => return make("UNSUPPORTED", Some(rule.clone()), Some("EARLIER_RULE_UNRESOLVED")),
        };
        if matched { return make("MATCHED", Some(rule.clone()), None); }
    }
    if rules.omitted_count > 0 { make("UNSUPPORTED", None, Some("RULE_LIST_TRUNCATED")) }
    else { make("NO_MATCH", None, None) }
}

pub async fn simulate_running_domain(core: &ExternalCore, host: &str) -> Result<RuleSimulationDto, ErrorDto> {
    if !valid_hostname(host) { return Err(ErrorDto::new("INVALID_HOSTNAME", "请输入有效的 ASCII 域名", false)); }
    let rules = running_rules(core).await?;
    Ok(simulate_domain(&rules, host))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_literal_loopback_only() {
        assert!(parse_loopback_endpoint("http://127.0.0.1:9090").is_ok());
        assert!(parse_loopback_endpoint("http://[::1]:9090").is_ok());
        for s in ["http://localhost:9090", "http://127.0.0.2:9090", "http://192.168.1.1:9090", "https://example.com", "http://127.0.0.1:9090/other", "http://127.0.0.1:9090?x=1", "http://u:p@127.0.0.1:9090"] {
            assert!(parse_loopback_endpoint(s).is_err(), "accepted {s}");
        }
    }
    #[test]
    fn connection_page_preserves_unknown_fields_as_unknown() {
        let input: Value = serde_json::from_str(include_str!("../fixtures/connections.json")).unwrap();
        let page = parse_connections(&input, "", 0, 1).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.skipped_count, 1);
        assert_eq!(page.items[0].id, "c-01");
        assert!(page.items[0].host.is_none());
        assert!(page.items[0].download_bytes.is_none());
        assert!(page.has_more);
        let filtered = parse_connections(&input, "EXAMPLE", 0, 20).unwrap();
        assert_eq!(filtered.total, 1);
        assert_eq!(filtered.items[0].upload_bytes, Some(512));
        let second = parse_connections(&input, "", 1, 2).unwrap();
        assert_eq!(second.items.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(), vec!["c-02", "c-03"]);
        assert!(!second.has_more);
        assert_eq!(safe_byte_count(Some(&serde_json::json!(9_007_199_254_740_992_u64))), None);
    }
    #[test]
    fn snapshot_evidence_counts_all_rows_without_claiming_system_routing() {
        let input: Value = serde_json::from_str(include_str!("../fixtures/connections.json")).unwrap();
        let snapshot = parse_snapshot(&input, "test").unwrap();
        assert_eq!(snapshot.connection_count, 4);
        assert_eq!(snapshot.route_evidence.len(), 1);
        assert_eq!(snapshot.route_evidence[0].chain, "Proxy → US");
        assert_eq!(snapshot.route_evidence[0].connection_count, 1);
        assert_eq!(snapshot.unknown_route_count, 3);
        assert_eq!(snapshot.identified_process_count, 1);
    }
    #[test]
    fn diagnostic_evidence_keeps_api_failure_reason() {
        let ok = diagnostic_probe("version", Ok(serde_json::json!({"version":"1.0"})), Duration::from_millis(12));
        assert_eq!(ok.status, "AVAILABLE");
        assert_eq!(ok.latency_ms, 12);
        let invalid = diagnostic_probe("connections", Ok(serde_json::json!({"connections":null})), Duration::ZERO);
        assert_eq!(invalid.reason_code.as_deref(), Some("CORE_RESPONSE_INVALID"));
        let offline = diagnostic_probe("version", Err(ErrorDto::new("CORE_UNAVAILABLE", "离线", true)), Duration::ZERO);
        assert_eq!(offline.status, "UNAVAILABLE");
        assert_eq!(offline.reason_code.as_deref(), Some("CORE_UNAVAILABLE"));
    }
    #[test]
    fn ordered_simulation_stops_at_unresolved_rule() {
        let rules = parse_rules(&serde_json::json!({"rules":[
            {"type":"DOMAIN","payload":"other.example","proxy":"DIRECT"},
            {"type":"RULE-SET","payload":"unknown","proxy":"GROUP"},
            {"type":"DOMAIN-SUFFIX","payload":"example.com","proxy":"GROUP"},
            {"type":"MATCH","payload":"","proxy":"DIRECT"}
        ]})).unwrap();
        let result = simulate_domain(&rules, "a.example.com");
        assert_eq!(result.state, "UNSUPPORTED");
        assert_eq!(result.rule.unwrap().index, 1);
        assert!(!valid_hostname("-bad.example"));
    }
}
