use std::collections::HashMap;

use async_trait::async_trait;

use crate::aws::model::{Instance, InstanceState, Volume};
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
    async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError>;
}

/// AWS SDK EC2クライアントの実装
pub struct AwsEc2Client {
    client: aws_sdk_ec2::Client,
}

impl AwsEc2Client {
    pub async fn new(profile: &str, region: &str) -> Result<Self, AppError> {
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .profile_name(profile)
            .region(aws_sdk_ec2::config::Region::new(region.to_string()))
            .load()
            .await;
        let client = aws_sdk_ec2::Client::new(&config);
        Ok(Self { client })
    }
}

#[async_trait]
impl Ec2Client for AwsEc2Client {
    async fn describe_instances(&self) -> Result<Vec<Instance>, AppError> {
        let mut instances = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.describe_instances();
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            for reservation in resp.reservations() {
                for sdk_instance in reservation.instances() {
                    instances.push(convert_instance(sdk_instance));
                }
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(instances)
    }

    async fn start_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .start_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .stop_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .reboot_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }

    async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .terminate_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(e.to_string()))?;
        Ok(())
    }
}

/// SDK の InstanceStateName → ドメインの InstanceState
fn convert_instance_state(state: &aws_sdk_ec2::types::InstanceStateName) -> InstanceState {
    match state {
        aws_sdk_ec2::types::InstanceStateName::Pending => InstanceState::Pending,
        aws_sdk_ec2::types::InstanceStateName::Running => InstanceState::Running,
        aws_sdk_ec2::types::InstanceStateName::ShuttingDown => InstanceState::ShuttingDown,
        aws_sdk_ec2::types::InstanceStateName::Terminated => InstanceState::Terminated,
        aws_sdk_ec2::types::InstanceStateName::Stopping => InstanceState::Stopping,
        aws_sdk_ec2::types::InstanceStateName::Stopped => InstanceState::Stopped,
        _ => InstanceState::Pending,
    }
}

/// SDK の Instance → ドメインの Instance
fn convert_instance(sdk: &aws_sdk_ec2::types::Instance) -> Instance {
    let name = sdk
        .tags()
        .iter()
        .find(|t| t.key() == Some("Name"))
        .and_then(|t| t.value().map(String::from))
        .unwrap_or_default();

    let state = sdk
        .state()
        .and_then(|s| s.name().cloned())
        .map(|s| convert_instance_state(&s))
        .unwrap_or(InstanceState::Pending);

    let instance_type = sdk
        .instance_type()
        .map(|t| t.as_str().to_string())
        .unwrap_or_default();

    let availability_zone = sdk
        .placement()
        .and_then(|p| p.availability_zone())
        .unwrap_or_default()
        .to_string();

    let security_groups = sdk
        .security_groups()
        .iter()
        .filter_map(|sg| sg.group_id().map(String::from))
        .collect();

    let volumes = sdk
        .block_device_mappings()
        .iter()
        .filter_map(|bdm| {
            let device_name = bdm.device_name().unwrap_or_default().to_string();
            bdm.ebs().map(|ebs| Volume {
                volume_id: ebs.volume_id().unwrap_or_default().to_string(),
                volume_type: String::new(),
                size_gb: 0,
                device_name,
                state: ebs
                    .status()
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_default(),
            })
        })
        .collect();

    let tags: HashMap<String, String> = sdk
        .tags()
        .iter()
        .filter_map(|t| {
            let key = t.key()?.to_string();
            let value = t.value()?.to_string();
            Some((key, value))
        })
        .collect();

    Instance {
        instance_id: sdk.instance_id().unwrap_or_default().to_string(),
        name,
        state,
        instance_type,
        availability_zone,
        private_ip: sdk.private_ip_address().map(String::from),
        public_ip: sdk.public_ip_address().map(String::from),
        vpc_id: sdk.vpc_id().map(String::from),
        subnet_id: sdk.subnet_id().map(String::from),
        ami_id: sdk.image_id().unwrap_or_default().to_string(),
        key_name: sdk.key_name().map(String::from),
        platform: sdk.platform_details().map(String::from),
        launch_time: sdk.launch_time().map(|t| {
            t.fmt(aws_sdk_ec2::primitives::DateTimeFormat::DateTime)
                .unwrap_or_default()
        }),
        security_groups,
        volumes,
        tags,
    }
}
