use awsui::app::App;
use awsui::event::AppEvent;
use awsui::tab::TabId;

use crate::clients::Clients;

pub(crate) fn handle_form_side_effect(
    app: &mut App,
    clients: &Clients,
    form_ctx: awsui::app::FormContext,
    tab_id: TabId,
) {
    use awsui::app::FormKind;
    let tx = app.event_tx.clone();
    let values: Vec<String> = form_ctx
        .fields
        .iter()
        .map(|f| f.input.value().to_string())
        .collect();

    match form_ctx.kind {
        FormKind::CreateS3Bucket => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                let bucket_name = values[0].clone();
                tokio::spawn(async move {
                    let result = c.create_bucket(&bucket_name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Bucket '{}' created", bucket_name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::CreateSecret => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                let name = values[0].clone();
                let value = values[1].clone();
                let description = if values.len() > 2 && !values[2].is_empty() {
                    Some(values[2].clone())
                } else {
                    None
                };
                tokio::spawn(async move {
                    let result = c.create_secret(&name, &value, description).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Secret '{}' created", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::UpdateSecretValue => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                let secret_id = app
                    .active_tab()
                    .and_then(|t| {
                        if let awsui::tab::ServiceData::Secrets { detail, .. } = &t.data {
                            detail.as_ref().map(|d| d.arn.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default();
                let new_value = values[0].clone();
                tokio::spawn(async move {
                    let result = c.update_secret_value(&secret_id, &new_value).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::CrudCompleted(tab_id, Ok("Secret value updated".to_string()))
                        }
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}

pub(crate) fn handle_danger_side_effect(
    app: &mut App,
    clients: &Clients,
    danger_action: awsui::app::DangerAction,
    tab_id: TabId,
) {
    use awsui::app::DangerAction;
    let tx = app.event_tx.clone();

    match danger_action {
        DangerAction::TerminateEc2(id) => {
            if let Some(client) = &clients.ec2 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.terminate_instances(std::slice::from_ref(&id)).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Instance {} terminated", id)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteS3Bucket(name) => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_bucket(&name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Bucket '{}' deleted", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteS3Object { bucket, key } => {
            if let Some(client) = &clients.s3 {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_object(&bucket, &key).await;
                    let event = match result {
                        Ok(()) => {
                            AppEvent::CrudCompleted(tab_id, Ok(format!("Object '{}' deleted", key)))
                        }
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteSecret(name) => {
            if let Some(client) = &clients.secrets {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_secret(&name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Secret '{}' deleted", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}
