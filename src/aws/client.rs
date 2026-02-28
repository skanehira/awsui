use std::collections::HashMap;

use async_trait::async_trait;

use crate::aws::model::{Instance, InstanceState, SecurityGroup, SecurityGroupRule, Volume};
use crate::error::{AppError, format_error_chain};

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
    async fn describe_security_groups(
        &self,
        group_ids: &[String],
    ) -> Result<Vec<SecurityGroup>, AppError>;
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
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn stop_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .stop_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn reboot_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .reboot_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn terminate_instances(&self, ids: &[String]) -> Result<(), AppError> {
        self.client
            .terminate_instances()
            .set_instance_ids(Some(ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;
        Ok(())
    }

    async fn describe_security_groups(
        &self,
        group_ids: &[String],
    ) -> Result<Vec<SecurityGroup>, AppError> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }

        let resp = self
            .client
            .describe_security_groups()
            .set_group_ids(Some(group_ids.to_vec()))
            .send()
            .await
            .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

        let security_groups = resp
            .security_groups()
            .iter()
            .map(convert_security_group)
            .collect();

        Ok(security_groups)
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

/// プロトコル文字列をフォーマットする
fn format_protocol(protocol: Option<&str>) -> String {
    match protocol {
        Some("-1") | None => "All".to_string(),
        Some(p) => p.to_string(),
    }
}

/// ポートレンジ文字列をフォーマットする
fn format_port_range(
    protocol: Option<&str>,
    from_port: Option<i32>,
    to_port: Option<i32>,
) -> String {
    // All trafficの場合
    if protocol == Some("-1") {
        return "All".to_string();
    }
    match (from_port, to_port) {
        (Some(from), Some(to)) if from == to => from.to_string(),
        (Some(from), Some(to)) => format!("{}-{}", from, to),
        _ => "All".to_string(),
    }
}

/// SDK の IpPermission → ドメインの SecurityGroupRule のリスト
/// 1つのIpPermissionに複数のソース/宛先が含まれる場合、それぞれ個別のルールに展開する
fn convert_ip_permission_to_rules(
    perm: &aws_sdk_ec2::types::IpPermission,
) -> Vec<SecurityGroupRule> {
    let protocol = format_protocol(perm.ip_protocol());
    let port_range = format_port_range(perm.ip_protocol(), perm.from_port(), perm.to_port());

    let mut rules = Vec::new();

    // IPv4 CIDRルール
    for ip_range in perm.ip_ranges() {
        rules.push(SecurityGroupRule {
            protocol: protocol.clone(),
            port_range: port_range.clone(),
            source_or_destination: ip_range.cidr_ip().unwrap_or_default().to_string(),
            description: ip_range.description().map(String::from),
        });
    }

    // IPv6 CIDRルール
    for ipv6_range in perm.ipv6_ranges() {
        rules.push(SecurityGroupRule {
            protocol: protocol.clone(),
            port_range: port_range.clone(),
            source_or_destination: ipv6_range.cidr_ipv6().unwrap_or_default().to_string(),
            description: ipv6_range.description().map(String::from),
        });
    }

    // セキュリティグループ参照ルール
    for group_pair in perm.user_id_group_pairs() {
        rules.push(SecurityGroupRule {
            protocol: protocol.clone(),
            port_range: port_range.clone(),
            source_or_destination: group_pair.group_id().unwrap_or_default().to_string(),
            description: group_pair.description().map(String::from),
        });
    }

    // プレフィックスリストルール
    for prefix_list in perm.prefix_list_ids() {
        rules.push(SecurityGroupRule {
            protocol: protocol.clone(),
            port_range: port_range.clone(),
            source_or_destination: prefix_list.prefix_list_id().unwrap_or_default().to_string(),
            description: prefix_list.description().map(String::from),
        });
    }

    rules
}

/// SDK の SecurityGroup → ドメインの SecurityGroup
pub(crate) fn convert_security_group(sdk: &aws_sdk_ec2::types::SecurityGroup) -> SecurityGroup {
    let inbound_rules = sdk
        .ip_permissions()
        .iter()
        .flat_map(convert_ip_permission_to_rules)
        .collect();

    let outbound_rules = sdk
        .ip_permissions_egress()
        .iter()
        .flat_map(convert_ip_permission_to_rules)
        .collect();

    SecurityGroup {
        group_id: sdk.group_id().unwrap_or_default().to_string(),
        group_name: sdk.group_name().unwrap_or_default().to_string(),
        description: sdk.description().unwrap_or_default().to_string(),
        inbound_rules,
        outbound_rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aws::model::{SecurityGroup, SecurityGroupRule};

    #[test]
    fn convert_security_group_returns_sg_with_rules_when_all_fields_present() {
        let sdk_sg = aws_sdk_ec2::types::SecurityGroup::builder()
            .group_id("sg-12345678")
            .group_name("my-sg")
            .description("My security group")
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("tcp")
                    .from_port(443)
                    .to_port(443)
                    .ip_ranges(
                        aws_sdk_ec2::types::IpRange::builder()
                            .cidr_ip("10.0.0.0/8")
                            .description("Internal VPN")
                            .build(),
                    )
                    .build(),
            )
            .ip_permissions_egress(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("-1")
                    .ip_ranges(
                        aws_sdk_ec2::types::IpRange::builder()
                            .cidr_ip("0.0.0.0/0")
                            .build(),
                    )
                    .build(),
            )
            .build();

        let result = convert_security_group(&sdk_sg);

        assert_eq!(
            result,
            SecurityGroup {
                group_id: "sg-12345678".to_string(),
                group_name: "my-sg".to_string(),
                description: "My security group".to_string(),
                inbound_rules: vec![SecurityGroupRule {
                    protocol: "tcp".to_string(),
                    port_range: "443".to_string(),
                    source_or_destination: "10.0.0.0/8".to_string(),
                    description: Some("Internal VPN".to_string()),
                }],
                outbound_rules: vec![SecurityGroupRule {
                    protocol: "All".to_string(),
                    port_range: "All".to_string(),
                    source_or_destination: "0.0.0.0/0".to_string(),
                    description: None,
                }],
            }
        );
    }

    #[test]
    fn convert_security_group_returns_multiple_rules_when_multiple_sources() {
        let sdk_sg = aws_sdk_ec2::types::SecurityGroup::builder()
            .group_id("sg-abcdef00")
            .group_name("web-sg")
            .description("Web server SG")
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("tcp")
                    .from_port(80)
                    .to_port(80)
                    .ip_ranges(
                        aws_sdk_ec2::types::IpRange::builder()
                            .cidr_ip("0.0.0.0/0")
                            .description("Public HTTP")
                            .build(),
                    )
                    .ipv6_ranges(
                        aws_sdk_ec2::types::Ipv6Range::builder()
                            .cidr_ipv6("::/0")
                            .description("Public HTTP IPv6")
                            .build(),
                    )
                    .build(),
            )
            .build();

        let result = convert_security_group(&sdk_sg);

        assert_eq!(
            result,
            SecurityGroup {
                group_id: "sg-abcdef00".to_string(),
                group_name: "web-sg".to_string(),
                description: "Web server SG".to_string(),
                inbound_rules: vec![
                    SecurityGroupRule {
                        protocol: "tcp".to_string(),
                        port_range: "80".to_string(),
                        source_or_destination: "0.0.0.0/0".to_string(),
                        description: Some("Public HTTP".to_string()),
                    },
                    SecurityGroupRule {
                        protocol: "tcp".to_string(),
                        port_range: "80".to_string(),
                        source_or_destination: "::/0".to_string(),
                        description: Some("Public HTTP IPv6".to_string()),
                    },
                ],
                outbound_rules: vec![],
            }
        );
    }

    #[test]
    fn convert_security_group_returns_sg_reference_rule_when_user_id_group_pair() {
        let sdk_sg = aws_sdk_ec2::types::SecurityGroup::builder()
            .group_id("sg-11111111")
            .group_name("backend-sg")
            .description("Backend SG")
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("tcp")
                    .from_port(5432)
                    .to_port(5432)
                    .user_id_group_pairs(
                        aws_sdk_ec2::types::UserIdGroupPair::builder()
                            .group_id("sg-22222222")
                            .description("From web tier")
                            .build(),
                    )
                    .build(),
            )
            .build();

        let result = convert_security_group(&sdk_sg);

        assert_eq!(
            result.inbound_rules,
            vec![SecurityGroupRule {
                protocol: "tcp".to_string(),
                port_range: "5432".to_string(),
                source_or_destination: "sg-22222222".to_string(),
                description: Some("From web tier".to_string()),
            }]
        );
    }

    #[test]
    fn convert_security_group_returns_port_range_when_range_differs() {
        let sdk_sg = aws_sdk_ec2::types::SecurityGroup::builder()
            .group_id("sg-33333333")
            .group_name("ephemeral-sg")
            .description("Ephemeral ports")
            .ip_permissions(
                aws_sdk_ec2::types::IpPermission::builder()
                    .ip_protocol("tcp")
                    .from_port(1024)
                    .to_port(65535)
                    .ip_ranges(
                        aws_sdk_ec2::types::IpRange::builder()
                            .cidr_ip("10.0.0.0/8")
                            .build(),
                    )
                    .build(),
            )
            .build();

        let result = convert_security_group(&sdk_sg);

        assert_eq!(
            result.inbound_rules,
            vec![SecurityGroupRule {
                protocol: "tcp".to_string(),
                port_range: "1024-65535".to_string(),
                source_or_destination: "10.0.0.0/8".to_string(),
                description: None,
            }]
        );
    }

    #[test]
    fn convert_security_group_returns_empty_rules_when_no_permissions() {
        let sdk_sg = aws_sdk_ec2::types::SecurityGroup::builder()
            .group_id("sg-empty0000")
            .group_name("empty-sg")
            .description("No rules")
            .build();

        let result = convert_security_group(&sdk_sg);

        assert_eq!(
            result,
            SecurityGroup {
                group_id: "sg-empty0000".to_string(),
                group_name: "empty-sg".to_string(),
                description: "No rules".to_string(),
                inbound_rules: vec![],
                outbound_rules: vec![],
            }
        );
    }
}
