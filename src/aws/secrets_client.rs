use std::collections::HashMap;

use async_trait::async_trait;

use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::error::AppError;

#[cfg(test)]
use mockall::automock;

/// Secrets Manager APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait SecretsClient: Send + Sync {
    async fn list_secrets(&self) -> Result<Vec<Secret>, AppError>;
    async fn describe_secret(&self, secret_id: &str) -> Result<SecretDetail, AppError>;
    async fn create_secret(
        &self,
        name: &str,
        value: &str,
        description: Option<String>,
    ) -> Result<(), AppError>;
    async fn update_secret_value(&self, secret_id: &str, value: &str) -> Result<(), AppError>;
    async fn delete_secret(&self, secret_id: &str) -> Result<(), AppError>;
}

/// AWS SDK Secrets Managerクライアントの実装
pub struct AwsSecretsClient {
    client: aws_sdk_secretsmanager::Client,
}

impl AwsSecretsClient {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_secretsmanager::config::Region::new(
                region.to_string(),
            ))
            .load()
            .await;
        let client = aws_sdk_secretsmanager::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl SecretsClient for AwsSecretsClient {
    async fn list_secrets(&self) -> Result<Vec<Secret>, AppError> {
        let mut secrets = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_secrets();
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            for sdk_secret in resp.secret_list() {
                secrets.push(convert_secret(sdk_secret));
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(secrets)
    }

    async fn describe_secret(&self, secret_id: &str) -> Result<SecretDetail, AppError> {
        let resp = self
            .client
            .describe_secret()
            .secret_id(secret_id)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;

        Ok(convert_secret_detail(&resp))
    }

    async fn create_secret(
        &self,
        name: &str,
        value: &str,
        description: Option<String>,
    ) -> Result<(), AppError> {
        let mut req = self.client.create_secret().name(name).secret_string(value);
        if let Some(desc) = description {
            req = req.description(desc);
        }
        req.send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn update_secret_value(&self, secret_id: &str, value: &str) -> Result<(), AppError> {
        self.client
            .put_secret_value()
            .secret_id(secret_id)
            .secret_string(value)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn delete_secret(&self, secret_id: &str) -> Result<(), AppError> {
        self.client
            .delete_secret()
            .secret_id(secret_id)
            .force_delete_without_recovery(true)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }
}

/// SDK の SecretListEntry → ドメインの Secret
fn convert_secret(sdk: &aws_sdk_secretsmanager::types::SecretListEntry) -> Secret {
    let tags: HashMap<String, String> = sdk
        .tags()
        .iter()
        .filter_map(|t| {
            let key = t.key()?.to_string();
            let value = t.value()?.to_string();
            Some((key, value))
        })
        .collect();

    Secret {
        name: sdk.name().unwrap_or_default().to_string(),
        arn: sdk.arn().unwrap_or_default().to_string(),
        description: sdk.description().map(String::from),
        last_changed_date: sdk.last_changed_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        last_accessed_date: sdk.last_accessed_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        tags,
    }
}

/// SDK の DescribeSecretOutput → ドメインの SecretDetail
fn convert_secret_detail(
    sdk: &aws_sdk_secretsmanager::operation::describe_secret::DescribeSecretOutput,
) -> SecretDetail {
    let tags: HashMap<String, String> = sdk
        .tags()
        .iter()
        .filter_map(|t| {
            let key = t.key()?.to_string();
            let value = t.value()?.to_string();
            Some((key, value))
        })
        .collect();

    let version_ids: Vec<String> = sdk
        .version_ids_to_stages()
        .map(|v| v.keys().cloned().collect())
        .unwrap_or_default();

    SecretDetail {
        name: sdk.name().unwrap_or_default().to_string(),
        arn: sdk.arn().unwrap_or_default().to_string(),
        description: sdk.description().map(String::from),
        kms_key_id: sdk.kms_key_id().map(String::from),
        rotation_enabled: sdk.rotation_enabled().unwrap_or(false),
        rotation_lambda_arn: sdk.rotation_lambda_arn().map(String::from),
        last_rotated_date: sdk.last_rotated_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        last_changed_date: sdk.last_changed_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        last_accessed_date: sdk.last_accessed_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        created_date: sdk.created_date().map(|t| {
            t.fmt(aws_sdk_secretsmanager::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        tags,
        version_ids,
    }
}
