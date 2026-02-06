use async_trait::async_trait;

use crate::aws::s3_model::{Bucket, S3Object};
use crate::error::AppError;

#[cfg(test)]
use mockall::automock;

/// S3 APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait S3Client: Send + Sync {
    async fn list_buckets(&self) -> Result<Vec<Bucket>, AppError>;
    async fn list_objects(
        &self,
        bucket_name: &str,
        prefix: Option<String>,
    ) -> Result<Vec<S3Object>, AppError>;
    async fn create_bucket(&self, bucket_name: &str) -> Result<(), AppError>;
    async fn delete_bucket(&self, bucket_name: &str) -> Result<(), AppError>;
    async fn delete_object(&self, bucket_name: &str, key: &str) -> Result<(), AppError>;
}

/// AWS SDK S3クライアントの実装
pub struct AwsS3Client {
    client: aws_sdk_s3::Client,
}

impl AwsS3Client {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_s3::config::Region::new(region.to_string()))
            .load()
            .await;
        let client = aws_sdk_s3::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl S3Client for AwsS3Client {
    async fn list_buckets(&self) -> Result<Vec<Bucket>, AppError> {
        let resp = self
            .client
            .list_buckets()
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;

        let buckets = resp.buckets().iter().map(convert_bucket).collect();

        Ok(buckets)
    }

    async fn list_objects(
        &self,
        bucket_name: &str,
        prefix: Option<String>,
    ) -> Result<Vec<S3Object>, AppError> {
        let mut objects = Vec::new();
        let mut continuation_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_objects_v2()
                .bucket(bucket_name)
                .delimiter("/");

            if let Some(p) = &prefix {
                req = req.prefix(p);
            }
            if let Some(token) = &continuation_token {
                req = req.continuation_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            // 共通プレフィックス（ディレクトリ）
            for cp in resp.common_prefixes() {
                if let Some(prefix_str) = cp.prefix() {
                    objects.push(S3Object {
                        key: prefix_str.to_string(),
                        size: None,
                        last_modified: None,
                        storage_class: None,
                        is_prefix: true,
                    });
                }
            }

            // オブジェクト
            for obj in resp.contents() {
                objects.push(convert_object(obj));
            }

            if resp.is_truncated() == Some(true) {
                continuation_token = resp.next_continuation_token().map(String::from);
            } else {
                break;
            }
        }

        Ok(objects)
    }

    async fn create_bucket(&self, bucket_name: &str) -> Result<(), AppError> {
        self.client
            .create_bucket()
            .bucket(bucket_name)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn delete_bucket(&self, bucket_name: &str) -> Result<(), AppError> {
        self.client
            .delete_bucket()
            .bucket(bucket_name)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn delete_object(&self, bucket_name: &str, key: &str) -> Result<(), AppError> {
        self.client
            .delete_object()
            .bucket(bucket_name)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }
}

/// SDK の Bucket → ドメインの Bucket
fn convert_bucket(sdk: &aws_sdk_s3::types::Bucket) -> Bucket {
    Bucket {
        name: sdk.name().unwrap_or_default().to_string(),
        creation_date: sdk.creation_date().map(|d| {
            d.fmt(aws_sdk_s3::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
    }
}

/// SDK の Object → ドメインの S3Object
fn convert_object(sdk: &aws_sdk_s3::types::Object) -> S3Object {
    S3Object {
        key: sdk.key().unwrap_or_default().to_string(),
        size: sdk.size(),
        last_modified: sdk.last_modified().map(|d| {
            d.fmt(aws_sdk_s3::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        storage_class: sdk.storage_class().map(|s| s.as_str().to_string()),
        is_prefix: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_bucket_returns_name_when_name_set() {
        let sdk = aws_sdk_s3::types::Bucket::builder()
            .name("my-bucket")
            .build();
        let bucket = convert_bucket(&sdk);
        assert_eq!(bucket.name, "my-bucket");
    }

    #[test]
    fn convert_bucket_returns_empty_name_when_no_name() {
        let sdk = aws_sdk_s3::types::Bucket::builder().build();
        let bucket = convert_bucket(&sdk);
        assert_eq!(bucket.name, "");
    }

    #[test]
    fn convert_bucket_returns_creation_date_when_set() {
        let dt = aws_sdk_s3::primitives::DateTime::from_secs(1700000000);
        let sdk = aws_sdk_s3::types::Bucket::builder()
            .name("test")
            .creation_date(dt)
            .build();
        let bucket = convert_bucket(&sdk);
        assert!(bucket.creation_date.is_some());
    }

    #[test]
    fn convert_bucket_returns_none_creation_date_when_not_set() {
        let sdk = aws_sdk_s3::types::Bucket::builder().name("test").build();
        let bucket = convert_bucket(&sdk);
        assert!(bucket.creation_date.is_none());
    }

    #[test]
    fn convert_object_returns_key_when_key_set() {
        let sdk = aws_sdk_s3::types::Object::builder()
            .key("path/to/file.txt")
            .build();
        let obj = convert_object(&sdk);
        assert_eq!(obj.key, "path/to/file.txt");
    }

    #[test]
    fn convert_object_returns_empty_key_when_no_key() {
        let sdk = aws_sdk_s3::types::Object::builder().build();
        let obj = convert_object(&sdk);
        assert_eq!(obj.key, "");
    }

    #[test]
    fn convert_object_returns_size_when_set() {
        let sdk = aws_sdk_s3::types::Object::builder()
            .key("file.txt")
            .size(1024)
            .build();
        let obj = convert_object(&sdk);
        assert_eq!(obj.size, Some(1024));
    }

    #[test]
    fn convert_object_returns_none_size_when_not_set() {
        let sdk = aws_sdk_s3::types::Object::builder().key("file.txt").build();
        let obj = convert_object(&sdk);
        assert!(obj.size.is_none());
    }

    #[test]
    fn convert_object_returns_storage_class_when_set() {
        let sdk = aws_sdk_s3::types::Object::builder()
            .key("file.txt")
            .storage_class(aws_sdk_s3::types::ObjectStorageClass::Standard)
            .build();
        let obj = convert_object(&sdk);
        assert_eq!(obj.storage_class.as_deref(), Some("STANDARD"));
    }

    #[test]
    fn convert_object_returns_none_storage_class_when_not_set() {
        let sdk = aws_sdk_s3::types::Object::builder().key("file.txt").build();
        let obj = convert_object(&sdk);
        assert!(obj.storage_class.is_none());
    }

    #[test]
    fn convert_object_returns_false_is_prefix_when_converted() {
        let sdk = aws_sdk_s3::types::Object::builder().key("file.txt").build();
        let obj = convert_object(&sdk);
        assert!(!obj.is_prefix);
    }

    #[test]
    fn convert_object_returns_last_modified_when_set() {
        let dt = aws_sdk_s3::primitives::DateTime::from_secs(1700000000);
        let sdk = aws_sdk_s3::types::Object::builder()
            .key("file.txt")
            .last_modified(dt)
            .build();
        let obj = convert_object(&sdk);
        assert!(obj.last_modified.is_some());
    }

    #[test]
    fn convert_object_returns_none_last_modified_when_not_set() {
        let sdk = aws_sdk_s3::types::Object::builder().key("file.txt").build();
        let obj = convert_object(&sdk);
        assert!(obj.last_modified.is_none());
    }
}
