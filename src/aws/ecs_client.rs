use async_trait::async_trait;

use crate::aws::ecs_model::{Cluster, Container, ContainerLogConfig, Service, Task};
use crate::error::{AppError, format_error_chain};

#[cfg(test)]
use mockall::automock;

/// ECS APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait EcsClient: Send + Sync {
    async fn list_clusters(&self) -> Result<Vec<Cluster>, AppError>;
    async fn list_services(&self, cluster_arn: &str) -> Result<Vec<Service>, AppError>;
    async fn list_tasks(
        &self,
        cluster_arn: &str,
        service_name: &str,
    ) -> Result<Vec<Task>, AppError>;
    async fn describe_task_definition_log_configs(
        &self,
        task_definition_arn: &str,
    ) -> Result<Vec<ContainerLogConfig>, AppError>;
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
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let services = resp.services().iter().map(convert_service).collect();

        Ok(services)
    }

    async fn list_tasks(
        &self,
        cluster_arn: &str,
        service_name: &str,
    ) -> Result<Vec<Task>, AppError> {
        // list_tasksでARN一覧を取得
        let mut task_arns = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self
                .client
                .list_tasks()
                .cluster(cluster_arn)
                .service_name(service_name);
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

            task_arns.extend(resp.task_arns().iter().map(|s| s.to_string()));

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        if task_arns.is_empty() {
            return Ok(Vec::new());
        }

        // describe_tasksで詳細を取得
        let resp = self
            .client
            .describe_tasks()
            .cluster(cluster_arn)
            .set_tasks(Some(task_arns))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let tasks = resp.tasks().iter().map(convert_task).collect();

        Ok(tasks)
    }

    async fn describe_task_definition_log_configs(
        &self,
        task_definition_arn: &str,
    ) -> Result<Vec<ContainerLogConfig>, AppError> {
        let resp = self
            .client
            .describe_task_definition()
            .task_definition(task_definition_arn)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let Some(task_def) = resp.task_definition() else {
            return Ok(Vec::new());
        };

        let configs = task_def
            .container_definitions()
            .iter()
            .filter_map(|cd| {
                let log_config = cd.log_configuration()?;
                if *log_config.log_driver() != aws_sdk_ecs::types::LogDriver::Awslogs {
                    return None;
                }
                let options = log_config.options();
                let container_name = cd.name().unwrap_or_default().to_string();
                let log_group = options
                    .and_then(|o| o.get("awslogs-group"))
                    .map(String::from);
                let stream_prefix = options
                    .and_then(|o| o.get("awslogs-stream-prefix"))
                    .map(String::from);
                let region = options
                    .and_then(|o| o.get("awslogs-region"))
                    .map(String::from);
                Some(ContainerLogConfig {
                    container_name,
                    log_group,
                    stream_prefix,
                    region,
                })
            })
            .collect();

        Ok(configs)
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

/// SDK の Task → ドメインの Task
fn convert_task(sdk: &aws_sdk_ecs::types::Task) -> Task {
    Task {
        task_arn: sdk.task_arn().unwrap_or_default().to_string(),
        cluster_arn: sdk.cluster_arn().unwrap_or_default().to_string(),
        task_definition_arn: sdk.task_definition_arn().unwrap_or_default().to_string(),
        last_status: sdk.last_status().unwrap_or_default().to_string(),
        desired_status: sdk.desired_status().unwrap_or_default().to_string(),
        cpu: sdk.cpu().map(|s| s.to_string()),
        memory: sdk.memory().map(|s| s.to_string()),
        launch_type: sdk.launch_type().map(|lt| lt.as_str().to_string()),
        platform_version: sdk.platform_version().map(|s| s.to_string()),
        health_status: sdk.health_status().map(|h| h.as_str().to_string()),
        connectivity: sdk.connectivity().map(|c| c.as_str().to_string()),
        availability_zone: sdk.availability_zone().map(|s| s.to_string()),
        started_at: sdk.started_at().map(|dt| {
            dt.fmt(aws_sdk_ecs::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        stopped_at: sdk.stopped_at().map(|dt| {
            dt.fmt(aws_sdk_ecs::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        stopped_reason: sdk.stop_code().map(|s| s.as_str().to_string()),
        containers: sdk.containers().iter().map(convert_container).collect(),
    }
}

/// SDK の Container → ドメインの Container
fn convert_container(sdk: &aws_sdk_ecs::types::Container) -> Container {
    Container {
        name: sdk.name().unwrap_or_default().to_string(),
        image: sdk.image().unwrap_or_default().to_string(),
        last_status: sdk.last_status().unwrap_or_default().to_string(),
        exit_code: sdk.exit_code(),
        health_status: sdk.health_status().map(|h| h.as_str().to_string()),
        reason: sdk.reason().map(|s| s.to_string()),
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
        scheduling_strategy: sdk.scheduling_strategy().map(|s| s.as_str().to_string()),
        created_at: sdk.created_at().map(|dt| {
            dt.fmt(aws_sdk_ecs::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        health_check_grace_period_seconds: sdk.health_check_grace_period_seconds(),
        deployment_status: sdk
            .deployments()
            .first()
            .and_then(|d| d.rollout_state())
            .map(|s| s.as_str().to_string()),
        enable_execute_command: sdk.enable_execute_command(),
    }
}
