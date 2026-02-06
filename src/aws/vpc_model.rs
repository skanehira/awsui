use std::collections::HashMap;

/// VPCのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vpc {
    pub vpc_id: String,
    pub name: String,
    pub cidr_block: String,
    pub state: String,
    pub is_default: bool,
    pub owner_id: String,
    pub tags: HashMap<String, String>,
}

/// サブネットのドメインモデル
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subnet {
    pub subnet_id: String,
    pub name: String,
    pub vpc_id: String,
    pub cidr_block: String,
    pub availability_zone: String,
    pub available_ip_count: i32,
    pub state: String,
    pub is_default: bool,
    pub map_public_ip_on_launch: bool,
}
