#![cfg(feature = "mock-data")]

use std::collections::HashMap;

use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, Container, ContainerLogConfig, Service, Task};
use crate::aws::logs_model::LogEvent;
use crate::aws::model::{Instance, InstanceState, Volume};
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail, SecretVersion};
use crate::aws::vpc_model::{Subnet, Vpc};

pub fn mock_instances() -> Vec<Instance> {
    vec![
        Instance {
            instance_id: "i-0abc1234def56789a".to_string(),
            name: "web-server-1".to_string(),
            state: InstanceState::Running,
            instance_type: "t3.medium".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            private_ip: Some("10.0.1.10".to_string()),
            public_ip: Some("54.199.100.1".to_string()),
            vpc_id: Some("vpc-0abc1234def56789a".to_string()),
            subnet_id: Some("subnet-0abc1234def56789a".to_string()),
            ami_id: "ami-0abc1234def56789a".to_string(),
            key_name: Some("my-key-pair".to_string()),
            platform: None,
            launch_time: Some("2024-01-15T09:30:00Z".to_string()),
            security_groups: vec!["sg-web-public".to_string()],
            volumes: vec![Volume {
                volume_id: "vol-0abc1234def56789a".to_string(),
                volume_type: "gp3".to_string(),
                size_gb: 20,
                device_name: "/dev/xvda".to_string(),
                state: "in-use".to_string(),
            }],
            tags: HashMap::from([
                ("Environment".to_string(), "production".to_string()),
                ("Team".to_string(), "platform".to_string()),
            ]),
        },
        Instance {
            instance_id: "i-0bcd2345efg67890b".to_string(),
            name: "api-server-1".to_string(),
            state: InstanceState::Running,
            instance_type: "t3.large".to_string(),
            availability_zone: "ap-northeast-1c".to_string(),
            private_ip: Some("10.0.2.20".to_string()),
            public_ip: None,
            vpc_id: Some("vpc-0abc1234def56789a".to_string()),
            subnet_id: Some("subnet-0bcd2345efg67890b".to_string()),
            ami_id: "ami-0bcd2345efg67890b".to_string(),
            key_name: Some("my-key-pair".to_string()),
            platform: None,
            launch_time: Some("2024-02-20T14:00:00Z".to_string()),
            security_groups: vec!["sg-api-internal".to_string()],
            volumes: vec![Volume {
                volume_id: "vol-0bcd2345efg67890b".to_string(),
                volume_type: "gp3".to_string(),
                size_gb: 50,
                device_name: "/dev/xvda".to_string(),
                state: "in-use".to_string(),
            }],
            tags: HashMap::from([
                ("Environment".to_string(), "production".to_string()),
                ("Team".to_string(), "backend".to_string()),
            ]),
        },
        Instance {
            instance_id: "i-0cde3456fgh78901c".to_string(),
            name: "batch-worker-1".to_string(),
            state: InstanceState::Stopped,
            instance_type: "c5.xlarge".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            private_ip: Some("10.0.3.30".to_string()),
            public_ip: None,
            vpc_id: Some("vpc-0abc1234def56789a".to_string()),
            subnet_id: Some("subnet-0abc1234def56789a".to_string()),
            ami_id: "ami-0cde3456fgh78901c".to_string(),
            key_name: Some("batch-key".to_string()),
            platform: None,
            launch_time: Some("2024-03-10T08:00:00Z".to_string()),
            security_groups: vec!["sg-batch-internal".to_string()],
            volumes: vec![],
            tags: HashMap::from([("Environment".to_string(), "staging".to_string())]),
        },
        Instance {
            instance_id: "i-0def4567ghi89012d".to_string(),
            name: "dev-instance".to_string(),
            state: InstanceState::Pending,
            instance_type: "t3.micro".to_string(),
            availability_zone: "ap-northeast-1c".to_string(),
            private_ip: None,
            public_ip: None,
            vpc_id: Some("vpc-0abc1234def56789a".to_string()),
            subnet_id: Some("subnet-0bcd2345efg67890b".to_string()),
            ami_id: "ami-0def4567ghi89012d".to_string(),
            key_name: None,
            platform: Some("windows".to_string()),
            launch_time: Some("2024-06-01T12:00:00Z".to_string()),
            security_groups: vec!["sg-dev".to_string()],
            volumes: vec![],
            tags: HashMap::new(),
        },
    ]
}

