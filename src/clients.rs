use std::sync::Arc;

use awsui::aws::client::Ec2Client;
use awsui::aws::cloudwatch_client::CloudWatchClient;
use awsui::aws::ecr_client::EcrClient;
use awsui::aws::ecs_client::EcsClient;
use awsui::aws::logs_client::LogsClient;
use awsui::aws::s3_client::S3Client;
use awsui::aws::secrets_client::SecretsClient;
use awsui::aws::vpc_client::VpcClient;

/// 各サービスのクライアントをまとめて保持する
#[derive(Default)]
pub(crate) struct Clients {
    pub(crate) ec2: Option<Arc<dyn Ec2Client>>,
    pub(crate) ecr: Option<Arc<dyn EcrClient>>,
    pub(crate) ecs: Option<Arc<dyn EcsClient>>,
    pub(crate) s3: Option<Arc<dyn S3Client>>,
    pub(crate) vpc: Option<Arc<dyn VpcClient>>,
    pub(crate) secrets: Option<Arc<dyn SecretsClient>>,
    pub(crate) logs: Option<Arc<dyn LogsClient>>,
    pub(crate) cloudwatch: Option<Arc<dyn CloudWatchClient>>,
}

impl Clients {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
