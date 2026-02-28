/// ECRリポジトリのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub repository_name: String,
    pub repository_uri: String,
    pub registry_id: String,
    pub created_at: Option<String>,
    pub image_tag_mutability: String,
}

/// ECR詳細画面のタブ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EcrDetailTab {
    Images,
    Scan,
    Lifecycle,
}

/// スキャン結果の重大度
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Informational,
    Undefined,
}

impl std::fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingSeverity::Critical => write!(f, "CRITICAL"),
            FindingSeverity::High => write!(f, "HIGH"),
            FindingSeverity::Medium => write!(f, "MEDIUM"),
            FindingSeverity::Low => write!(f, "LOW"),
            FindingSeverity::Informational => write!(f, "INFORMATIONAL"),
            FindingSeverity::Undefined => write!(f, "UNDEFINED"),
        }
    }
}

/// スキャン結果の個別脆弱性
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFinding {
    pub name: String,
    pub severity: FindingSeverity,
    pub description: String,
    pub uri: String,
}

/// イメージスキャン結果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageScanResult {
    pub findings: Vec<ScanFinding>,
    pub severity_counts: Vec<(FindingSeverity, i64)>,
}

/// ECRイメージのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub image_digest: String,
    pub image_tags: Vec<String>,
    pub pushed_at: Option<String>,
    pub image_size_bytes: Option<i64>,
}
