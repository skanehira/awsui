use std::collections::HashMap;

use async_trait::async_trait;

use crate::aws::vpc_model::{Subnet, Vpc};
use crate::error::AppError;

#[cfg(test)]
use mockall::automock;

/// VPC APIクライアントのtrait。テスト時はmockallでモック化される。
#[cfg_attr(test, automock)]
#[async_trait]
pub trait VpcClient: Send + Sync {
    async fn describe_vpcs(&self) -> Result<Vec<Vpc>, AppError>;
    async fn describe_subnets(&self, vpc_id: &str) -> Result<Vec<Subnet>, AppError>;
}

/// AWS SDK EC2クライアントによるVPC実装
pub struct AwsVpcClient {
    client: aws_sdk_ec2::Client,
}

impl AwsVpcClient {
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
impl VpcClient for AwsVpcClient {
    async fn describe_vpcs(&self) -> Result<Vec<Vpc>, AppError> {
        let mut vpcs = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.describe_vpcs();
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            for sdk_vpc in resp.vpcs() {
                vpcs.push(convert_vpc(sdk_vpc));
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(vpcs)
    }

    async fn describe_subnets(&self, vpc_id: &str) -> Result<Vec<Subnet>, AppError> {
        let mut subnets = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.describe_subnets().filters(
                aws_sdk_ec2::types::Filter::builder()
                    .name("vpc-id")
                    .values(vpc_id)
                    .build(),
            );
            if let Some(token) = &next_token {
                req = req.next_token(token);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| AppError::AwsApi(e.to_string()))?;

            for sdk_subnet in resp.subnets() {
                subnets.push(convert_subnet(sdk_subnet));
            }

            next_token = resp.next_token().map(String::from);
            if next_token.is_none() {
                break;
            }
        }

        Ok(subnets)
    }
}

/// SDK の Vpc → ドメインの Vpc
fn convert_vpc(sdk: &aws_sdk_ec2::types::Vpc) -> Vpc {
    let name = sdk
        .tags()
        .iter()
        .find(|t| t.key() == Some("Name"))
        .and_then(|t| t.value().map(String::from))
        .unwrap_or_default();

    let tags: HashMap<String, String> = sdk
        .tags()
        .iter()
        .filter_map(|t| {
            let key = t.key()?.to_string();
            let value = t.value()?.to_string();
            Some((key, value))
        })
        .collect();

    Vpc {
        vpc_id: sdk.vpc_id().unwrap_or_default().to_string(),
        name,
        cidr_block: sdk.cidr_block().unwrap_or_default().to_string(),
        state: sdk
            .state()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default(),
        is_default: sdk.is_default().unwrap_or_default(),
        owner_id: sdk.owner_id().unwrap_or_default().to_string(),
        tags,
    }
}

/// SDK の Subnet → ドメインの Subnet
fn convert_subnet(sdk: &aws_sdk_ec2::types::Subnet) -> Subnet {
    let name = sdk
        .tags()
        .iter()
        .find(|t| t.key() == Some("Name"))
        .and_then(|t| t.value().map(String::from))
        .unwrap_or_default();

    Subnet {
        subnet_id: sdk.subnet_id().unwrap_or_default().to_string(),
        name,
        vpc_id: sdk.vpc_id().unwrap_or_default().to_string(),
        cidr_block: sdk.cidr_block().unwrap_or_default().to_string(),
        availability_zone: sdk.availability_zone().unwrap_or_default().to_string(),
        available_ip_count: sdk.available_ip_address_count().unwrap_or_default(),
        state: sdk
            .state()
            .map(|s| s.as_str().to_string())
            .unwrap_or_default(),
        is_default: sdk.default_for_az().unwrap_or_default(),
        map_public_ip_on_launch: sdk.map_public_ip_on_launch().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_ec2::types::Tag;

    fn build_sdk_vpc() -> aws_sdk_ec2::types::Vpc {
        aws_sdk_ec2::types::Vpc::builder()
            .vpc_id("vpc-0abc1234def56789")
            .cidr_block("10.0.0.0/16")
            .state(aws_sdk_ec2::types::VpcState::Available)
            .is_default(false)
            .owner_id("123456789012")
            .tags(Tag::builder().key("Name").value("my-vpc").build())
            .tags(
                Tag::builder()
                    .key("Environment")
                    .value("production")
                    .build(),
            )
            .build()
    }

    fn build_sdk_subnet() -> aws_sdk_ec2::types::Subnet {
        aws_sdk_ec2::types::Subnet::builder()
            .subnet_id("subnet-0abc1234def56789")
            .vpc_id("vpc-0abc1234def56789")
            .cidr_block("10.0.1.0/24")
            .availability_zone("ap-northeast-1a")
            .available_ip_address_count(251)
            .state(aws_sdk_ec2::types::SubnetState::Available)
            .default_for_az(false)
            .map_public_ip_on_launch(true)
            .tags(Tag::builder().key("Name").value("public-subnet-1a").build())
            .build()
    }

    #[test]
    fn convert_vpc_returns_vpc_id_when_valid_sdk_vpc() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.vpc_id, "vpc-0abc1234def56789");
    }

