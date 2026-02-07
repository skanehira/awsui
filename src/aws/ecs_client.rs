use async_trait::async_trait;

use crate::aws::ecs_model::{Cluster, Service};
use crate::error::AppError;

#[cfg(test)]
use mockall::automock;

/// ECS APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait EcsClient: Send + Sync {
    async fn list_clusters(&self) -> Result<Vec<Cluster>, AppError>;
    async fn list_services(&self, cluster_arn: &str) -> Result<Vec<Service>, AppError>;
}

/// AWS SDK ECSクライアントの実装
pub struct AwsEcsClient {
    client: aws_sdk_ecs::Client,
}

impl AwsEcsClient {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_ecs::config::Region::new(region.to_string()))
            .load()
            .await;
        let client = aws_sdk_ecs::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl EcsClient for AwsEcsClient {
    async fn list_clusters(&self) -> Result<Vec<Cluster>, AppError> {
        // list_clustersでARN一覧を取得
        let mut cluster_arns = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_clusters();
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            cluster_arns.extend(resp.cluster_arns().iter().map(|s| s.to_string()));

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        if cluster_arns.is_empty() {
            return Ok(Vec::new());
        }

        // describe_clustersで詳細を取得
        let resp = self
            .client
            .describe_clusters()
            .set_clusters(Some(cluster_arns))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;

        let clusters = resp.clusters().iter().map(convert_cluster).collect();

        Ok(clusters)
    }

    async fn list_services(&self, cluster_arn: &str) -> Result<Vec<Service>, AppError> {
        // list_servicesでARN一覧を取得
        let mut service_arns = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_services().cluster(cluster_arn);
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            service_arns.extend(resp.service_arns().iter().map(|s| s.to_string()));

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        if service_arns.is_empty() {
            return Ok(Vec::new());
        }

        // describe_servicesで詳細を取得
        let resp = self
            .client
            .describe_services()
            .cluster(cluster_arn)
            .set_services(Some(service_arns))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;

        let services = resp.services().iter().map(convert_service).collect();

        Ok(services)
    }
}

/// SDK の Cluster → ドメインの Cluster
fn convert_cluster(sdk: &aws_sdk_ecs::types::Cluster) -> Cluster {
    Cluster {
        cluster_name: sdk.cluster_name().unwrap_or_default().to_string(),
        cluster_arn: sdk.cluster_arn().unwrap_or_default().to_string(),
        status: sdk.status().unwrap_or_default().to_string(),
        running_tasks_count: sdk.running_tasks_count(),
        pending_tasks_count: sdk.pending_tasks_count(),
        active_services_count: sdk.active_services_count(),
        registered_container_instances_count: sdk.registered_container_instances_count(),
    }
}

/// SDK の Service → ドメインの Service
fn convert_service(sdk: &aws_sdk_ecs::types::Service) -> Service {
    Service {
        service_name: sdk.service_name().unwrap_or_default().to_string(),
        service_arn: sdk.service_arn().unwrap_or_default().to_string(),
        cluster_arn: sdk.cluster_arn().unwrap_or_default().to_string(),
        status: sdk.status().unwrap_or_default().to_string(),
        desired_count: sdk.desired_count(),
        running_count: sdk.running_count(),
        pending_count: sdk.pending_count(),
        task_definition: sdk.task_definition().unwrap_or_default().to_string(),
        launch_type: sdk.launch_type().map(|lt| lt.as_str().to_string()),
    }
}
