use std::collections::HashMap;

/// Secrets Managerのシークレット（一覧用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Secret {
    pub name: String,
    pub arn: String,
    pub description: Option<String>,
    pub last_changed_date: Option<String>,
    pub last_accessed_date: Option<String>,
    pub tags: HashMap<String, String>,
}

/// シークレットのバージョン情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretVersion {
    pub version_id: String,
    pub staging_labels: Vec<String>,
}

/// Secrets Managerのシークレット詳細
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretDetail {
    pub name: String,
    pub arn: String,
    pub description: Option<String>,
    pub kms_key_id: Option<String>,
    pub rotation_enabled: bool,
    pub rotation_lambda_arn: Option<String>,
    pub rotation_days: Option<i64>,
    pub last_rotated_date: Option<String>,
    pub last_changed_date: Option<String>,
    pub last_accessed_date: Option<String>,
    pub created_date: Option<String>,
    pub tags: HashMap<String, String>,
    pub version_ids: Vec<String>,
    pub version_stages: Vec<SecretVersion>,
    pub secret_value: Option<String>,
}