pub fn mock_repositories() -> Vec<Repository> {
    vec![
        Repository {
            repository_name: "myapp/web".to_string(),
            repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web"
                .to_string(),
            registry_id: "123456789012".to_string(),
            created_at: Some("2024-01-10T00:00:00Z".to_string()),
            image_tag_mutability: "MUTABLE".to_string(),
        },
        Repository {
            repository_name: "myapp/api".to_string(),
            repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/api"
                .to_string(),
            registry_id: "123456789012".to_string(),
            created_at: Some("2024-01-10T00:00:00Z".to_string()),
            image_tag_mutability: "IMMUTABLE".to_string(),
        },
        Repository {
            repository_name: "shared/nginx".to_string(),
            repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/shared/nginx"
                .to_string(),
            registry_id: "123456789012".to_string(),
            created_at: Some("2023-12-01T00:00:00Z".to_string()),
            image_tag_mutability: "MUTABLE".to_string(),
        },
    ]
}

pub fn mock_images() -> Vec<Image> {
    vec![
        Image {
            image_digest: "sha256:abc123def456".to_string(),
            image_tags: vec!["latest".to_string(), "v1.2.3".to_string()],
            pushed_at: Some("2024-06-01T10:00:00Z".to_string()),
            image_size_bytes: Some(150_000_000),
        },
        Image {
            image_digest: "sha256:bcd234efg567".to_string(),
            image_tags: vec!["v1.2.2".to_string()],
            pushed_at: Some("2024-05-15T08:00:00Z".to_string()),
            image_size_bytes: Some(148_000_000),
        },
    ]
}

pub fn mock_clusters() -> Vec<Cluster> {
    vec![
        Cluster {
            cluster_name: "production".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
            status: "ACTIVE".to_string(),
            running_tasks_count: 5,
            pending_tasks_count: 0,
            active_services_count: 3,
            registered_container_instances_count: 0,
        },
        Cluster {
            cluster_name: "staging".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/staging".to_string(),
            status: "ACTIVE".to_string(),
            running_tasks_count: 2,
            pending_tasks_count: 1,
            active_services_count: 2,
            registered_container_instances_count: 0,
        },
    ]
}

pub fn mock_services() -> Vec<Service> {
    vec![
        Service {
            service_name: "web-service".to_string(),
            service_arn: "arn:aws:ecs:ap-northeast-1:123456789012:service/production/web-service"
                .to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
            status: "ACTIVE".to_string(),
            desired_count: 3,
            running_count: 3,
            pending_count: 0,
            task_definition: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/web:10"
                .to_string(),
            launch_type: Some("FARGATE".to_string()),
            scheduling_strategy: Some("REPLICA".to_string()),
            created_at: Some("2024-01-15T00:00:00Z".to_string()),
            health_check_grace_period_seconds: Some(60),
            deployment_status: Some("PRIMARY".to_string()),
        },
        Service {
            service_name: "api-service".to_string(),
            service_arn: "arn:aws:ecs:ap-northeast-1:123456789012:service/production/api-service"
                .to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
            status: "ACTIVE".to_string(),
            desired_count: 2,
            running_count: 2,
            pending_count: 0,
            task_definition: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/api:5"
                .to_string(),
            launch_type: Some("FARGATE".to_string()),
            scheduling_strategy: Some("REPLICA".to_string()),
            created_at: Some("2024-02-01T00:00:00Z".to_string()),
            health_check_grace_period_seconds: Some(30),
            deployment_status: Some("PRIMARY".to_string()),
        },
    ]
}

