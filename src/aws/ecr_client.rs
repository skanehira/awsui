use async_trait::async_trait;

use crate::aws::ecr_model::{Image, Repository};
use crate::error::{AppError, format_error_chain};

#[cfg(test)]
use mockall::automock;

/// ECR APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait EcrClient: Send + Sync {
    async fn describe_repositories(&self) -> Result<Vec<Repository>, AppError>;
    async fn list_images(&self, repository_name: &str) -> Result<Vec<Image>, AppError>;
}

/// AWS SDK ECRクライアントの実装
pub struct AwsEcrClient {
    client: aws_sdk_ecr::Client,
}

impl AwsEcrClient {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_ecr::config::Region::new(region.to_string()))
            .load()
            .await;
        let client = aws_sdk_ecr::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl EcrClient for AwsEcrClient {
    async fn describe_repositories(&self) -> Result<Vec<Repository>, AppError> {
        let mut repositories = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.describe_repositories();
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

            for sdk_repo in resp.repositories() {
                repositories.push(convert_repository(sdk_repo));
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(repositories)
    }

    async fn list_images(&self, repository_name: &str) -> Result<Vec<Image>, AppError> {
        let mut images = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .describe_images()
                .repository_name(repository_name);
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

            for sdk_image in resp.image_details() {
                images.push(convert_image(sdk_image));
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(images)
    }
}

/// SDK の Repository → ドメインの Repository
fn convert_repository(sdk: &aws_sdk_ecr::types::Repository) -> Repository {
    let created_at = sdk.created_at().map(|t| {
        t.fmt(aws_sdk_ecr::primitives::DateTimeFormat::DateTime)
            .unwrap_or_default()
    });

    let image_tag_mutability = sdk
        .image_tag_mutability()
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    Repository {
        repository_name: sdk.repository_name().unwrap_or_default().to_string(),
        repository_uri: sdk.repository_uri().unwrap_or_default().to_string(),
        registry_id: sdk.registry_id().unwrap_or_default().to_string(),
        created_at,
        image_tag_mutability,
    }
}

/// SDK の ImageDetail → ドメインの Image
fn convert_image(sdk: &aws_sdk_ecr::types::ImageDetail) -> Image {
    let image_tags = sdk.image_tags().iter().map(|t| t.to_string()).collect();

    let pushed_at = sdk.image_pushed_at().map(|t| {
        t.fmt(aws_sdk_ecr::primitives::DateTimeFormat::DateTime)
            .unwrap_or_default()
    });

    Image {
        image_digest: sdk.image_digest().unwrap_or_default().to_string(),
        image_tags,
        pushed_at,
        image_size_bytes: sdk.image_size_in_bytes(),
    }
}
