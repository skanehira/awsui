use crate::error::AppError;
use std::path::PathBuf;

/// SSOプロファイル情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsoProfile {
    pub name: String,
    pub region: Option<String>,
    pub sso_start_url: String,
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
    let mut profiles = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_region: Option<String> = None;
    let mut current_sso_url: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('[') && line.ends_with(']') {
            // 前のプロファイルを保存
            if let (Some(name), Some(sso_url)) = (current_name.take(), current_sso_url.take()) {
                profiles.push(SsoProfile {
                    name,
                    region: current_region.take(),
                    sso_start_url: sso_url,
                });
            }
            current_region = None;
            current_sso_url = None;

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
                _ => {}
            }
        }
    }

    // 最後のプロファイルを保存
    if let (Some(name), Some(sso_url)) = (current_name, current_sso_url) {
        profiles.push(SsoProfile {
            name,
            region: current_region,
            sso_start_url: sso_url,
        });
    }

    profiles
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
        assert_eq!(profiles[1].name, "staging");
        assert_eq!(profiles[1].region.as_deref(), Some("us-west-2"));
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
            },
            SsoProfile {
                name: "staging".to_string(),
                region: None,
                sso_start_url: "https://example.com".to_string(),
            },
        ];
        assert_eq!(profile_names(&profiles), vec!["dev", "staging"]);
    }
}
