use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::service::ServiceKind;

const MAX_RECENT: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub service: ServiceKind,
}

/// 最近使用したサービスの履歴を読み込む
pub fn load_recent() -> Vec<RecentEntry> {
    let Some(path) = recent_file_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// 最近使用したサービスの履歴を保存する
pub fn save_recent(entries: &[RecentEntry]) {
    let Some(path) = recent_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(json) = serde_json::to_string_pretty(entries) else {
        return;
    };
    let _ = std::fs::write(&path, json);
}

/// サービスを使用履歴に追加（先頭に移動、重複除去、上限10件）
pub fn update_recent(service: ServiceKind) {
    let mut entries = load_recent();
    entries.retain(|e| e.service != service);
    entries.insert(0, RecentEntry { service });
    entries.truncate(MAX_RECENT);
    save_recent(&entries);
}

/// インメモリで履歴リストを更新する（先頭に移動、重複除去、上限10件）
pub fn apply_recent_update(services: &mut Vec<ServiceKind>, service: ServiceKind) {
    services.retain(|&s| s != service);
    services.insert(0, service);
    services.truncate(MAX_RECENT);
}

fn recent_file_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|home| {
        PathBuf::from(home)
            .join(".config")
            .join("awsui")
            .join("recent.json")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_recent_update_returns_service_at_front_when_existing_service_added() {
        let mut services = vec![ServiceKind::S3, ServiceKind::Ec2];
        apply_recent_update(&mut services, ServiceKind::Ec2);
        assert_eq!(services, vec![ServiceKind::Ec2, ServiceKind::S3]);
    }

    #[test]
    fn apply_recent_update_returns_new_service_at_front_when_new_service_added() {
        let mut services = vec![ServiceKind::S3];
        apply_recent_update(&mut services, ServiceKind::Ecs);
        assert_eq!(services, vec![ServiceKind::Ecs, ServiceKind::S3]);
    }

    #[test]
    fn apply_recent_update_returns_truncated_list_when_exceeding_max() {
        let mut services: Vec<ServiceKind> = ServiceKind::ALL.to_vec();
        // 追加し続けても MAX_RECENT を超えない
        for _ in 0..15 {
            apply_recent_update(&mut services, ServiceKind::Ec2);
        }
        assert!(services.len() <= MAX_RECENT);
    }

    #[test]
    fn serialize_returns_valid_json_when_entries_serialized() {
        let entries = vec![
            RecentEntry {
                service: ServiceKind::Ec2,
            },
            RecentEntry {
                service: ServiceKind::S3,
            },
        ];
        let json = serde_json::to_string(&entries).unwrap();
        let deserialized: Vec<RecentEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].service, ServiceKind::Ec2);
        assert_eq!(deserialized[1].service, ServiceKind::S3);
    }
}
