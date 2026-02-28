use async_trait::async_trait;

use crate::aws::cloudwatch_model::{MetricDataPoint, MetricResult};
use crate::error::{AppError, format_error_chain};

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait CloudWatchClient: Send + Sync {
    async fn get_metric_data(&self, instance_id: &str) -> Result<Vec<MetricResult>, AppError>;
}

/// AWS SDK CloudWatchクライアント実装
pub struct AwsCloudWatchClient {
    client: aws_sdk_cloudwatch::Client,
}

impl AwsCloudWatchClient {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_cloudwatch::config::Region::new(region.to_string()))
            .load()
            .await;
        let client = aws_sdk_cloudwatch::Client::new(&config);
        Ok(Self { client })
    }
}

/// メトリクスクエリを構築するヘルパー
fn build_metric_query(
    id: &str,
    metric_name: &str,
    instance_id: &str,
    stat: &str,
) -> aws_sdk_cloudwatch::types::MetricDataQuery {
    use aws_sdk_cloudwatch::types::{Dimension, Metric, MetricDataQuery, MetricStat};

    let dimension = Dimension::builder()
        .name("InstanceId")
        .value(instance_id)
        .build();

    let metric = Metric::builder()
        .namespace("AWS/EC2")
        .metric_name(metric_name)
        .dimensions(dimension)
        .build();

    let metric_stat = MetricStat::builder()
        .metric(metric)
        .period(300) // 5分間隔
        .stat(stat)
        .build();

    MetricDataQuery::builder()
        .id(id)
        .metric_stat(metric_stat)
        .build()
}

#[async_trait]
impl CloudWatchClient for AwsCloudWatchClient {
    async fn get_metric_data(&self, instance_id: &str) -> Result<Vec<MetricResult>, AppError> {
        let now = std::time::SystemTime::now();
        let one_hour_ago = now - std::time::Duration::from_secs(3600);

        let start = aws_sdk_cloudwatch::primitives::DateTime::from(one_hour_ago);
        let end = aws_sdk_cloudwatch::primitives::DateTime::from(now);

        let queries = vec![
            build_metric_query("cpu", "CPUUtilization", instance_id, "Average"),
            build_metric_query("net_in", "NetworkIn", instance_id, "Average"),
            build_metric_query("net_out", "NetworkOut", instance_id, "Average"),
        ];

        let resp = self
            .client
            .get_metric_data()
            .set_metric_data_queries(Some(queries))
            .start_time(start)
            .end_time(end)
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let mut results = Vec::new();
        for result in resp.metric_data_results() {
            let label = result.label().unwrap_or("Unknown").to_string();
            let timestamps = result.timestamps();
            let values = result.values();

            let data_points: Vec<MetricDataPoint> = timestamps
                .iter()
                .zip(values.iter())
                .map(|(ts, val)| MetricDataPoint {
                    timestamp: ts.secs() as f64,
                    value: *val,
                })
                .collect();

            results.push(MetricResult { label, data_points });
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_metric_data_returns_metric_results_when_called() {
        let mut mock = MockCloudWatchClient::new();
        mock.expect_get_metric_data()
            .withf(|id| id == "i-001")
            .returning(|_| {
                Ok(vec![
                    MetricResult {
                        label: "CPUUtilization".to_string(),
                        data_points: vec![
                            MetricDataPoint {
                                timestamp: 1700000000.0,
                                value: 10.0,
                            },
                            MetricDataPoint {
                                timestamp: 1700000300.0,
                                value: 25.5,
                            },
                        ],
                    },
                    MetricResult {
                        label: "NetworkIn".to_string(),
                        data_points: vec![MetricDataPoint {
                            timestamp: 1700000000.0,
                            value: 1024.0,
                        }],
                    },
                ])
            });

        let result = mock.get_metric_data("i-001").await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, "CPUUtilization");
        assert_eq!(result[0].data_points.len(), 2);
        assert_eq!(result[1].label, "NetworkIn");
    }

    #[tokio::test]
    async fn get_metric_data_returns_error_when_api_fails() {
        let mut mock = MockCloudWatchClient::new();
        mock.expect_get_metric_data()
            .returning(|_| Err(AppError::AwsApi("access denied".to_string())));

        let result = mock.get_metric_data("i-001").await;
        assert!(result.is_err());
    }
}
