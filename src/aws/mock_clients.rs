#![cfg(feature = "mock-data")]

use std::time::Duration;

use async_trait::async_trait;

use crate::aws::client::Ec2Client;
use crate::aws::cloudwatch_client::CloudWatchClient;
use crate::aws::cloudwatch_model::MetricResult;
use crate::aws::ecr_client::EcrClient;
use crate::aws::ecr_model::{Image, ImageScanResult, Repository};
use crate::aws::ecs_client::EcsClient;
use crate::aws::ecs_model::{Cluster, ContainerLogConfig, Service, Task};
use crate::aws::logs_client::LogsClient;
use crate::aws::logs_model::LogEvent;
use crate::aws::mock_data;
use crate::aws::model::{Instance, SecurityGroup};
use crate::aws::s3_client::S3Client;
use crate::aws::s3_model::{Bucket, BucketSettings, ObjectContent, S3Object};
use crate::aws::secrets_client::SecretsClient;
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_client::VpcClient;
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::error::AppError;

const MOCK_DELAY: Duration = Duration::from_millis(100);

pub struct MockEc2ClientImpl;

#[async_trait]
impl Ec2Client for MockEc2ClientImpl {
    async fn describe_instances(&self) -> Result<Vec<Instance>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_instances())
    }

    async fn start_instances(&self, ids: &[String]) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        let _ = ids;
        Ok(())
    }

    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        let _ = ids;
        Ok(())
    }

    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        let _ = ids;
        Ok(())
    }

    async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        let _ = ids;
        Ok(())
    }

    async fn describe_security_groups(
        &self,
        _group_ids: &[String],
    ) -> Result<Vec<SecurityGroup>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_security_groups())
    }
}

pub struct MockEcrClientImpl;

#[async_trait]
impl EcrClient for MockEcrClientImpl {
    async fn describe_repositories(&self) -> Result<Vec<Repository>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_repositories())
    }

    async fn list_images(&self, _repository_name: &str) -> Result<Vec<Image>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_images())
    }

    async fn create_repository(
        &self,
        _repository_name: &str,
        _image_tag_mutability: &str,
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn delete_repository(&self, _repository_name: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn delete_images(
        &self,
        _repository_name: &str,
        _image_digests: &[String],
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn get_lifecycle_policy(
        &self,
        _repository_name: &str,
    ) -> Result<Option<String>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(Some(mock_data::mock_lifecycle_policy_json()))
    }

    async fn describe_image_scan_findings(
        &self,
        _repository_name: &str,
        _image_digest: &str,
    ) -> Result<Option<ImageScanResult>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(Some(mock_data::mock_scan_result()))
    }
}

pub struct MockEcsClientImpl;

#[async_trait]
impl EcsClient for MockEcsClientImpl {
    async fn list_clusters(&self) -> Result<Vec<Cluster>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_clusters())
    }

    async fn list_services(&self, _cluster_arn: &str) -> Result<Vec<Service>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_services())
    }

    async fn list_tasks(
        &self,
        _cluster_arn: &str,
        _service_name: &str,
    ) -> Result<Vec<Task>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_tasks())
    }

    async fn describe_task_definition_log_configs(
        &self,
        _task_definition_arn: &str,
    ) -> Result<Vec<ContainerLogConfig>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_log_configs())
    }

    async fn update_service(
        &self,
        _cluster_arn: &str,
        _service_name: &str,
        _force_new_deployment: bool,
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn update_service_desired_count(
        &self,
        _cluster_arn: &str,
        _service_name: &str,
        _desired_count: i32,
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn stop_task(&self, _cluster_arn: &str, _task_arn: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }
}

pub struct MockS3ClientImpl;

#[async_trait]
impl S3Client for MockS3ClientImpl {
    async fn list_buckets(&self) -> Result<Vec<Bucket>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_buckets())
    }

    async fn list_objects(
        &self,
        _bucket_name: &str,
        _prefix: Option<String>,
    ) -> Result<Vec<S3Object>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_objects())
    }

    async fn create_bucket(&self, _bucket_name: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn delete_bucket(&self, _bucket_name: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn delete_object(&self, _bucket_name: &str, _key: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn get_bucket_settings(&self, _bucket_name: &str) -> Result<BucketSettings, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_bucket_settings())
    }

    async fn get_object(&self, _bucket_name: &str, _key: &str) -> Result<ObjectContent, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_object_content())
    }

    async fn download_object(&self, _bucket_name: &str, _key: &str) -> Result<Vec<u8>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(b"mock file content".to_vec())
    }

    async fn put_object(
        &self,
        _bucket_name: &str,
        _key: &str,
        _body: Vec<u8>,
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }
}

pub struct MockVpcClientImpl;

#[async_trait]
impl VpcClient for MockVpcClientImpl {
    async fn describe_vpcs(&self) -> Result<Vec<Vpc>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_vpcs())
    }

    async fn describe_subnets(&self, _vpc_id: &str) -> Result<Vec<Subnet>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_subnets())
    }
}

pub struct MockSecretsClientImpl;

#[async_trait]
impl SecretsClient for MockSecretsClientImpl {
    async fn list_secrets(&self) -> Result<Vec<Secret>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_secrets())
    }

    async fn describe_secret(&self, _secret_id: &str) -> Result<SecretDetail, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_secret_detail())
    }

    async fn create_secret(
        &self,
        _name: &str,
        _value: &str,
        _description: Option<String>,
    ) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn update_secret_value(&self, _secret_id: &str, _value: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn delete_secret(&self, _secret_id: &str) -> Result<(), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(())
    }

    async fn get_secret_value(&self, _secret_id: &str) -> Result<String, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok("mock-secret-value-12345".to_string())
    }
}

pub struct MockCloudWatchClientImpl;

#[async_trait]
impl CloudWatchClient for MockCloudWatchClientImpl {
    async fn get_metric_data(&self, _instance_id: &str) -> Result<Vec<MetricResult>, AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok(mock_data::mock_metrics())
    }
}

pub struct MockLogsClientImpl;

#[async_trait]
impl LogsClient for MockLogsClientImpl {
    async fn get_log_events(
        &self,
        _log_group: &str,
        _log_stream: &str,
        _next_token: Option<String>,
    ) -> Result<(Vec<LogEvent>, Option<String>), AppError> {
        tokio::time::sleep(MOCK_DELAY).await;
        Ok((mock_data::mock_log_events(), None))
    }
}
