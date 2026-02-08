use crate::config::SsoProfile;
use std::path::PathBuf;

/// SSOトークンの状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SsoTokenStatus {
    /// トークンが有効
    Valid,
    /// トークンが期限切れ
    Expired,
    /// キャッシュファイルが見つからない
    NotFound,
}

/// SSOキャッシュファイルのパスを計算する
fn cache_file_path(profile: &SsoProfile) -> PathBuf {
    let hash_input = profile
        .sso_session
        .as_deref()
        .unwrap_or(&profile.sso_start_url);
    let hash = sha1_smol::Sha1::from(hash_input).digest().to_string();

    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".aws")
        .join("sso")
        .join("cache")
        .join(format!("{}.json", hash))
}

/// プロファイルのSSOトークン状態をチェックする
pub fn check_sso_token(profile: &SsoProfile) -> SsoTokenStatus {
    let path = cache_file_path(profile);

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return SsoTokenStatus::NotFound,
    };

    parse_token_status(&content)
}

fn parse_token_status(content: &str) -> SsoTokenStatus {
    // expiresAt フィールドを簡易的に抽出（JSON パーサー不要）
    let Some(expires_at) = extract_expires_at(content) else {
        return SsoTokenStatus::NotFound;
    };

    // RFC3339形式をパース: "2026-02-07T04:16:52Z" or "2026-02-07T04:16:52UTC"
    let Some(expires) = parse_rfc3339(&expires_at) else {
        return SsoTokenStatus::NotFound;
    };

    let now = std::time::SystemTime::now();
    if expires > now {
        SsoTokenStatus::Valid
    } else {
        SsoTokenStatus::Expired
    }
}

/// JSON文字列から expiresAt の値を抽出する
fn extract_expires_at(content: &str) -> Option<String> {
    // "expiresAt" : "..." のパターンを検索
    let key = "\"expiresAt\"";
    let idx = content.find(key)?;
    let after_key = &content[idx + key.len()..];
    // コロンの後の引用符を探す
    let quote_start = after_key.find('"')? + 1;
    let rest = &after_key[quote_start..];
    let quote_end = rest.find('"')?;
    Some(rest[..quote_end].to_string())
}

/// RFC3339形式の日時文字列をSystemTimeにパースする
fn parse_rfc3339(s: &str) -> Option<std::time::SystemTime> {
    // "2026-02-07T04:16:52Z" or "2026-02-07T04:16:52UTC"
    let s = s.trim_end_matches("UTC").trim_end_matches('Z');
    let (date, time) = s.split_once('T')?;
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i64 = parts[0].parse().ok()?;
    let month: i64 = parts[1].parse().ok()?;
    let day: i64 = parts[2].parse().ok()?;

    let time_parts: Vec<&str> = time.split(':').collect();
    if time_parts.len() != 3 {
        return None;
    }
    let hour: i64 = time_parts[0].parse().ok()?;
    let min: i64 = time_parts[1].parse().ok()?;
    let sec: i64 = time_parts[2].parse().ok()?;

    // Unix epoch からの秒数を計算
    let days = days_from_civil(year, month, day);
    let total_secs = days * 86400 + hour * 3600 + min * 60 + sec;

    if total_secs >= 0 {
        Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(total_secs as u64))
    } else {
        None
    }
}

/// 年月日からUnix epoch (1970-01-01) からの日数を計算する
/// Howard Hinnant's algorithm
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = month as u64;
    let doy = if m > 2 {
        (153 * (m - 3) + 2) / 5 + (day as u64) - 1
    } else {
        (153 * (m + 9) + 2) / 5 + (day as u64) - 1
    };
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe as i64) - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_file_path_uses_sso_session_when_present() {
        let profile = SsoProfile {
            name: "dev".to_string(),
            region: None,
            sso_start_url: "https://my-sso.awsapps.com/start".to_string(),
            sso_session: Some("my-sso".to_string()),
        };
        let path = cache_file_path(&profile);
        let expected_hash = sha1_smol::Sha1::from("my-sso").digest().to_string();
        assert!(path.ends_with(format!("{}.json", expected_hash)));
    }

    #[test]
    fn cache_file_path_uses_sso_start_url_when_no_session() {
        let profile = SsoProfile {
            name: "dev".to_string(),
            region: None,
            sso_start_url: "https://my-sso.awsapps.com/start".to_string(),
            sso_session: None,
        };
        let path = cache_file_path(&profile);
        let expected_hash = sha1_smol::Sha1::from("https://my-sso.awsapps.com/start")
            .digest()
            .to_string();
        assert!(path.ends_with(format!("{}.json", expected_hash)));
    }

    #[test]
    fn extract_expires_at_returns_value_when_valid_json() {
        let content = r#"{"expiresAt": "2026-02-07T04:16:52Z", "accessToken": "xxx"}"#;
        assert_eq!(
            extract_expires_at(content),
            Some("2026-02-07T04:16:52Z".to_string())
        );
    }

    #[test]
    fn extract_expires_at_returns_none_when_no_field() {
        let content = r#"{"accessToken": "xxx"}"#;
        assert_eq!(extract_expires_at(content), None);
    }

    #[test]
    fn parse_rfc3339_returns_some_when_valid() {
        let result = parse_rfc3339("2026-02-07T04:16:52Z");
        assert!(result.is_some());
    }

    #[test]
    fn parse_rfc3339_returns_some_when_utc_suffix() {
        let result = parse_rfc3339("2026-02-07T04:16:52UTC");
        assert!(result.is_some());
    }

    #[test]
    fn parse_rfc3339_returns_none_when_invalid() {
        assert!(parse_rfc3339("not-a-date").is_none());
    }

    #[test]
    fn parse_token_status_returns_valid_when_future_expiry() {
        // 2099年の未来日時
        let content = r#"{"expiresAt": "2099-12-31T23:59:59Z", "accessToken": "xxx"}"#;
        assert_eq!(parse_token_status(content), SsoTokenStatus::Valid);
    }

    #[test]
    fn parse_token_status_returns_expired_when_past_expiry() {
        // 2020年の過去日時
        let content = r#"{"expiresAt": "2020-01-01T00:00:00Z", "accessToken": "xxx"}"#;
        assert_eq!(parse_token_status(content), SsoTokenStatus::Expired);
    }

    #[test]
    fn parse_token_status_returns_not_found_when_no_expires_at() {
        let content = r#"{"accessToken": "xxx"}"#;
        assert_eq!(parse_token_status(content), SsoTokenStatus::NotFound);
    }

    #[test]
    fn check_sso_token_returns_not_found_when_no_cache_file() {
        let profile = SsoProfile {
            name: "nonexistent".to_string(),
            region: None,
            sso_start_url: "https://nonexistent.example.com".to_string(),
            sso_session: None,
        };
        assert_eq!(check_sso_token(&profile), SsoTokenStatus::NotFound);
    }
}
