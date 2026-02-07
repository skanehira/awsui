use std::collections::HashMap;

use async_trait::async_trait;

use crate::aws::vpc_model::{Subnet, Vpc};
use crate::error::{AppError, format_error_chain};

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
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
                .map_err(|e| AppError::AwsApi(format_error_chain(&e)))?;

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
