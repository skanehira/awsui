/// ECRリポジトリのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub repository_name: String,
    pub repository_uri: String,
    pub registry_id: String,
    pub created_at: Option<String>,
    pub image_tag_mutability: String,
}

/// ECRイメージのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub image_digest: String,
    pub image_tags: Vec<String>,
    pub pushed_at: Option<String>,
    pub image_size_bytes: Option<i64>,
}