    #[test]
    fn convert_vpc_returns_name_from_tags_when_name_tag_exists() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.name, "my-vpc");
    }

    #[test]
    fn convert_vpc_returns_empty_name_when_no_name_tag() {
        let sdk = aws_sdk_ec2::types::Vpc::builder()
            .vpc_id("vpc-noname")
            .build();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.name, "");
    }

    #[test]
    fn convert_vpc_returns_cidr_block_when_set() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.cidr_block, "10.0.0.0/16");
    }

    #[test]
    fn convert_vpc_returns_available_state_when_sdk_state_available() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.state, "available");
    }

    #[test]
    fn convert_vpc_returns_is_default_false_when_not_default() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert!(!vpc.is_default);
    }

    #[test]
    fn convert_vpc_returns_is_default_true_when_default() {
        let sdk = aws_sdk_ec2::types::Vpc::builder()
            .vpc_id("vpc-default")
            .is_default(true)
            .build();
        let vpc = convert_vpc(&sdk);
        assert!(vpc.is_default);
    }

    #[test]
    fn convert_vpc_returns_owner_id_when_set() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.owner_id, "123456789012");
    }

    #[test]
    fn convert_vpc_returns_tags_as_hashmap_when_tags_set() {
        let sdk = build_sdk_vpc();
        let vpc = convert_vpc(&sdk);
        assert_eq!(vpc.tags.get("Name"), Some(&"my-vpc".to_string()));
        assert_eq!(vpc.tags.get("Environment"), Some(&"production".to_string()));
    }

    #[test]
    fn convert_subnet_returns_subnet_id_when_valid_sdk_subnet() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.subnet_id, "subnet-0abc1234def56789");
    }

    #[test]
    fn convert_subnet_returns_name_from_tags_when_name_tag_exists() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.name, "public-subnet-1a");
    }

    #[test]
    fn convert_subnet_returns_empty_name_when_no_name_tag() {
        let sdk = aws_sdk_ec2::types::Subnet::builder()
            .subnet_id("subnet-noname")
            .build();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.name, "");
    }

    #[test]
    fn convert_subnet_returns_vpc_id_when_set() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.vpc_id, "vpc-0abc1234def56789");
    }

    #[test]
    fn convert_subnet_returns_cidr_block_when_set() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.cidr_block, "10.0.1.0/24");
    }

    #[test]
    fn convert_subnet_returns_availability_zone_when_set() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.availability_zone, "ap-northeast-1a");
    }

    #[test]
    fn convert_subnet_returns_available_ip_count_when_set() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.available_ip_count, 251);
    }

    #[test]
    fn convert_subnet_returns_available_state_when_sdk_state_available() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert_eq!(subnet.state, "available");
    }

    #[test]
    fn convert_subnet_returns_is_default_false_when_not_default() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert!(!subnet.is_default);
    }

    #[test]
    fn convert_subnet_returns_map_public_ip_true_when_enabled() {
        let sdk = build_sdk_subnet();
        let subnet = convert_subnet(&sdk);
        assert!(subnet.map_public_ip_on_launch);
    }

    #[test]
    fn convert_subnet_returns_map_public_ip_false_when_disabled() {
        let sdk = aws_sdk_ec2::types::Subnet::builder()
            .subnet_id("subnet-nopub")
            .map_public_ip_on_launch(false)
            .build();
        let subnet = convert_subnet(&sdk);
        assert!(!subnet.map_public_ip_on_launch);
    }
}
