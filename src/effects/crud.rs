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
        FormKind::CreateEcrRepository => {
            if let Some(client) = &clients.ecr {
                let c = client.clone();
                let repo_name = values[0].clone();
                let tag_mutability = values[1].clone();
                tokio::spawn(async move {
                    let result = c.create_repository(&repo_name, &tag_mutability).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Repository '{}' created", repo_name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::DownloadS3Object => {
            if let Some(client) = &clients.s3 {
                // タブからバケット名とオブジェクトキーを取得
                let Some((bucket_name, object_key)) = app.active_tab().and_then(|t| {
                    if let awsui::tab::ServiceData::S3 {
                        selected_bucket,
                        objects,
                        ..
                    } = &t.data
                    {
                        let bucket = selected_bucket.clone()?;
                        let obj = objects.get(t.detail_tag_index)?;
                        if !obj.is_prefix {
                            Some((bucket, obj.key.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }) else {
                    return;
                };

                let save_dir = values[0].clone();

                // パス解決（ディレクトリ存在チェック含む）
                let Ok(file_path) =
                    awsui::aws::s3_model::resolve_download_path(&save_dir, &object_key)
                        .inspect_err(|e| {
                            let _ = tx.try_send(AppEvent::CrudCompleted(
                                tab_id,
                                Err(awsui::error::AppError::AwsApi(e.clone())),
                            ));
                        })
                else {
                    return;
                };

                let c = client.clone();
                tokio::spawn(async move {
                    let result = async {
                        let bytes = c.download_object(&bucket_name, &object_key).await?;
                        tokio::fs::write(&file_path, &bytes).await.map_err(|e| {
                            awsui::error::AppError::AwsApi(format!("Failed to write file: {}", e))
                        })?;
                        Ok::<_, awsui::error::AppError>(format!(
                            "Downloaded '{}' to {}",
                            object_key,
                            file_path.display()
                        ))
                    }
                    .await;
                    let event = match result {
                        Ok(msg) => AppEvent::CrudCompleted(tab_id, Ok(msg)),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::UploadS3Object => {
            if let Some(client) = &clients.s3 {
                // タブからバケット名とcurrent_prefixを取得
                let Some((bucket_name, current_prefix)) = app.active_tab().and_then(|t| {
                    if let awsui::tab::ServiceData::S3 {
                        selected_bucket,
                        current_prefix,
                        ..
                    } = &t.data
                    {
                        Some((selected_bucket.clone()?, current_prefix.clone()))
                    } else {
                        None
                    }
                }) else {
                    return;
                };

                let local_file_path = values[0].clone();

                // S3キー構築（prefix + basename）
                let Ok(s3_key) =
                    awsui::aws::s3_model::resolve_upload_key(&current_prefix, &local_file_path)
                        .inspect_err(|e| {
                            let _ = tx.try_send(AppEvent::CrudCompleted(
                                tab_id,
                                Err(awsui::error::AppError::AwsApi(e.clone())),
                            ));
                        })
                else {
                    return;
                };

                let c = client.clone();
                tokio::spawn(async move {
                    let result = async {
                        let bytes = tokio::fs::read(&local_file_path).await.map_err(|e| {
                            awsui::error::AppError::AwsApi(format!("Failed to read file: {}", e))
                        })?;
                        c.put_object(&bucket_name, &s3_key, bytes).await?;
                        Ok::<_, awsui::error::AppError>(format!(
                            "Uploaded '{}' to s3://{}/{}",
                            local_file_path, bucket_name, s3_key
                        ))
                    }
                    .await;
                    let event = match result {
                        Ok(msg) => AppEvent::CrudCompleted(tab_id, Ok(msg)),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        FormKind::ScaleEcsService => {
            if let Some(client) = clients.ecs.clone() {
                // handle_form_submitでバリデーション済みのため parse() は必ず成功する
                let desired_count: i32 = values[0]
                    .parse()
                    .expect("validated as non-negative integer before submission");
                let Some((cluster_arn, service_name)) = app.active_tab().and_then(|t| {
                    if let awsui::tab::ServiceData::Ecs {
                        services,
                        nav_level:
                            Some(awsui::tab::EcsNavLevel::ServiceDetail { service_index, .. }),
                        ..
                    } = &t.data
                        && let Some(svc) = services.get(*service_index)
                    {
                        Some((svc.cluster_arn.clone(), svc.service_name.clone()))
                    } else {
                        None
                    }
                }) else {
                    return;
                };
                tokio::spawn(async move {
                    let result = client
                        .update_service_desired_count(&cluster_arn, &service_name, desired_count)
                        .await;
                    let event = match result {
                        Ok(()) => AppEvent::TabEvent(
                            tab_id,
                            awsui::event::TabEvent::ActionCompleted(Ok(format!(
                                "Desired count updated to {} for {}",
                                desired_count, service_name
                            ))),
                        ),
                        Err(e) => AppEvent::TabEvent(
                            tab_id,
                            awsui::event::TabEvent::ActionCompleted(Err(e)),
                        ),
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
        DangerAction::DeleteEcrRepository(name) => {
            if let Some(client) = &clients.ecr {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c.delete_repository(&name).await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Repository '{}' deleted", name)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
        DangerAction::DeleteEcrImage {
            repository_name,
            image_digest,
        } => {
            if let Some(client) = &clients.ecr {
                let c = client.clone();
                tokio::spawn(async move {
                    let result = c
                        .delete_images(&repository_name, std::slice::from_ref(&image_digest))
                        .await;
                    let event = match result {
                        Ok(()) => AppEvent::CrudCompleted(
                            tab_id,
                            Ok(format!("Image '{}' deleted", image_digest)),
                        ),
                        Err(e) => AppEvent::CrudCompleted(tab_id, Err(e)),
                    };
                    let _ = tx.send(event).await;
                });
            }
        }
    }
}
