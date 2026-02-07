use crate::error::AppError;
use std::path::PathBuf;

/// SSOプロファイル情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsoProfile {
    pub name: String,
    pub region: Option<String>,
    pub sso_start_url: String,
    pub sso_session: Option<String>,
}

/// ~/.aws/config からSSOプロファイル一覧を読み込む
pub fn load_sso_profiles() -> Result<Vec<SsoProfile>, AppError> {
    let config_path = aws_config_path()?;
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        AppError::Config(format!("Failed to read {}: {}", config_path.display(), e))
    })?;
    Ok(parse_sso_profiles(&content))
}

fn aws_config_path() -> Result<PathBuf, AppError> {
    if let Ok(path) = std::env::var("AWS_CONFIG_FILE") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var("HOME").map_err(|_| AppError::Config("HOME not set".to_string()))?;
    Ok(PathBuf::from(home).join(".aws").join("config"))
}

fn parse_sso_profiles(content: &str) -> Vec<SsoProfile> {
    // 1. sso-session セクションを先にパース
    let sso_sessions = parse_sso_sessions(content);

    // 2. プロファイルをパース
    let mut profiles = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_region: Option<String> = None;
    let mut current_sso_url: Option<String> = None;
    let mut current_sso_session: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            // 前のプロファイルを保存
            flush_profile(
                &mut profiles,
                &sso_sessions,
                current_name.take(),
                current_region.take(),
                current_sso_url.take(),
                current_sso_session.take(),
            );

            let section = line.trim_matches(|c| c == '[' || c == ']');
            current_name = if let Some(name) = section.strip_prefix("profile ") {
                Some(name.trim().to_string())
            } else if section == "default" {
                Some("default".to_string())
            } else {
                None
            };
            continue;
        }

        if current_name.is_some()
            && let Some((key, value)) = line.split_once('=')
        {
            let key = key.trim();
            let value = value.trim();
            match key {
                "region" => current_region = Some(value.to_string()),
                "sso_start_url" => current_sso_url = Some(value.to_string()),
                "sso_session" => current_sso_session = Some(value.to_string()),
                _ => {}
            }
        }
    }

    // 最後のプロファイルを保存
    flush_profile(
        &mut profiles,
        &sso_sessions,
        current_name,
        current_region,
        current_sso_url,
        current_sso_session,
    );

    profiles
}

/// sso-session セクション情報
struct SsoSessionInfo {
    sso_start_url: String,
    sso_region: Option<String>,
}

/// [sso-session xxx] セクションをパースしてマップを返す
fn parse_sso_sessions(content: &str) -> std::collections::HashMap<String, SsoSessionInfo> {
    let mut sessions = std::collections::HashMap::new();
    let mut current_session: Option<String> = None;
    let mut current_url: Option<String> = None;
    let mut current_region: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            if let (Some(name), Some(url)) = (current_session.take(), current_url.take()) {
                sessions.insert(
                    name,
                    SsoSessionInfo {
                        sso_start_url: url,
                        sso_region: current_region.take(),
                    },
                );
            }
            current_region = None;
            current_url = None;

            let section = line.trim_matches(|c| c == '[' || c == ']');
            current_session = section
                .strip_prefix("sso-session ")
                .map(|s| s.trim().to_string());
            continue;
        }

        if current_session.is_some()
            && let Some((key, value)) = line.split_once('=')
        {
            let key = key.trim();
            let value = value.trim();
            match key {
                "sso_start_url" => current_url = Some(value.to_string()),
                "sso_region" => current_region = Some(value.to_string()),
                _ => {}
            }
        }
    }

    if let (Some(name), Some(url)) = (current_session, current_url) {
        sessions.insert(
            name,
            SsoSessionInfo {
                sso_start_url: url,
                sso_region: current_region,
            },
        );
    }

    sessions
}

/// プロファイル情報をprofilesに追加する
fn flush_profile(
    profiles: &mut Vec<SsoProfile>,
    sso_sessions: &std::collections::HashMap<String, SsoSessionInfo>,
    name: Option<String>,
    region: Option<String>,
    sso_url: Option<String>,
    sso_session: Option<String>,
) {
    let Some(name) = name else { return };

    // sso_start_url が直接ある場合はそのまま使用
    if let Some(url) = sso_url {
        profiles.push(SsoProfile {
            name,
            region,
            sso_start_url: url,
            sso_session,
        });
        return;
    }

    // sso_session 経由で sso_start_url を解決
    if let Some(session_name) = sso_session
        && let Some(session) = sso_sessions.get(&session_name)
    {
        profiles.push(SsoProfile {
            name,
            region: region.or_else(|| session.sso_region.clone()),
            sso_start_url: session.sso_start_url.clone(),
            sso_session: Some(session_name),
        });
    }
}

/// プロファイル名からリージョンを取得する
pub fn get_region_for_profile(profiles: &[SsoProfile], profile_name: &str) -> Option<String> {
    profiles
        .iter()
        .find(|p| p.name == profile_name)
        .and_then(|p| p.region.clone())
}

