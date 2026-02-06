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
}
