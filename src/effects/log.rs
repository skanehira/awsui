use std::sync::Arc;

use awsui::app::App;
use awsui::event::{AppEvent, TabEvent};
use awsui::tab::TabId;

#[cfg(not(feature = "mock-data"))]
use awsui::aws::logs_client::AwsLogsClient;

use crate::clients::Clients;

/// CloudWatch Logsからログイベントを取得する
pub(crate) async fn fetch_log_events(
    app: &mut App,
    clients: &mut Clients,
    tab_id: TabId,
    log_group: &str,
    log_stream: &str,
    next_token: Option<String>,
) {
    // Logsクライアントがない場合は作成
    if clients.logs.is_none() {
        #[cfg(feature = "mock-data")]
        {
            clients.logs = Some(Arc::new(awsui::aws::mock_clients::MockLogsClientImpl));
        }
        #[cfg(not(feature = "mock-data"))]
        {
            let Some(profile_name) = &app.profile else {
                return;
            };
            let region = app
                .region
                .clone()
                .unwrap_or_else(|| "ap-northeast-1".to_string());
            match AwsLogsClient::new(profile_name, &region).await {
                Ok(client) => {
                    clients.logs = Some(Arc::new(client));
                }
                Err(e) => {
                    if let Some(tab) = app.find_tab_mut(tab_id) {
                        tab.loading = false;
                    }
                    app.show_message(awsui::app::MessageLevel::Error, "Error", e.to_string());
                    return;
                }
            }
        }
    }

    if let Some(client) = &clients.logs {
        let tx = app.event_tx.clone();
        let c = client.clone();
        let group = log_group.to_string();
        let stream = log_stream.to_string();
        tokio::spawn(async move {
            let result = c.get_log_events(&group, &stream, next_token).await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::EcsLogEventsLoaded(result),
                ))
                .await;
        });
    }
}

/// ログポーリングを管理する（ログビュー表示中は2秒間隔でポーリング、非表示時は停止）
pub(crate) fn manage_log_polling(
    app: &App,
    clients: &Clients,
    log_poll_handle: &mut Option<tokio::task::JoinHandle<()>>,
) {
    let should_poll = app.active_tab().is_some_and(|tab| {
        if let awsui::tab::ServiceData::Ecs {
            nav_level: Some(awsui::tab::EcsNavLevel::LogView { .. }),
            ..
        } = &tab.data
        {
            !tab.loading
        } else {
            false
        }
    });

    if should_poll {
        // 既にポーリング中なら何もしない
        if log_poll_handle.as_ref().is_some_and(|h| !h.is_finished()) {
            return;
        }

        // ログ情報を取得
        let Some(tab) = app.active_tab() else {
            return;
        };
        let tab_id = tab.id;
        let Some((log_group, log_stream, next_token)) = (|| {
            if let awsui::tab::ServiceData::Ecs {
                nav_level: Some(awsui::tab::EcsNavLevel::LogView { log_state, .. }),
                ..
            } = &tab.data
            {
                return Some((
                    log_state.log_group.clone(),
                    log_state.log_stream.clone(),
                    log_state.next_forward_token.clone(),
                ));
            }
            None
        })() else {
            return;
        };

        let Some(client) = clients.logs.clone() else {
            return;
        };
        let tx = app.event_tx.clone();

        *log_poll_handle = Some(tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let result = client
                .get_log_events(&log_group, &log_stream, next_token)
                .await;
            let _ = tx
                .send(AppEvent::TabEvent(
                    tab_id,
                    TabEvent::EcsLogEventsLoaded(result),
                ))
                .await;
        }));
    } else {
        // ログビューでなくなったらポーリング停止
        if let Some(handle) = log_poll_handle.take() {
            handle.abort();
        }
    }
}
