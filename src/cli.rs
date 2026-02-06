use clap::Parser;

/// awsui - TUI application for managing AWS resources via SSO
#[derive(Parser, Debug)]
#[command(name = "awsui")]
pub struct Cli {
    /// Allow delete operations. Without value: all services. With value: comma-separated service names (ec2,ecr,ecs,s3,vpc,secrets)
    #[arg(long, value_name = "SERVICES", num_args = 0..=1, default_missing_value = "__all__")]
    pub allow_delete: Option<String>,
}

/// 削除権限の状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletePermissions {
    /// 削除操作は一切許可されない
    None,
    /// 全サービスの削除操作を許可
    All,
    /// 指定されたサービスのみ削除許可
    Services(Vec<String>),
}

const VALID_SERVICES: &[&str] = &["ec2", "ecr", "ecs", "s3", "vpc", "secrets"];

impl DeletePermissions {
    /// CLI引数からDeletePermissionsを構築する
    pub fn from_cli(allow_delete: Option<&str>) -> Result<Self, String> {
        match allow_delete {
            None => Ok(DeletePermissions::None),
            Some("__all__") => Ok(DeletePermissions::All),
            Some(services_str) => {
                let services: Vec<String> = services_str
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();

                if services.is_empty() {
                    return Err("--allow-delete requires at least one service name".to_string());
                }

                for service in &services {
                    if !VALID_SERVICES.contains(&service.as_str()) {
                        return Err(format!(
                            "Invalid service name '{}'. Valid services: {}",
                            service,
                            VALID_SERVICES.join(", ")
                        ));
                    }
                }

                Ok(DeletePermissions::Services(services))
            }
        }
    }

    /// 指定サービスの削除が許可されているか
    pub fn can_delete(&self, service: &str) -> bool {
        match self {
            DeletePermissions::None => false,
            DeletePermissions::All => true,
            DeletePermissions::Services(services) => services.iter().any(|s| s == service),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ──────────────────────────────────────────────
    // DeletePermissions::from_cli テスト
    // ──────────────────────────────────────────────

    #[test]
    fn from_cli_returns_none_when_no_argument() {
        let result = DeletePermissions::from_cli(None).unwrap();
        assert_eq!(result, DeletePermissions::None);
    }

    #[test]
    fn from_cli_returns_all_when_flag_without_value() {
        let result = DeletePermissions::from_cli(Some("__all__")).unwrap();
        assert_eq!(result, DeletePermissions::All);
    }

    #[test]
    fn from_cli_returns_services_when_single_service() {
        let result = DeletePermissions::from_cli(Some("ec2")).unwrap();
        assert_eq!(result, DeletePermissions::Services(vec!["ec2".to_string()]));
    }

    #[test]
    fn from_cli_returns_services_when_multiple_services() {
        let result = DeletePermissions::from_cli(Some("ec2,s3,secrets")).unwrap();
        assert_eq!(
            result,
            DeletePermissions::Services(vec![
                "ec2".to_string(),
                "s3".to_string(),
                "secrets".to_string()
            ])
        );
    }

    #[test]
    fn from_cli_returns_error_when_invalid_service() {
        let result = DeletePermissions::from_cli(Some("ec2,invalid"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Invalid service name 'invalid'")
        );
    }

    #[test]
    fn from_cli_returns_services_when_uppercase_input() {
        let result = DeletePermissions::from_cli(Some("EC2,S3")).unwrap();
        assert_eq!(
            result,
            DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()])
        );
    }

    #[test]
    fn from_cli_returns_services_when_whitespace_around_names() {
        let result = DeletePermissions::from_cli(Some(" ec2 , s3 ")).unwrap();
        assert_eq!(
            result,
            DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()])
        );
    }

    #[test]
    fn from_cli_returns_error_when_empty_string() {
        let result = DeletePermissions::from_cli(Some(""));
        assert!(result.is_err());
    }

    // ──────────────────────────────────────────────
    // can_delete テスト
    // ──────────────────────────────────────────────

    #[test]
    fn can_delete_returns_false_when_permissions_none() {
        let perms = DeletePermissions::None;
        assert!(!perms.can_delete("ec2"));
        assert!(!perms.can_delete("s3"));
    }

    #[test]
    fn can_delete_returns_true_when_permissions_all() {
        let perms = DeletePermissions::All;
        assert!(perms.can_delete("ec2"));
        assert!(perms.can_delete("s3"));
        assert!(perms.can_delete("secrets"));
    }

    #[test]
    fn can_delete_returns_true_when_service_in_list() {
        let perms = DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()]);
        assert!(perms.can_delete("ec2"));
        assert!(perms.can_delete("s3"));
    }

    #[test]
    fn can_delete_returns_false_when_service_not_in_list() {
        let perms = DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()]);
        assert!(!perms.can_delete("ecs"));
        assert!(!perms.can_delete("secrets"));
    }

    // ──────────────────────────────────────────────
    // CLI引数パーステスト
    // ──────────────────────────────────────────────

    #[test]
    fn cli_parse_returns_none_when_no_allow_delete_flag() {
        let cli = Cli::parse_from(["awsui"]);
        assert!(cli.allow_delete.is_none());
    }

    #[test]
    fn cli_parse_returns_all_sentinel_when_flag_without_value() {
        let cli = Cli::parse_from(["awsui", "--allow-delete"]);
        assert_eq!(cli.allow_delete, Some("__all__".to_string()));
    }

    #[test]
    fn cli_parse_returns_services_when_flag_with_value() {
        let cli = Cli::parse_from(["awsui", "--allow-delete", "ec2,s3"]);
        assert_eq!(cli.allow_delete, Some("ec2,s3".to_string()));
    }
}
