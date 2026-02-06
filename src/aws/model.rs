use std::collections::HashMap;

/// EC2インスタンスの状態
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceState {
    Pending,
    Running,
    ShuttingDown,
    Terminated,
    Stopping,
    Stopped,
}

impl InstanceState {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::ShuttingDown => "shutting-down",
            Self::Terminated => "terminated",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
        }
    }

    pub fn icon(&self) -> &str {
        match self {
            Self::Running => "●",
            Self::Stopped => "○",
            Self::Pending | Self::Stopping | Self::ShuttingDown => "◐",
            Self::Terminated => "◌",
        }
    }
}

/// EC2インスタンスのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub instance_id: String,
    pub name: String,
    pub state: InstanceState,
    pub instance_type: String,
    pub availability_zone: String,
    pub private_ip: Option<String>,
    pub public_ip: Option<String>,
    pub vpc_id: Option<String>,
    pub subnet_id: Option<String>,
    pub ami_id: String,
    pub key_name: Option<String>,
    pub platform: Option<String>,
    pub launch_time: Option<String>,
    pub security_groups: Vec<String>,
    pub volumes: Vec<Volume>,
    pub tags: HashMap<String, String>,
}

/// EBSボリューム情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volume {
    pub volume_id: String,
    pub volume_type: String,
    pub size_gb: i32,
    pub device_name: String,
    pub state: String,
}
