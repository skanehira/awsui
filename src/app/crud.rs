use tui_input::Input;

use super::App;
use crate::service::ServiceKind;
use crate::ui_state::*;

impl App {
    /// Create操作のハンドリング
    pub(super) fn handle_create(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        let form_ctx = match view {
            (ServiceKind::Ecr, crate::tab::TabView::List) => Some(FormContext {
                kind: FormKind::CreateEcrRepository,
                fields: vec![
                    FormField {
                        label: "Repository Name".to_string(),
                        input: Input::default(),
                        required: true,
                    },
                    FormField {
                        label: "Tag Mutability (MUTABLE/IMMUTABLE)".to_string(),
                        input: Input::from("MUTABLE"),
                        required: true,
                    },
                ],
                focused_field: 0,
            }),
            (ServiceKind::S3, crate::tab::TabView::List) => Some(FormContext {
                kind: FormKind::CreateS3Bucket,
                fields: vec![FormField {
                    label: "Bucket Name".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            }),
            (ServiceKind::SecretsManager, crate::tab::TabView::List) => Some(FormContext {
                kind: FormKind::CreateSecret,
                fields: vec![
                    FormField {
                        label: "Name".to_string(),
                        input: Input::default(),
                        required: true,
                    },
                    FormField {
                        label: "Value".to_string(),
                        input: Input::default(),
                        required: true,
                    },
                    FormField {
                        label: "Description".to_string(),
                        input: Input::default(),
                        required: false,
                    },
                ],
                focused_field: 0,
            }),
            _ => None,
        };
        if let Some(ctx) = form_ctx
            && let Some(tab) = self.active_tab_mut()
        {
            tab.mode = Mode::Form(ctx);
        }
    }

    /// 削除権限がなければエラーメッセージを表示して false を返す
    fn check_delete_permission(&mut self, service: &str) -> bool {
        if self.can_delete(service) {
            return true;
        }
        self.show_message(
            MessageLevel::Error,
            "Permission Denied",
            format!("Delete not allowed. Use --allow-delete={service} or --allow-delete"),
        );
        false
    }

    /// Delete操作のハンドリング
    pub(super) fn handle_delete(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        match view {
            (ServiceKind::Ecr, crate::tab::TabView::List) => {
                if !self.check_delete_permission("ecr") {
                    return;
                }
                let repo_name = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::Ecr { repositories, .. } = &tab.data {
                        repositories
                            .filtered
                            .get(tab.selected_index)
                            .map(|r| r.repository_name.clone())
                    } else {
                        None
                    }
                });
                let Some(name) = repo_name else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteEcrRepository(name),
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::Ecr, crate::tab::TabView::Detail) => {
                if !self.check_delete_permission("ecr") {
                    return;
                }
                let image_info = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::Ecr {
                        repositories,
                        images,
                        ..
                    } = &tab.data
                    {
                        let repo_name = repositories
                            .filtered
                            .get(tab.selected_index)
                            .map(|r| r.repository_name.clone())?;
                        let image = images.get(tab.detail_tag_index)?;
                        Some((repo_name, image.image_digest.clone()))
                    } else {
                        None
                    }
                });
                let Some((repository_name, image_digest)) = image_info else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteEcrImage {
                            repository_name,
                            image_digest,
                        },
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::Ec2, crate::tab::TabView::List) => {
                if !self.check_delete_permission("ec2") {
                    return;
                }
                let id = self.selected_instance().map(|i| i.instance_id.clone());
                let Some(id) = id else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::TerminateEc2(id),
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::S3, crate::tab::TabView::List) => {
                if !self.check_delete_permission("s3") {
                    return;
                }
                let bucket_name = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::S3 { buckets, .. } = &tab.data {
                        buckets
                            .filtered
                            .get(tab.selected_index)
                            .map(|b| b.name.clone())
                    } else {
                        None
                    }
                });
                let Some(name) = bucket_name else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Bucket(name),
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::S3, crate::tab::TabView::Detail) => {
                if !self.check_delete_permission("s3") {
                    return;
                }
                let obj_info = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::S3 {
                        objects,
                        selected_bucket,
                        ..
                    } = &tab.data
                    {
                        objects.get(tab.detail_tag_index).and_then(|obj| {
                            if !obj.is_prefix {
                                Some((selected_bucket.clone().unwrap_or_default(), obj.key.clone()))
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                });
                let Some((bucket, key)) = obj_info else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteS3Object { bucket, key },
                        input: Input::default(),
                    });
                }
            }
            (ServiceKind::SecretsManager, crate::tab::TabView::List) => {
                if !self.check_delete_permission("secretsmanager") {
                    return;
                }
                let secret_name = self.active_tab().and_then(|tab| {
                    if let crate::tab::ServiceData::Secrets { secrets, .. } = &tab.data {
                        secrets
                            .filtered
                            .get(tab.selected_index)
                            .map(|s| s.name.clone())
                    } else {
                        None
                    }
                });
                let Some(name) = secret_name else {
                    return;
                };
                if let Some(tab) = self.active_tab_mut() {
                    tab.mode = Mode::DangerConfirm(DangerConfirmContext {
                        action: DangerAction::DeleteSecret(name),
                        input: Input::default(),
                    });
                }
            }
            _ => {}
        }
    }

    /// Download操作のハンドリング（S3オブジェクトダウンロード）
    pub(super) fn handle_download(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        if view != (ServiceKind::S3, crate::tab::TabView::Detail) {
            return;
        }
        // prefixでないオブジェクトが選択されているか確認
        let is_file = self.active_tab().is_some_and(|tab| {
            if let crate::tab::ServiceData::S3 { objects, .. } = &tab.data {
                objects
                    .get(tab.detail_tag_index)
                    .is_some_and(|obj| !obj.is_prefix)
            } else {
                false
            }
        });
        if !is_file {
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::Form(FormContext {
                kind: FormKind::DownloadS3Object,
                fields: vec![FormField {
                    label: "Save Directory".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            });
        }
    }

    /// Upload操作のハンドリング（S3オブジェクトアップロード）
    pub(super) fn handle_upload(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        if view != (ServiceKind::S3, crate::tab::TabView::Detail) {
            return;
        }
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::Form(FormContext {
                kind: FormKind::UploadS3Object,
                fields: vec![FormField {
                    label: "Local File Path".to_string(),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            });
        }
    }

    /// Edit操作のハンドリング
    pub(super) fn handle_edit(&mut self) {
        let Some(view) = self.current_view() else {
            return;
        };
        if view != (ServiceKind::SecretsManager, crate::tab::TabView::Detail) {
            return;
        }
        let detail_name = self.active_tab().and_then(|tab| {
            if let crate::tab::ServiceData::Secrets { detail, .. } = &tab.data {
                detail.as_ref().map(|d| d.name.clone())
            } else {
                None
            }
        });
        let Some(name) = detail_name else {
            return;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.mode = Mode::Form(FormContext {
                kind: FormKind::UpdateSecretValue,
                fields: vec![FormField {
                    label: format!("New value for '{}'", name),
                    input: Input::default(),
                    required: true,
                }],
                focused_field: 0,
            });
        }
    }

    /// FormSubmitのハンドリング
    pub(super) fn handle_form_submit(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let Mode::Form(ctx) = &tab.mode else {
            return SideEffect::None;
        };

        // 必須フィールドのバリデーション
        for field in &ctx.fields {
            if field.required && field.input.value().is_empty() {
                let msg = format!("'{}' is required", field.label);
                self.show_message(MessageLevel::Error, "Validation Error", msg);
                return SideEffect::None;
            }
        }

        // ScaleEcsService: 0以上の整数バリデーション
        if ctx.kind == FormKind::ScaleEcsService {
            let value = ctx.fields[0].input.value();
            match value.parse::<i32>() {
                Ok(n) if n >= 0 => {}
                _ => {
                    self.show_message(
                        MessageLevel::Error,
                        "Validation Error",
                        "Desired count must be a non-negative integer".to_string(),
                    );
                    return SideEffect::None;
                }
            }
        }

        // FormContextを取り出してNormalに戻す
        let Some(tab) = self.active_tab_mut() else {
            return SideEffect::None;
        };
        let Mode::Form(ctx) = std::mem::replace(&mut tab.mode, Mode::Normal) else {
            return SideEffect::None;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.loading = true;
        }
        SideEffect::FormSubmit(ctx)
    }

    /// フォームの次のフィールドにフォーカスを移動
    pub(super) fn handle_form_next_field(&mut self) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::Form(ctx) = &mut tab.mode
        {
            ctx.focused_field = (ctx.focused_field + 1) % ctx.fields.len();
        }
    }

    /// フォーム入力のハンドリング
    pub(super) fn handle_form_input(&mut self, req: tui_input::InputRequest) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::Form(ctx) = &mut tab.mode
            && let Some(field) = ctx.fields.get_mut(ctx.focused_field)
        {
            field.input.handle(req);
        }
    }

    /// DangerConfirmSubmitのハンドリング
    pub(super) fn handle_danger_confirm_submit(&mut self) -> SideEffect {
        let Some(tab) = self.active_tab() else {
            return SideEffect::None;
        };
        let Mode::DangerConfirm(ctx) = &tab.mode else {
            return SideEffect::None;
        };

        if ctx.input.value() != ctx.action.confirm_text() {
            return SideEffect::None;
        }

        let Some(tab) = self.active_tab_mut() else {
            return SideEffect::None;
        };
        let Mode::DangerConfirm(ctx) = std::mem::replace(&mut tab.mode, Mode::Normal) else {
            return SideEffect::None;
        };
        if let Some(tab) = self.active_tab_mut() {
            tab.loading = true;
        }
        SideEffect::DangerAction(ctx.action)
    }

    /// DangerConfirm入力のハンドリング
    pub(super) fn handle_danger_confirm_input(&mut self, req: tui_input::InputRequest) {
        if let Some(tab) = self.active_tab_mut()
            && let Mode::DangerConfirm(ctx) = &mut tab.mode
        {
            ctx.input.handle(req);
        }
    }

    /// シークレット値の表示/非表示切り替え
    pub(super) fn reveal_secret_value(&mut self) {
        let Some(tab) = self.active_tab_mut() else {
            return;
        };
        if let crate::tab::ServiceData::Secrets {
            detail: Some(d),
            value_visible,
            ..
        } = &mut tab.data
        {
            if d.secret_value.is_some() {
                *value_visible = !*value_visible;
            } else {
                tab.loading = true;
            }
        }
    }
}
