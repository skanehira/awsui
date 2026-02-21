/// ECSクラスターのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    pub cluster_name: String,
    pub cluster_arn: String,
    pub status: String,
    pub running_tasks_count: i32,
    pub pending_tasks_count: i32,
    pub active_services_count: i32,
    pub registered_container_instances_count: i32,
}

/// ECSサービスのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Service {
    pub service_name: String,
    pub service_arn: String,
    pub cluster_arn: String,
    pub status: String,
    pub desired_count: i32,
    pub running_count: i32,
    pub pending_count: i32,
    pub task_definition: String,
    pub launch_type: Option<String>,
    pub scheduling_strategy: Option<String>,
    pub created_at: Option<String>,
    pub health_check_grace_period_seconds: Option<i32>,
    pub deployment_status: Option<String>,
    pub enable_execute_command: bool,
}

/// ECSタスクのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    pub task_arn: String,
    pub cluster_arn: String,
    pub task_definition_arn: String,
    pub last_status: String,
    pub desired_status: String,
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub launch_type: Option<String>,
    pub platform_version: Option<String>,
    pub health_status: Option<String>,
    pub connectivity: Option<String>,
    pub availability_zone: Option<String>,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub stopped_reason: Option<String>,
    pub containers: Vec<Container>,
}

/// ECSコンテナのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub last_status: String,
    pub exit_code: Option<i32>,
    pub health_status: Option<String>,
    pub reason: Option<String>,
}

/// コンテナのawslogsログ設定
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerLogConfig {
    pub container_name: String,
    pub log_group: Option<String>,
    pub stream_prefix: Option<String>,
    pub region: Option<String>,
}
