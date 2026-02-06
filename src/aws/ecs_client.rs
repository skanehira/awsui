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

#[cfg(test)]
mod tests {
    use super::*;

    fn build_sdk_cluster() -> aws_sdk_ecs::types::Cluster {
        aws_sdk_ecs::types::Cluster::builder()
            .cluster_name("my-cluster")
            .cluster_arn("arn:aws:ecs:ap-northeast-1:123456789012:cluster/my-cluster")
            .status("ACTIVE")
            .running_tasks_count(5)
            .pending_tasks_count(2)
            .active_services_count(3)
            .registered_container_instances_count(4)
            .build()
    }

    fn build_sdk_service() -> aws_sdk_ecs::types::Service {
        aws_sdk_ecs::types::Service::builder()
            .service_name("my-service")
            .service_arn("arn:aws:ecs:ap-northeast-1:123456789012:service/my-cluster/my-service")
            .cluster_arn("arn:aws:ecs:ap-northeast-1:123456789012:cluster/my-cluster")
            .status("ACTIVE")
            .desired_count(3)
            .running_count(3)
            .pending_count(0)
            .task_definition("arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1")
            .launch_type(aws_sdk_ecs::types::LaunchType::Fargate)
            .build()
    }

    #[test]
    fn convert_cluster_returns_cluster_name_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.cluster_name, "my-cluster");
    }

    #[test]
    fn convert_cluster_returns_cluster_arn_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(
            cluster.cluster_arn,
            "arn:aws:ecs:ap-northeast-1:123456789012:cluster/my-cluster"
        );
    }

    #[test]
    fn convert_cluster_returns_status_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.status, "ACTIVE");
    }

    #[test]
    fn convert_cluster_returns_task_counts_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.running_tasks_count, 5);
        assert_eq!(cluster.pending_tasks_count, 2);
    }

    #[test]
    fn convert_cluster_returns_service_count_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.active_services_count, 3);
    }

    #[test]
    fn convert_cluster_returns_container_instances_count_when_valid_sdk_cluster() {
        let sdk = build_sdk_cluster();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.registered_container_instances_count, 4);
    }

    #[test]
    fn convert_cluster_returns_defaults_when_empty_sdk_cluster() {
        let sdk = aws_sdk_ecs::types::Cluster::builder().build();
        let cluster = convert_cluster(&sdk);
        assert_eq!(cluster.cluster_name, "");
        assert_eq!(cluster.cluster_arn, "");
        assert_eq!(cluster.status, "");
        assert_eq!(cluster.running_tasks_count, 0);
        assert_eq!(cluster.pending_tasks_count, 0);
        assert_eq!(cluster.active_services_count, 0);
        assert_eq!(cluster.registered_container_instances_count, 0);
    }

    #[test]
    fn convert_service_returns_service_name_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(service.service_name, "my-service");
    }

    #[test]
    fn convert_service_returns_service_arn_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(
            service.service_arn,
            "arn:aws:ecs:ap-northeast-1:123456789012:service/my-cluster/my-service"
        );
    }

    #[test]
    fn convert_service_returns_cluster_arn_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(
            service.cluster_arn,
            "arn:aws:ecs:ap-northeast-1:123456789012:cluster/my-cluster"
        );
    }

    #[test]
    fn convert_service_returns_status_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(service.status, "ACTIVE");
    }

    #[test]
    fn convert_service_returns_counts_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(service.desired_count, 3);
        assert_eq!(service.running_count, 3);
        assert_eq!(service.pending_count, 0);
    }

    #[test]
    fn convert_service_returns_task_definition_when_valid_sdk_service() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(
            service.task_definition,
            "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/my-task:1"
        );
    }

    #[test]
    fn convert_service_returns_launch_type_when_set() {
        let sdk = build_sdk_service();
        let service = convert_service(&sdk);
        assert_eq!(service.launch_type.as_deref(), Some("FARGATE"));
    }

    #[test]
    fn convert_service_returns_none_launch_type_when_not_set() {
        let sdk = aws_sdk_ecs::types::Service::builder().build();
        let service = convert_service(&sdk);
        assert!(service.launch_type.is_none());
    }

    #[test]
    fn convert_service_returns_defaults_when_empty_sdk_service() {
        let sdk = aws_sdk_ecs::types::Service::builder().build();
        let service = convert_service(&sdk);
        assert_eq!(service.service_name, "");
        assert_eq!(service.service_arn, "");
        assert_eq!(service.cluster_arn, "");
        assert_eq!(service.status, "");
        assert_eq!(service.desired_count, 0);
        assert_eq!(service.running_count, 0);
        assert_eq!(service.pending_count, 0);
        assert_eq!(service.task_definition, "");
        assert!(service.launch_type.is_none());
    }
}
