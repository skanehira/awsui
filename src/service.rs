use serde::{Deserialize, Serialize};

/// AWSサービスの種別
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceKind {
    Ec2,
    Ecr,
    Ecs,
    S3,
    Vpc,
    SecretsManager,
}

impl ServiceKind {
    /// 全サービスの一覧（表示順）
    pub const ALL: &[ServiceKind] = &[
        ServiceKind::Ec2,
        ServiceKind::Ecr,
        ServiceKind::Ecs,
        ServiceKind::S3,
        ServiceKind::Vpc,
        ServiceKind::SecretsManager,
    ];

    /// CLI引数用のサービス名（--allow-delete等で使用）
    pub fn cli_name(&self) -> &'static str {
        match self {
            ServiceKind::Ec2 => "ec2",
            ServiceKind::Ecr => "ecr",
            ServiceKind::Ecs => "ecs",
            ServiceKind::S3 => "s3",
            ServiceKind::Vpc => "vpc",
            ServiceKind::SecretsManager => "secrets",
        }
    }

    /// 短縮名（タブバー等で使用）
    pub fn short_name(&self) -> &'static str {
        match self {
            ServiceKind::Ec2 => "EC2",
            ServiceKind::Ecr => "ECR",
            ServiceKind::Ecs => "ECS",
            ServiceKind::S3 => "S3",
            ServiceKind::Vpc => "VPC",
            ServiceKind::SecretsManager => "Secrets Manager",
        }
    }

    /// フルネーム（ダッシュボード等で使用）
    pub fn full_name(&self) -> &'static str {
        match self {
            ServiceKind::Ec2 => "Elastic Compute Cloud (EC2)",
            ServiceKind::Ecr => "Elastic Container Registry (ECR)",
            ServiceKind::Ecs => "Elastic Container Service (ECS)",
            ServiceKind::S3 => "Simple Storage Service (S3)",
            ServiceKind::Vpc => "Virtual Private Cloud (VPC)",
            ServiceKind::SecretsManager => "Secrets Manager",
        }
    }
}

impl std::fmt::Display for ServiceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short_name())
    }
}