/// プロファイル名の一覧を返す
pub fn profile_names(profiles: &[SsoProfile]) -> Vec<String> {
    profiles.iter().map(|p| p.name.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sso_profiles_returns_profiles_when_valid_config() {
        let content = r#"
[default]
region = us-east-1

[profile dev-account]
sso_start_url = https://my-sso.awsapps.com/start
region = ap-northeast-1

[profile staging]
sso_start_url = https://my-sso.awsapps.com/start
region = us-west-2

[profile no-sso]
region = eu-west-1
"#;
        let profiles = parse_sso_profiles(content);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name, "dev-account");
        assert_eq!(profiles[0].region.as_deref(), Some("ap-northeast-1"));
        assert!(profiles[0].sso_session.is_none());
        assert_eq!(profiles[1].name, "staging");
        assert_eq!(profiles[1].region.as_deref(), Some("us-west-2"));
        assert!(profiles[1].sso_session.is_none());
    }

    #[test]
    fn parse_sso_profiles_returns_empty_when_no_sso_profiles() {
        let content = r#"
[default]
region = us-east-1

[profile regular]
region = eu-west-1
"#;
        let profiles = parse_sso_profiles(content);
        assert!(profiles.is_empty());
    }

    #[test]
    fn parse_sso_profiles_returns_profile_without_region_when_region_missing() {
        let content = r#"
[profile sso-no-region]
sso_start_url = https://my-sso.awsapps.com/start
"#;
        let profiles = parse_sso_profiles(content);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "sso-no-region");
        assert!(profiles[0].region.is_none());
    }

    #[test]
    fn get_region_for_profile_returns_region_when_profile_exists() {
        let profiles = vec![SsoProfile {
            name: "dev".to_string(),
            region: Some("ap-northeast-1".to_string()),
            sso_start_url: "https://example.com".to_string(),
            sso_session: None,
        }];
        assert_eq!(
            get_region_for_profile(&profiles, "dev"),
            Some("ap-northeast-1".to_string())
        );
    }

    #[test]
    fn get_region_for_profile_returns_none_when_profile_not_found() {
        let profiles = vec![];
        assert_eq!(get_region_for_profile(&profiles, "nonexistent"), None);
    }

    #[test]
    fn profile_names_returns_names_when_profiles_exist() {
        let profiles = vec![
            SsoProfile {
                name: "dev".to_string(),
                region: None,
                sso_start_url: "https://example.com".to_string(),
                sso_session: None,
            },
            SsoProfile {
                name: "staging".to_string(),
                region: None,
                sso_start_url: "https://example.com".to_string(),
                sso_session: None,
            },
        ];
        assert_eq!(profile_names(&profiles), vec!["dev", "staging"]);
    }

    #[test]
    fn parse_sso_profiles_returns_profiles_when_sso_session_format() {
        let content = r#"
[sso-session my-sso]
sso_region = ap-northeast-1
sso_start_url = https://my-sso.awsapps.com/start
sso_registration_scopes = sso:account:access

[profile dev]
sso_session = my-sso
sso_account_id = 123456789012
sso_role_name = DevAccess

[profile staging]
sso_session = my-sso
sso_account_id = 987654321098
sso_role_name = StagingAccess
region = us-west-2
"#;
        let profiles = parse_sso_profiles(content);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name, "dev");
        assert_eq!(
            profiles[0].sso_start_url,
            "https://my-sso.awsapps.com/start"
        );
        // region未指定の場合、sso_sessionのsso_regionにフォールバック
        assert_eq!(profiles[0].region.as_deref(), Some("ap-northeast-1"));
        assert_eq!(profiles[0].sso_session.as_deref(), Some("my-sso"));
        assert_eq!(profiles[1].name, "staging");
        // profile側にregionがあればそちらを優先
        assert_eq!(profiles[1].region.as_deref(), Some("us-west-2"));
        assert_eq!(profiles[1].sso_session.as_deref(), Some("my-sso"));
    }

    #[test]
    fn parse_sso_profiles_returns_default_when_default_has_sso_session() {
        let content = r#"
[default]
region = ap-northeast-1
sso_session = my-sso
sso_account_id = 123456789012
sso_role_name = DevAccess

[sso-session my-sso]
sso_region = ap-northeast-1
sso_start_url = https://my-sso.awsapps.com/start
"#;
        let profiles = parse_sso_profiles(content);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "default");
        assert_eq!(
            profiles[0].sso_start_url,
            "https://my-sso.awsapps.com/start"
        );
    }

    #[test]
    fn parse_sso_profiles_returns_mixed_when_both_formats() {
        let content = r#"
[sso-session my-sso]
sso_start_url = https://my-sso.awsapps.com/start
sso_region = us-east-1

[profile session-based]
sso_session = my-sso
sso_account_id = 111111111111

[profile direct-url]
sso_start_url = https://other.awsapps.com/start
region = eu-west-1

[profile no-sso]
region = ap-southeast-1
"#;
        let profiles = parse_sso_profiles(content);
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].name, "session-based");
        assert_eq!(
            profiles[0].sso_start_url,
            "https://my-sso.awsapps.com/start"
        );
        assert_eq!(profiles[1].name, "direct-url");
        assert_eq!(profiles[1].sso_start_url, "https://other.awsapps.com/start");
    }
}
