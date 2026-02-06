use async_trait::async_trait;

use crate::aws::model::Instance;
use crate::error::AppError;

#[cfg(test)]
use mockall::automock;

/// EC2 APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait Ec2Client: Send + Sync {
    async fn describe_instances(&self) -> Result<Vec<Instance>, AppError>;
    async fn start_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError>;
    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError>;
}
