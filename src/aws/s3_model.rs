/// S3バケットのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bucket {
    pub name: String,
    pub creation_date: Option<String>,
}

/// S3オブジェクトのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Object {
    pub key: String,
    pub size: Option<i64>,
    pub last_modified: Option<String>,
    pub storage_class: Option<String>,
    pub is_prefix: bool,
}
