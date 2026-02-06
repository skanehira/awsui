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

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_ec2::types::{
        EbsInstanceBlockDevice, GroupIdentifier, InstanceBlockDeviceMapping,
        InstanceState as SdkInstanceState, InstanceStateName, Placement, Tag,
    };

    fn build_sdk_instance() -> aws_sdk_ec2::types::Instance {
        aws_sdk_ec2::types::Instance::builder()
            .instance_id("i-1234567890abcdef0")
            .image_id("ami-12345678")
            .instance_type(aws_sdk_ec2::types::InstanceType::T2Micro)
            .state(
                SdkInstanceState::builder()
                    .name(InstanceStateName::Running)
                    .build(),
            )
            .placement(
                Placement::builder()
                    .availability_zone("ap-northeast-1a")
                    .build(),
            )
            .private_ip_address("10.0.1.100")
            .public_ip_address("54.123.45.67")
            .vpc_id("vpc-abc123")
            .subnet_id("subnet-def456")
            .key_name("my-key")
            .platform_details("Linux/UNIX")
            .tags(Tag::builder().key("Name").value("my-instance").build())
            .tags(
                Tag::builder()
                    .key("Environment")
                    .value("production")
                    .build(),
            )
            .security_groups(GroupIdentifier::builder().group_id("sg-123456").build())
            .block_device_mappings(
                InstanceBlockDeviceMapping::builder()
                    .device_name("/dev/xvda")
                    .ebs(
                        EbsInstanceBlockDevice::builder()
                            .volume_id("vol-abcdef")
                            .status(aws_sdk_ec2::types::AttachmentStatus::Attached)
                            .build(),
                    )
                    .build(),
            )
            .build()
    }

    #[test]
    fn convert_instance_returns_instance_id_when_valid_sdk_instance() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.instance_id, "i-1234567890abcdef0");
    }

    #[test]
    fn convert_instance_returns_name_from_tags_when_name_tag_exists() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.name, "my-instance");
    }

    #[test]
    fn convert_instance_returns_empty_name_when_no_name_tag() {
        let sdk = aws_sdk_ec2::types::Instance::builder()
            .instance_id("i-noname")
            .build();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.name, "");
    }

    #[test]
    fn convert_instance_returns_running_state_when_sdk_state_running() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.state, InstanceState::Running);
    }

    #[test]
    fn convert_instance_returns_pending_state_when_no_state() {
        let sdk = aws_sdk_ec2::types::Instance::builder()
            .instance_id("i-nostate")
            .build();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.state, InstanceState::Pending);
    }

    #[test]
    fn convert_instance_returns_instance_type_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.instance_type, "t2.micro");
    }

    #[test]
    fn convert_instance_returns_availability_zone_when_placement_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.availability_zone, "ap-northeast-1a");
    }

    #[test]
    fn convert_instance_returns_private_ip_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.private_ip.as_deref(), Some("10.0.1.100"));
    }

    #[test]
    fn convert_instance_returns_public_ip_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.public_ip.as_deref(), Some("54.123.45.67"));
    }

    #[test]
    fn convert_instance_returns_none_public_ip_when_not_set() {
        let sdk = aws_sdk_ec2::types::Instance::builder()
            .instance_id("i-nopubip")
            .build();
        let instance = convert_instance(&sdk);
        assert!(instance.public_ip.is_none());
    }

    #[test]
    fn convert_instance_returns_vpc_and_subnet_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.vpc_id.as_deref(), Some("vpc-abc123"));
        assert_eq!(instance.subnet_id.as_deref(), Some("subnet-def456"));
    }

    #[test]
    fn convert_instance_returns_ami_id_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.ami_id, "ami-12345678");
    }

    #[test]
    fn convert_instance_returns_key_name_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.key_name.as_deref(), Some("my-key"));
    }

    #[test]
    fn convert_instance_returns_platform_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.platform.as_deref(), Some("Linux/UNIX"));
    }

    #[test]
    fn convert_instance_returns_security_groups_when_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.security_groups, vec!["sg-123456"]);
    }

    #[test]
    fn convert_instance_returns_volumes_when_block_device_mappings_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.volumes.len(), 1);
        assert_eq!(instance.volumes[0].volume_id, "vol-abcdef");
        assert_eq!(instance.volumes[0].device_name, "/dev/xvda");
        assert_eq!(instance.volumes[0].state, "attached");
    }

    #[test]
    fn convert_instance_returns_tags_as_hashmap_when_tags_set() {
        let sdk = build_sdk_instance();
        let instance = convert_instance(&sdk);
        assert_eq!(instance.tags.get("Name"), Some(&"my-instance".to_string()));
        assert_eq!(
            instance.tags.get("Environment"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn convert_instance_returns_empty_volumes_when_no_block_devices() {
        let sdk = aws_sdk_ec2::types::Instance::builder()
            .instance_id("i-novols")
            .build();
        let instance = convert_instance(&sdk);
        assert!(instance.volumes.is_empty());
    }

    #[test]
    fn convert_instance_state_returns_stopped_when_sdk_stopped() {
        let state = convert_instance_state(&InstanceStateName::Stopped);
        assert_eq!(state, InstanceState::Stopped);
    }

    #[test]
    fn convert_instance_state_returns_terminated_when_sdk_terminated() {
        let state = convert_instance_state(&InstanceStateName::Terminated);
        assert_eq!(state, InstanceState::Terminated);
    }

    #[test]
    fn convert_instance_state_returns_stopping_when_sdk_stopping() {
        let state = convert_instance_state(&InstanceStateName::Stopping);
        assert_eq!(state, InstanceState::Stopping);
    }

    #[test]
    fn convert_instance_state_returns_shutting_down_when_sdk_shutting_down() {
        let state = convert_instance_state(&InstanceStateName::ShuttingDown);
        assert_eq!(state, InstanceState::ShuttingDown);
    }

    #[test]
    fn convert_instance_state_returns_pending_when_sdk_pending() {
        let state = convert_instance_state(&InstanceStateName::Pending);
        assert_eq!(state, InstanceState::Pending);
    }

    #[test]
    fn convert_instance_state_returns_running_when_sdk_running() {
        let state = convert_instance_state(&InstanceStateName::Running);
        assert_eq!(state, InstanceState::Running);
    }
}
