use async_trait::async_trait;

use crate::aws::ecr_model::{Image, ImageScanResult, Repository};
use crate::error::{AppError, format_error_chain};

#[cfg(test)]
use mockall::automock;

/// ECR APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait EcrClient: Send + Sync {
    async fn describe_repositories(&self) -> Result<Vec<Repository>, AppError>;
    async fn list_images(&self, repository_name: &str) -> Result<Vec<Image>, AppError>;
    async fn create_repository(
        &self,
        repository_name: &str,
        image_tag_mutability: &str,
    ) -> Result<(), AppError>;
    async fn delete_repository(&self, repository_name: &str) -> Result<(), AppError>;
    async fn delete_images(
        &self,
        repository_name: &str,
        image_digests: &[String],
    ) -> Result<(), AppError>;
    /// ライフサイクルポリシーのJSON文字列を取得。未設定の場合はNone。
    async fn get_lifecycle_policy(&self, repository_name: &str)
    -> Result<Option<String>, AppError>;
    /// イメージスキャン結果を取得。スキャン未実施の場合はNone。
    async fn describe_image_scan_findings(
        &self,
        repository_name: &str,
        image_digest: &str,
    ) -> Result<Option<ImageScanResult>, AppError>;
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

    async fn create_repository(
        &self,
        repository_name: &str,
        image_tag_mutability: &str,
    ) -> Result<(), AppError> {
        let mutability = match image_tag_mutability {
            "IMMUTABLE" => aws_sdk_ecr::types::ImageTagMutability::Immutable,
            _ => aws_sdk_ecr::types::ImageTagMutability::Mutable,
        };
        self.client
            .create_repository()
            .repository_name(repository_name)
            .image_tag_mutability(mutability)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn delete_repository(&self, repository_name: &str) -> Result<(), AppError> {
        self.client
            .delete_repository()
            .repository_name(repository_name)
            .force(false)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn delete_images(
        &self,
        repository_name: &str,
        image_digests: &[String],
    ) -> Result<(), AppError> {
        let image_ids: Vec<_> = image_digests
            .iter()
            .map(|d| {
                aws_sdk_ecr::types::ImageIdentifier::builder()
                    .image_digest(d)
                    .build()
            })
            .collect();
        self.client
            .batch_delete_image()
            .repository_name(repository_name)
            .set_image_ids(Some(image_ids))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn get_lifecycle_policy(
        &self,
        repository_name: &str,
    ) -> Result<Option<String>, AppError> {
        match self
            .client
            .get_lifecycle_policy()
            .repository_name(repository_name)
            .send()
            .await
        {
            Ok(resp) => Ok(resp.lifecycle_policy_text().map(String::from)),
            Err(e) => {
                let service_err = e.as_service_error();
                if service_err.is_some_and(|se| se.is_lifecycle_policy_not_found_exception()) {
                    Ok(None)
                } else {
                    Err(AppError::AwsApi(format_error_chain(&e)))
                }
            }
        }
    }

    async fn describe_image_scan_findings(
        &self,
        repository_name: &str,
        image_digest: &str,
    ) -> Result<Option<ImageScanResult>, AppError> {
        let image_id = aws_sdk_ecr::types::ImageIdentifier::builder()
            .image_digest(image_digest)
            .build();
        match self
            .client
            .describe_image_scan_findings()
            .repository_name(repository_name)
            .image_id(image_id)
            .send()
            .await
        {
            Ok(resp) => {
                let scan_findings = resp.image_scan_findings().map(convert_scan_findings);
                Ok(scan_findings)
            }
            Err(e) => {
                let service_err = e.as_service_error();
                if service_err.is_some_and(|se| se.is_scan_not_found_exception()) {
                    Ok(None)
                } else {
                    Err(AppError::AwsApi(format_error_chain(&e)))
                }
            }
        }
    }
}

/// SDK の ImageScanFindings → ドメインの ImageScanResult
fn convert_scan_findings(sdk: &aws_sdk_ecr::types::ImageScanFindings) -> ImageScanResult {
    use crate::aws::ecr_model::{FindingSeverity, ScanFinding};

    let findings: Vec<ScanFinding> = sdk
        .findings()
        .iter()
        .map(|f| {
            let severity = match f.severity() {
                Some(aws_sdk_ecr::types::FindingSeverity::Critical) => FindingSeverity::Critical,
                Some(aws_sdk_ecr::types::FindingSeverity::High) => FindingSeverity::High,
                Some(aws_sdk_ecr::types::FindingSeverity::Medium) => FindingSeverity::Medium,
                Some(aws_sdk_ecr::types::FindingSeverity::Low) => FindingSeverity::Low,
                Some(aws_sdk_ecr::types::FindingSeverity::Informational) => {
                    FindingSeverity::Informational
                }
                _ => FindingSeverity::Undefined,
            };
            ScanFinding {
                name: f.name().unwrap_or_default().to_string(),
                severity,
                description: f.description().unwrap_or_default().to_string(),
                uri: f.uri().unwrap_or_default().to_string(),
            }
        })
        .collect();

    let severity_counts: Vec<(FindingSeverity, i64)> = sdk
        .finding_severity_counts()
        .map(|counts| {
            let mut result = Vec::new();
            let severity_order = [
                (
                    aws_sdk_ecr::types::FindingSeverity::Critical,
                    FindingSeverity::Critical,
                ),
                (
                    aws_sdk_ecr::types::FindingSeverity::High,
                    FindingSeverity::High,
                ),
                (
                    aws_sdk_ecr::types::FindingSeverity::Medium,
                    FindingSeverity::Medium,
                ),
                (
                    aws_sdk_ecr::types::FindingSeverity::Low,
                    FindingSeverity::Low,
                ),
                (
                    aws_sdk_ecr::types::FindingSeverity::Informational,
                    FindingSeverity::Informational,
                ),
                (
                    aws_sdk_ecr::types::FindingSeverity::Undefined,
                    FindingSeverity::Undefined,
                ),
            ];
            for (sdk_sev, domain_sev) in severity_order {
                if let Some(&count) = counts.get(&sdk_sev)
                    && count > 0
                {
                    result.push((domain_sev, count as i64));
                }
            }
            result
        })
        .unwrap_or_default();

    ImageScanResult {
        findings,
        severity_counts,
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