pub fn mock_tasks() -> Vec<Task> {
    vec![
        Task {
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/abc123".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
            task_definition_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/web:10"
                .to_string(),
            last_status: "RUNNING".to_string(),
            desired_status: "RUNNING".to_string(),
            cpu: Some("256".to_string()),
            memory: Some("512".to_string()),
            launch_type: Some("FARGATE".to_string()),
            platform_version: Some("1.4.0".to_string()),
            health_status: Some("HEALTHY".to_string()),
            connectivity: Some("CONNECTED".to_string()),
            availability_zone: Some("ap-northeast-1a".to_string()),
            started_at: Some("2024-06-01T10:00:00Z".to_string()),
            stopped_at: None,
            stopped_reason: None,
            containers: vec![Container {
                name: "web".to_string(),
                image: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web:latest"
                    .to_string(),
                last_status: "RUNNING".to_string(),
                exit_code: None,
                health_status: Some("HEALTHY".to_string()),
                reason: None,
            }],
        },
        Task {
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/def456".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
            task_definition_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/web:10"
                .to_string(),
            last_status: "RUNNING".to_string(),
            desired_status: "RUNNING".to_string(),
            cpu: Some("256".to_string()),
            memory: Some("512".to_string()),
            launch_type: Some("FARGATE".to_string()),
            platform_version: Some("1.4.0".to_string()),
            health_status: Some("HEALTHY".to_string()),
            connectivity: Some("CONNECTED".to_string()),
            availability_zone: Some("ap-northeast-1c".to_string()),
            started_at: Some("2024-06-01T10:05:00Z".to_string()),
            stopped_at: None,
            stopped_reason: None,
            containers: vec![Container {
                name: "web".to_string(),
                image: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web:latest"
                    .to_string(),
                last_status: "RUNNING".to_string(),
                exit_code: None,
                health_status: Some("HEALTHY".to_string()),
                reason: None,
            }],
        },
    ]
}

pub fn mock_log_configs() -> Vec<ContainerLogConfig> {
    vec![ContainerLogConfig {
        container_name: "web".to_string(),
        log_group: Some("/ecs/production/web".to_string()),
        stream_prefix: Some("ecs".to_string()),
        region: Some("ap-northeast-1".to_string()),
    }]
}

pub fn mock_log_events() -> Vec<LogEvent> {
    vec![
        LogEvent {
            timestamp: 1717236000000,
            formatted_time: "2024-06-01 10:00:00".to_string(),
            message: "Server started on port 8080".to_string(),
        },
        LogEvent {
            timestamp: 1717236060000,
            formatted_time: "2024-06-01 10:01:00".to_string(),
            message: "GET /health 200 OK".to_string(),
        },
        LogEvent {
            timestamp: 1717236120000,
            formatted_time: "2024-06-01 10:02:00".to_string(),
            message: "GET /api/users 200 OK".to_string(),
        },
    ]
}

pub fn mock_buckets() -> Vec<Bucket> {
    vec![
        Bucket {
            name: "my-app-assets-prod".to_string(),
            creation_date: Some("2024-01-05T00:00:00Z".to_string()),
        },
        Bucket {
            name: "my-app-logs".to_string(),
            creation_date: Some("2024-01-05T00:00:00Z".to_string()),
        },
        Bucket {
            name: "my-app-backups".to_string(),
            creation_date: Some("2024-03-01T00:00:00Z".to_string()),
        },
    ]
}

pub fn mock_objects() -> Vec<S3Object> {
    vec![
        S3Object {
            key: "images/".to_string(),
            size: None,
            last_modified: None,
            storage_class: None,
            is_prefix: true,
        },
        S3Object {
            key: "index.html".to_string(),
            size: Some(4096),
            last_modified: Some("2024-06-01T00:00:00Z".to_string()),
            storage_class: Some("STANDARD".to_string()),
            is_prefix: false,
        },
        S3Object {
            key: "styles.css".to_string(),
            size: Some(2048),
            last_modified: Some("2024-06-01T00:00:00Z".to_string()),
            storage_class: Some("STANDARD".to_string()),
            is_prefix: false,
        },
    ]
}

