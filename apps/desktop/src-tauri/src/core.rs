use crate::models::{ConnectionDto, ConnectionPageDto, CoreSnapshotDto, ErrorDto};
use serde_json::Value;
use std::{net::{Ipv4Addr, Ipv6Addr}, time::Duration};
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
    let connections = value.get("connections").and_then(Value::as_array)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "连接列表字段缺失", false))?;
    Ok(CoreSnapshotDto {
        state: "EXTERNAL_READONLY".into(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        core_version: core.version.clone(),
        connection_count: connections.len() as u64,
        upload_bytes: value.get("uploadTotal").and_then(Value::as_u64),
        download_bytes: value.get("downloadTotal").and_then(Value::as_u64),
    })
}

fn bounded_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.chars().take(256).collect())
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
            upload_bytes: entry.get("upload").and_then(Value::as_u64),
            download_bytes: entry.get("download").and_then(Value::as_u64) });
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
    }
}
