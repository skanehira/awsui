use async_trait::async_trait;

use crate::aws::logs_model::LogEvent;
use crate::error::{AppError, format_error_chain};

#[cfg(test)]
use mockall::automock;

/// CloudWatch Logs APIクライアントのtrait
#[cfg_attr(test, automock)]
#[async_trait]
pub trait LogsClient: Send + Sync {
    async fn get_log_events(
        &self,
        log_group: &str,
        log_stream: &str,
        next_token: Option<String>,
    ) -> Result<(Vec<LogEvent>, Option<String>), AppError>;
}

/// AWS SDK CloudWatch Logsクライアントの実装
pub struct AwsLogsClient {
    client: aws_sdk_cloudwatchlogs::Client,
}

impl AwsLogsClient {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_cloudwatchlogs::config::Region::new(
                region.to_string(),
            ))
            .load()
            .await;
        let client = aws_sdk_cloudwatchlogs::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl LogsClient for AwsLogsClient {
    async fn get_log_events(
        &self,
        log_group: &str,
        log_stream: &str,
        next_token: Option<String>,
    ) -> Result<(Vec<LogEvent>, Option<String>), AppError> {
        let mut req = self
            .client
            .get_log_events()
            .log_group_name(log_group)
            .log_stream_name(log_stream)
            .start_from_head(false);

        if let Some(token) = next_token {
            req = req.next_token(token);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let events = resp
            .events()
            .iter()
            .map(|e| {
                let timestamp = e.timestamp().unwrap_or(0);
                let formatted_time = format_timestamp(timestamp);
                let message = e.message().unwrap_or_default().to_string();
                LogEvent {
                    timestamp,
                    formatted_time,
                    message,
                }
            })
            .collect();

        let forward_token = resp.next_forward_token().map(String::from);

        Ok((events, forward_token))
    }
}

/// epoch ms → "YYYY-MM-DD HH:MM:SS" 形式に変換
fn format_timestamp(epoch_ms: i64) -> String {
    let secs = epoch_ms / 1000;
    let dt = aws_sdk_ecs::primitives::DateTime::from_secs(secs);
    dt.fmt(aws_sdk_ecs::primitives::DateTimeFormat::DateTime)
        .unwrap_or_else(|_| "???".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_timestamp_returns_datetime_when_valid_epoch_ms() {
        let result = format_timestamp(1706000000000);
        assert!(!result.is_empty());
        assert!(result.starts_with("2024-01-23"));
    }

    #[test]
    fn format_timestamp_returns_datetime_when_zero() {
        let result = format_timestamp(0);
        assert!(!result.is_empty());
    }
}