pub fn mock_vpcs() -> Vec<Vpc> {
    vec![
        Vpc {
            vpc_id: "vpc-0abc1234def56789a".to_string(),
            name: "main-vpc".to_string(),
            cidr_block: "10.0.0.0/16".to_string(),
            state: "available".to_string(),
            is_default: false,
            owner_id: "123456789012".to_string(),
            tags: HashMap::from([("Environment".to_string(), "production".to_string())]),
        },
        Vpc {
            vpc_id: "vpc-0default".to_string(),
            name: "default".to_string(),
            cidr_block: "172.31.0.0/16".to_string(),
            state: "available".to_string(),
            is_default: true,
            owner_id: "123456789012".to_string(),
            tags: HashMap::new(),
        },
    ]
}

pub fn mock_subnets() -> Vec<Subnet> {
    vec![
        Subnet {
            subnet_id: "subnet-0abc1234def56789a".to_string(),
            name: "public-subnet-1a".to_string(),
            vpc_id: "vpc-0abc1234def56789a".to_string(),
            cidr_block: "10.0.1.0/24".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            available_ip_count: 250,
            state: "available".to_string(),
            is_default: false,
            map_public_ip_on_launch: true,
        },
        Subnet {
            subnet_id: "subnet-0bcd2345efg67890b".to_string(),
            name: "private-subnet-1c".to_string(),
            vpc_id: "vpc-0abc1234def56789a".to_string(),
            cidr_block: "10.0.2.0/24".to_string(),
            availability_zone: "ap-northeast-1c".to_string(),
            available_ip_count: 248,
            state: "available".to_string(),
            is_default: false,
            map_public_ip_on_launch: false,
        },
        Subnet {
            subnet_id: "subnet-0cde3456fgh78901c".to_string(),
            name: "private-subnet-1a".to_string(),
            vpc_id: "vpc-0abc1234def56789a".to_string(),
            cidr_block: "10.0.3.0/24".to_string(),
            availability_zone: "ap-northeast-1a".to_string(),
            available_ip_count: 252,
            state: "available".to_string(),
            is_default: false,
            map_public_ip_on_launch: false,
        },
    ]
}

pub fn mock_secrets() -> Vec<Secret> {
    vec![
        Secret {
            name: "prod/database/password".to_string(),
            arn: "arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:prod/database/password-AbCdEf".to_string(),
            description: Some("Production database password".to_string()),
            last_changed_date: Some("2024-05-01T00:00:00Z".to_string()),
            last_accessed_date: Some("2024-06-01T00:00:00Z".to_string()),
            tags: HashMap::from([("Environment".to_string(), "production".to_string())]),
        },
        Secret {
            name: "prod/api/key".to_string(),
            arn: "arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:prod/api/key-GhIjKl".to_string(),
            description: Some("External API key".to_string()),
            last_changed_date: Some("2024-04-15T00:00:00Z".to_string()),
            last_accessed_date: Some("2024-06-01T00:00:00Z".to_string()),
            tags: HashMap::new(),
        },
        Secret {
            name: "staging/config".to_string(),
            arn: "arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:staging/config-MnOpQr".to_string(),
            description: None,
            last_changed_date: Some("2024-03-20T00:00:00Z".to_string()),
            last_accessed_date: None,
            tags: HashMap::from([("Environment".to_string(), "staging".to_string())]),
        },
    ]
}

pub fn mock_secret_detail() -> SecretDetail {
    SecretDetail {
        name: "prod/database/password".to_string(),
        arn: "arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:prod/database/password-AbCdEf".to_string(),
        description: Some("Production database password".to_string()),
        kms_key_id: Some("alias/aws/secretsmanager".to_string()),
        rotation_enabled: false,
        rotation_lambda_arn: None,
        rotation_days: None,
        last_rotated_date: None,
        last_changed_date: Some("2024-05-01T00:00:00Z".to_string()),
        last_accessed_date: Some("2024-06-01T00:00:00Z".to_string()),
        created_date: Some("2024-01-01T00:00:00Z".to_string()),
        tags: HashMap::from([("Environment".to_string(), "production".to_string())]),
        version_ids: vec!["v1".to_string()],
        version_stages: vec![SecretVersion {
            version_id: "v1".to_string(),
            staging_labels: vec!["AWSCURRENT".to_string()],
        }],
        secret_value: None,
    }
}
