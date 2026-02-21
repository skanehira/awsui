use super::*;
use crate::action::Action;
use crate::aws::model::InstanceState;
use crate::cli::DeletePermissions;
use crate::config::SsoProfile;
use crate::error::AppError;
use crate::event::TabEvent;
use crate::tab::{ServiceData, TabView};
use std::collections::HashMap;

fn create_test_instance(id: &str, name: &str, state: InstanceState) -> Instance {
    Instance {
        instance_id: id.to_string(),
        name: name.to_string(),
        state,
        instance_type: "t3.micro".to_string(),
        availability_zone: "ap-northeast-1a".to_string(),
        private_ip: None,
        public_ip: None,
        vpc_id: None,
        subnet_id: None,
        ami_id: "ami-test".to_string(),
        key_name: None,
        platform: None,
        launch_time: None,
        security_groups: Vec::new(),
        volumes: Vec::new(),
        tags: HashMap::new(),
    }
}

fn app_with_ec2_tab() -> App {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ec2);
    app
}

fn set_ec2_instances(app: &mut App, data: Vec<Instance>) {
    let tab = app.active_tab_mut().unwrap();
    if let ServiceData::Ec2 { instances } = &mut tab.data {
        instances.set_items(data);
    }
    tab.loading = false;
}

// ──────────────────────────────────────────────
// create_tab テスト
// ──────────────────────────────────────────────

#[test]
fn create_tab_creates_tab_and_switches_to_it() {
    let mut app = App::new("dev".to_string(), None);
    assert!(app.tabs.is_empty());
    assert!(app.show_dashboard);

    let id = app.create_tab(ServiceKind::Ec2);
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_index, 0);
    assert!(!app.show_dashboard);
    assert_eq!(app.active_tab().unwrap().id, id);
    assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
}

// ──────────────────────────────────────────────
// dispatch Quit テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_sets_should_quit_when_quit_action() {
    let mut app = App::new("dev".to_string(), None);
    let result = app.dispatch(Action::Quit);
    assert!(app.should_quit);
    assert_eq!(result, SideEffect::None);
}

// ──────────────────────────────────────────────
// ダッシュボード移動テスト
// ──────────────────────────────────────────────

#[test]
fn move_down_increments_dashboard_selected_index_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    assert!(app.show_dashboard);
    assert_eq!(app.dashboard.selected_index, 0);
    app.dispatch(Action::MoveDown);
    assert_eq!(app.dashboard.selected_index, 1);
}

#[test]
fn move_up_decrements_dashboard_selected_index_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.selected_index = 2;
    app.dispatch(Action::MoveUp);
    assert_eq!(app.dashboard.selected_index, 1);
}

#[test]
fn move_to_top_sets_dashboard_index_to_zero() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.selected_index = 3;
    app.dispatch(Action::MoveToTop);
    assert_eq!(app.dashboard.selected_index, 0);
}

#[test]
fn move_to_bottom_sets_dashboard_index_to_last() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.recent_services.clear();
    app.dispatch(Action::MoveToBottom);
    assert_eq!(app.dashboard.selected_index, ServiceKind::ALL.len() - 1);
}

// ──────────────────────────────────────────────
// タブリスト移動テスト
// ──────────────────────────────────────────────

#[test]
fn move_down_increments_tab_selected_index_when_on_tab_list() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ],
    );
    app.dispatch(Action::MoveDown);
    assert_eq!(app.active_tab().unwrap().selected_index, 1);
}

#[test]
fn move_up_decrements_tab_selected_index_when_on_tab_list() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![
            create_test_instance("i-001", "a", InstanceState::Running),
            create_test_instance("i-002", "b", InstanceState::Stopped),
        ],
    );
    app.active_tab_mut().unwrap().selected_index = 1;
    app.dispatch(Action::MoveUp);
    assert_eq!(app.active_tab().unwrap().selected_index, 0);
}

// ──────────────────────────────────────────────
// タブ詳細移動テスト
// ──────────────────────────────────────────────

#[test]
fn move_down_increments_detail_tag_index_when_on_tab_detail() {
    let mut app = app_with_ec2_tab();
    let mut instance = create_test_instance("i-001", "web", InstanceState::Running);
    instance.tags.insert("env".to_string(), "prod".to_string());
    instance
        .tags
        .insert("team".to_string(), "backend".to_string());
    set_ec2_instances(&mut app, vec![instance]);
    // Switch to detail
    app.active_tab_mut().unwrap().tab_view = TabView::Detail;
    app.active_tab_mut().unwrap().detail_tab = DetailTab::Overview;
    app.dispatch(Action::MoveDown);
    assert_eq!(app.active_tab().unwrap().detail_tag_index, 1);
}

#[test]
fn move_up_decrements_detail_tag_index_when_on_tab_detail() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    let tab = app.active_tab_mut().unwrap();
    tab.tab_view = TabView::Detail;
    tab.detail_tag_index = 1;
    app.dispatch(Action::MoveUp);
    assert_eq!(app.active_tab().unwrap().detail_tag_index, 0);
}

// ──────────────────────────────────────────────
// handle_enter テスト
// ──────────────────────────────────────────────

#[test]
fn handle_enter_creates_tab_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.recent_services.clear();
    assert!(app.show_dashboard);
    app.dashboard.selected_index = 0; // EC2 (All Servicesの先頭)
    app.dispatch(Action::Enter);
    assert_eq!(app.tabs.len(), 1);
    assert!(!app.show_dashboard);
    assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
}

// ──────────────────────────────────────────────
// サービスピッカーテスト
// ──────────────────────────────────────────────

#[test]
fn new_tab_opens_service_picker() {
    let mut app = app_with_ec2_tab();
    assert!(app.service_picker.is_none());
    app.dispatch(Action::NewTab);
    assert!(app.service_picker.is_some());
    assert_eq!(
        app.service_picker.as_ref().unwrap().filtered_services.len(),
        ServiceKind::ALL.len()
    );
}

#[test]
fn picker_confirm_creates_tab_and_closes_picker() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::NewTab);
    assert!(app.service_picker.is_some());
    let old_tab_count = app.tabs.len();
    app.dispatch(Action::PickerConfirm);
    assert!(app.service_picker.is_none());
    assert_eq!(app.tabs.len(), old_tab_count + 1);
}

#[test]
fn picker_cancel_closes_picker_without_creating_tab() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::NewTab);
    let old_tab_count = app.tabs.len();
    app.dispatch(Action::PickerCancel);
    assert!(app.service_picker.is_none());
    assert_eq!(app.tabs.len(), old_tab_count);
}

#[test]
fn picker_move_down_increments_index() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::NewTab);
    assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 0);
    app.dispatch(Action::PickerMoveDown);
    assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 1);
}

#[test]
fn picker_move_up_decrements_index() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::NewTab);
    app.dispatch(Action::PickerMoveDown);
    app.dispatch(Action::PickerMoveDown);
    assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 2);
    app.dispatch(Action::PickerMoveUp);
    assert_eq!(app.service_picker.as_ref().unwrap().selected_index, 1);
}

#[test]
fn handle_enter_switches_to_detail_when_on_tab_list_with_data() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.dispatch(Action::Enter);
    assert_eq!(app.active_tab().unwrap().tab_view, TabView::Detail);
}

#[test]
fn handle_enter_stays_on_list_when_empty() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::Enter);
    assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
}

// ──────────────────────────────────────────────
// handle_back テスト
// ──────────────────────────────────────────────

#[test]
fn handle_back_switches_to_list_when_on_tab_detail() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.active_tab_mut().unwrap().tab_view = TabView::Detail;
    app.dispatch(Action::Back);
    assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
}

#[test]
fn handle_back_does_nothing_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dispatch(Action::Back);
    assert!(app.show_dashboard);
    assert!(!app.should_quit);
}

#[test]
fn handle_back_does_nothing_when_on_tab_list() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::Back);
    assert_eq!(app.active_tab().unwrap().tab_view, TabView::List);
}

// ──────────────────────────────────────────────
// show_message / dismiss_message テスト
// ──────────────────────────────────────────────

#[test]
fn show_message_sets_message_when_called() {
    let mut app = App::new("dev".to_string(), None);
    app.show_message(MessageLevel::Error, "Error", "Something failed");
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.title, "Error");
    assert_eq!(msg.body, "Something failed");
}

#[test]
fn dismiss_message_clears_message_when_called() {
    let mut app = App::new("dev".to_string(), None);
    app.show_message(MessageLevel::Info, "Info", "test");
    app.dismiss_message();
    assert!(app.message.is_none());
}

// ──────────────────────────────────────────────
// handle_event テスト
// ──────────────────────────────────────────────

#[test]
fn handle_event_routes_instances_loaded_to_correct_tab() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    let instances = vec![
        create_test_instance("i-001", "web", InstanceState::Running),
        create_test_instance("i-002", "api", InstanceState::Stopped),
    ];
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::InstancesLoaded(Ok(instances)),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ec2 { instances } = &tab.data {
        assert_eq!(instances.all().len(), 2);
        assert_eq!(instances.filtered.len(), 2);
    } else {
        panic!("Expected Ec2 ServiceData");
    }
}

#[test]
fn handle_event_shows_error_when_instances_loaded_err() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::InstancesLoaded(Err(AppError::AwsApi("access denied".to_string()))),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

#[test]
fn handle_event_shows_info_when_instances_loaded_empty() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::InstancesLoaded(Ok(vec![])),
    ));
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No instances found");
}

#[test]
fn handle_event_crud_completed_shows_success_and_sets_loading() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    app.handle_event(AppEvent::CrudCompleted(
        tab_id,
        Ok("Bucket created".to_string()),
    ));
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Success);
    assert_eq!(msg.body, "Bucket created");
    assert!(app.active_tab().unwrap().loading);
}

#[test]
fn handle_event_crud_completed_shows_error_when_err() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    app.handle_event(AppEvent::CrudCompleted(
        tab_id,
        Err(AppError::AwsApi("access denied".to_string())),
    ));
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// StartStop / Reboot テスト
// ──────────────────────────────────────────────

#[test]
fn start_stop_sets_confirm_stop_when_instance_running() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.dispatch(Action::StartStop);
    assert_eq!(
        app.active_tab().unwrap().mode,
        Mode::Confirm(ConfirmAction::Stop("i-001".to_string()))
    );
}

#[test]
fn start_stop_sets_confirm_start_when_instance_stopped() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Stopped)],
    );
    app.dispatch(Action::StartStop);
    assert_eq!(
        app.active_tab().unwrap().mode,
        Mode::Confirm(ConfirmAction::Start("i-001".to_string()))
    );
}

#[test]
fn reboot_sets_confirm_reboot_when_instance_exists() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.dispatch(Action::Reboot);
    assert_eq!(
        app.active_tab().unwrap().mode,
        Mode::Confirm(ConfirmAction::Reboot("i-001".to_string()))
    );
}

// ──────────────────────────────────────────────
// ConfirmYes テスト
// ──────────────────────────────────────────────

#[test]
fn confirm_yes_returns_confirm_action_when_in_confirm_mode() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
    let result = app.dispatch(Action::ConfirmYes);
    assert_eq!(
        result,
        SideEffect::Confirm(ConfirmAction::Stop("i-001".to_string()))
    );
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

#[test]
fn confirm_no_sets_normal_mode_when_in_confirm() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Confirm(ConfirmAction::Stop("i-001".to_string()));
    let result = app.dispatch(Action::ConfirmNo);
    assert_eq!(result, SideEffect::None);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

// ──────────────────────────────────────────────
// Create / Delete / Edit テスト
// ──────────────────────────────────────────────

#[test]
fn handle_create_sets_form_mode_when_s3_list() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::S3);
    app.active_tab_mut().unwrap().loading = false;
    app.dispatch(Action::Create);
    assert!(matches!(
        app.active_tab().unwrap().mode,
        Mode::Form(FormContext {
            kind: FormKind::CreateS3Bucket,
            ..
        })
    ));
}

#[test]
fn handle_create_sets_form_mode_when_secrets_list() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::SecretsManager);
    app.active_tab_mut().unwrap().loading = false;
    app.dispatch(Action::Create);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.kind, FormKind::CreateSecret);
        assert_eq!(ctx.fields.len(), 3);
    } else {
        panic!("Expected Form mode");
    }
}

#[test]
fn handle_create_does_nothing_when_ec2_list() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::Create);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

#[test]
fn handle_delete_shows_permission_denied_when_no_permission() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.dispatch(Action::Delete);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.title, "Permission Denied");
}

#[test]
fn handle_delete_sets_danger_confirm_when_ec2_with_permission() {
    let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
    app.create_tab(ServiceKind::Ec2);
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    app.dispatch(Action::Delete);
    if let Mode::DangerConfirm(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.action, DangerAction::TerminateEc2("i-001".to_string()));
    } else {
        panic!("Expected DangerConfirm mode");
    }
}

#[test]
fn handle_edit_sets_form_mode_when_secrets_detail_with_detail() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::SecretsManager);
    let tab = app.active_tab_mut().unwrap();
    tab.tab_view = TabView::Detail;
    tab.loading = false;
    if let ServiceData::Secrets { detail, .. } = &mut tab.data {
        *detail = Some(Box::new(crate::aws::secrets_model::SecretDetail {
            name: "my-secret".to_string(),
            arn: "arn:test".to_string(),
            description: None,
            kms_key_id: None,
            rotation_enabled: false,
            rotation_lambda_arn: None,
            rotation_days: None,
            last_rotated_date: None,
            last_changed_date: None,
            last_accessed_date: None,
            created_date: None,
            tags: HashMap::new(),
            version_ids: Vec::new(),
            version_stages: Vec::new(),
            secret_value: None,
        }));
    }
    app.dispatch(Action::Edit);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.kind, FormKind::UpdateSecretValue);
        assert_eq!(ctx.fields.len(), 1);
    } else {
        panic!("Expected Form mode");
    }
}

// ──────────────────────────────────────────────
// FormSubmit テスト
// ──────────────────────────────────────────────

#[test]
fn form_submit_shows_error_when_required_field_empty() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::CreateS3Bucket,
        fields: vec![FormField {
            label: "Bucket Name".to_string(),
            input: Input::default(),
            required: true,
        }],
        focused_field: 0,
    });
    app.dispatch(Action::FormSubmit);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

#[test]
fn form_submit_returns_form_submit_side_effect_when_valid() {
    let mut app = app_with_ec2_tab();
    let mut input = Input::default();
    input.handle(tui_input::InputRequest::InsertChar('t'));
    input.handle(tui_input::InputRequest::InsertChar('e'));
    input.handle(tui_input::InputRequest::InsertChar('s'));
    input.handle(tui_input::InputRequest::InsertChar('t'));
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::CreateS3Bucket,
        fields: vec![FormField {
            label: "Bucket Name".to_string(),
            input,
            required: true,
        }],
        focused_field: 0,
    });
    let result = app.dispatch(Action::FormSubmit);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    assert!(matches!(result, SideEffect::FormSubmit(_)));
    assert!(app.active_tab().unwrap().loading);
}

// ──────────────────────────────────────────────
// DangerConfirm テスト
// ──────────────────────────────────────────────

#[test]
fn danger_confirm_submit_does_nothing_when_text_mismatch() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
        action: DangerAction::TerminateEc2("i-001".to_string()),
        input: Input::default(),
    });
    app.dispatch(Action::DangerConfirmSubmit);
    assert!(matches!(
        app.active_tab().unwrap().mode,
        Mode::DangerConfirm(_)
    ));
}

#[test]
fn danger_confirm_submit_returns_danger_action_when_text_matches() {
    let mut app = app_with_ec2_tab();
    let mut input = Input::default();
    for c in "i-001".chars() {
        input.handle(tui_input::InputRequest::InsertChar(c));
    }
    app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
        action: DangerAction::TerminateEc2("i-001".to_string()),
        input,
    });
    let result = app.dispatch(Action::DangerConfirmSubmit);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
    assert_eq!(
        result,
        SideEffect::DangerAction(DangerAction::TerminateEc2("i-001".to_string()))
    );
}

// ──────────────────────────────────────────────
// apply_filter テスト
// ──────────────────────────────────────────────

#[test]
fn apply_filter_filters_tab_data_when_on_tab() {
    let mut app = app_with_ec2_tab();
    let tab = app.active_tab_mut().unwrap();
    if let ServiceData::Ec2 { instances } = &mut tab.data {
        instances.set_items(vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ]);
    }
    tab.filter_input = Input::from("web");
    tab.loading = false;
    app.apply_filter();
    let tab = app.active_tab().unwrap();
    if let ServiceData::Ec2 { instances } = &tab.data {
        assert_eq!(instances.filtered.len(), 1);
        assert_eq!(instances.filtered[0].name, "web");
    } else {
        panic!("Expected Ec2 data");
    }
}

#[test]
fn apply_filter_filters_dashboard_services_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.filter_input = Input::from("EC2");
    app.apply_filter();
    assert!(!app.dashboard.filtered_services.is_empty());
    // EC2 should be in the results
    assert!(app.dashboard.filtered_services.contains(&ServiceKind::Ec2));
}

// ──────────────────────────────────────────────
// can_delete テスト
// ──────────────────────────────────────────────

#[test]
fn can_delete_returns_false_when_default_permissions() {
    let app = App::new("dev".to_string(), None);
    assert!(!app.can_delete("ec2"));
    assert!(!app.can_delete("s3"));
}

#[test]
fn can_delete_returns_true_when_all_permissions() {
    let app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
    assert!(app.can_delete("ec2"));
    assert!(app.can_delete("s3"));
}

#[test]
fn can_delete_returns_true_when_service_permitted() {
    let app = App::with_delete_permissions(
        "dev".to_string(),
        None,
        DeletePermissions::Services(vec!["ec2".to_string(), "s3".to_string()]),
    );
    assert!(app.can_delete("ec2"));
    assert!(app.can_delete("s3"));
    assert!(!app.can_delete("ecs"));
}

// ──────────────────────────────────────────────
// switch_tab_next / switch_tab_prev テスト
// ──────────────────────────────────────────────

#[test]
fn switch_tab_next_cycles_through_tabs() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ec2);
    app.create_tab(ServiceKind::S3);
    assert_eq!(app.active_tab_index, 1); // last created
    app.switch_tab_next();
    assert_eq!(app.active_tab_index, 0); // wraps around
    app.switch_tab_next();
    assert_eq!(app.active_tab_index, 1);
}

#[test]
fn switch_tab_prev_cycles_through_tabs() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ec2);
    app.create_tab(ServiceKind::S3);
    assert_eq!(app.active_tab_index, 1);
    app.switch_tab_prev();
    assert_eq!(app.active_tab_index, 0);
    app.switch_tab_prev();
    assert_eq!(app.active_tab_index, 1); // wraps around
}

// ──────────────────────────────────────────────
// close_tab テスト
// ──────────────────────────────────────────────

#[test]
fn close_tab_removes_tab_and_shows_dashboard_when_last() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ec2);
    assert!(!app.show_dashboard);
    app.close_tab();
    assert!(app.tabs.is_empty());
    assert!(app.show_dashboard);
}

#[test]
fn close_tab_adjusts_index_when_not_last() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ec2);
    app.create_tab(ServiceKind::S3);
    assert_eq!(app.active_tab_index, 1);
    app.close_tab();
    assert_eq!(app.tabs.len(), 1);
    assert_eq!(app.active_tab_index, 0);
    assert_eq!(app.active_tab().unwrap().service, ServiceKind::Ec2);
}

// ──────────────────────────────────────────────
// current_view テスト
// ──────────────────────────────────────────────

#[test]
fn current_view_returns_ec2_list_when_ec2_tab_in_list_view() {
    let app = app_with_ec2_tab();
    assert_eq!(app.current_view(), Some((ServiceKind::Ec2, TabView::List)));
}

#[test]
fn current_view_returns_ec2_detail_when_ec2_tab_in_detail_view() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().tab_view = TabView::Detail;
    assert_eq!(
        app.current_view(),
        Some((ServiceKind::Ec2, TabView::Detail))
    );
}

#[test]
fn current_view_returns_none_when_no_tabs() {
    let app = App::new("dev".to_string(), None);
    assert_eq!(app.current_view(), None);
}

// ──────────────────────────────────────────────
// SwitchDetailTab テスト
// ──────────────────────────────────────────────

#[test]
fn switch_detail_tab_toggles_ec2_detail_tab() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().tab_view = TabView::Detail;
    assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Overview);
    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Tags);
    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Overview);
}

// ──────────────────────────────────────────────
// ShowHelp テスト
// ──────────────────────────────────────────────

#[test]
fn show_help_sets_flag_and_back_dismisses() {
    let mut app = App::new("dev".to_string(), None);
    app.dispatch(Action::ShowHelp);
    assert!(app.show_help);
    app.dispatch(Action::Back);
    assert!(!app.show_help);
}

// ──────────────────────────────────────────────
// Message overlay テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_dismiss_message_clears_message_overlay() {
    let mut app = App::new("dev".to_string(), None);
    app.show_message(MessageLevel::Info, "Info", "test");
    app.dispatch(Action::DismissMessage);
    assert!(app.message.is_none());
}

#[test]
fn dispatch_back_clears_message_overlay() {
    let mut app = App::new("dev".to_string(), None);
    app.show_message(MessageLevel::Info, "Info", "test");
    app.dispatch(Action::Back);
    assert!(app.message.is_none());
}

// ──────────────────────────────────────────────
// half_page テスト
// ──────────────────────────────────────────────

#[test]
fn half_page_up_moves_10_when_on_tab_list() {
    let mut app = app_with_ec2_tab();
    let instances: Vec<Instance> = (0..20)
        .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
        .collect();
    set_ec2_instances(&mut app, instances);
    app.active_tab_mut().unwrap().selected_index = 15;
    app.dispatch(Action::HalfPageUp);
    assert_eq!(app.active_tab().unwrap().selected_index, 5);
}

#[test]
fn half_page_down_moves_10_when_on_tab_list() {
    let mut app = app_with_ec2_tab();
    let instances: Vec<Instance> = (0..20)
        .map(|i| create_test_instance(&format!("i-{i:03}"), "inst", InstanceState::Running))
        .collect();
    set_ec2_instances(&mut app, instances);
    app.active_tab_mut().unwrap().selected_index = 5;
    app.dispatch(Action::HalfPageDown);
    assert_eq!(app.active_tab().unwrap().selected_index, 15);
}

// ──────────────────────────────────────────────
// Noop テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_returns_side_effect_none_when_noop() {
    let mut app = App::new("dev".to_string(), None);
    let result = app.dispatch(Action::Noop);
    assert_eq!(result, SideEffect::None);
}

// ──────────────────────────────────────────────
// Filter mode テスト
// ──────────────────────────────────────────────

#[test]
fn start_filter_sets_filter_mode_on_tab() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::StartFilter);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Filter);
}

#[test]
fn confirm_filter_sets_normal_mode_on_tab() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Filter;
    app.dispatch(Action::ConfirmFilter);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

#[test]
fn cancel_filter_resets_filter_and_sets_normal_mode() {
    let mut app = app_with_ec2_tab();
    let tab = app.active_tab_mut().unwrap();
    tab.mode = Mode::Filter;
    tab.filter_input = Input::from("web");
    if let ServiceData::Ec2 { instances } = &mut tab.data {
        instances.set_items(vec![create_test_instance(
            "i-001",
            "web",
            InstanceState::Running,
        )]);
    }
    app.dispatch(Action::CancelFilter);
    let tab = app.active_tab().unwrap();
    assert_eq!(tab.mode, Mode::Normal);
    assert!(tab.filter_input.value().is_empty());
}

// ──────────────────────────────────────────────
// FormNextField テスト
// ──────────────────────────────────────────────

#[test]
fn form_next_field_advances_when_multiple_fields() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
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
        ],
        focused_field: 0,
    });
    app.dispatch(Action::FormNextField);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.focused_field, 1);
    } else {
        panic!("Expected Form mode");
    }
}

#[test]
fn form_next_field_wraps_around_when_at_last() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
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
        ],
        focused_field: 1,
    });
    app.dispatch(Action::FormNextField);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.focused_field, 0);
    } else {
        panic!("Expected Form mode");
    }
}

// ──────────────────────────────────────────────
// Refresh テスト
// ──────────────────────────────────────────────

#[test]
fn refresh_sets_loading_on_active_tab() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().loading = false;
    app.dispatch(Action::Refresh);
    assert!(app.active_tab().unwrap().loading);
}

// ──────────────────────────────────────────────
// DangerAction テスト
// ──────────────────────────────────────────────

#[test]
fn danger_action_confirm_text_returns_id_when_terminate_ec2() {
    let action = DangerAction::TerminateEc2("i-001".to_string());
    assert_eq!(action.confirm_text(), "i-001");
}

#[test]
fn danger_action_confirm_text_returns_name_when_delete_s3_bucket() {
    let action = DangerAction::DeleteS3Bucket("my-bucket".to_string());
    assert_eq!(action.confirm_text(), "my-bucket");
}

#[test]
fn danger_action_confirm_text_returns_key_when_delete_s3_object() {
    let action = DangerAction::DeleteS3Object {
        bucket: "my-bucket".to_string(),
        key: "path/to/file.txt".to_string(),
    };
    assert_eq!(action.confirm_text(), "path/to/file.txt");
}

#[test]
fn danger_action_message_returns_terminate_msg_when_ec2() {
    let action = DangerAction::TerminateEc2("i-001".to_string());
    assert_eq!(action.message(), "Type 'i-001' to terminate this instance:");
}

#[test]
fn danger_action_message_returns_delete_msg_when_s3_bucket() {
    let action = DangerAction::DeleteS3Bucket("my-bucket".to_string());
    assert_eq!(action.message(), "Type 'my-bucket' to delete this bucket:");
}

// ──────────────────────────────────────────────
// FormContext テスト
// ──────────────────────────────────────────────

#[test]
fn form_context_field_values_returns_label_value_pairs() {
    let mut input = Input::default();
    input.handle(tui_input::InputRequest::InsertChar('a'));
    let ctx = FormContext {
        kind: FormKind::CreateS3Bucket,
        fields: vec![FormField {
            label: "Name".to_string(),
            input,
            required: true,
        }],
        focused_field: 0,
    };
    let values = ctx.field_values();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], ("Name", "a"));
}

// ──────────────────────────────────────────────
// Dashboard filter mode テスト
// ──────────────────────────────────────────────

#[test]
fn start_filter_sets_dashboard_filter_mode_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dispatch(Action::StartFilter);
    assert_eq!(app.dashboard.mode, Mode::Filter);
}

#[test]
fn cancel_filter_resets_dashboard_filter_when_on_dashboard() {
    let mut app = App::new("dev".to_string(), None);
    app.dashboard.mode = Mode::Filter;
    app.dashboard.filter_input = Input::from("ec");
    app.dispatch(Action::CancelFilter);
    assert_eq!(app.dashboard.mode, Mode::Normal);
    assert!(app.dashboard.filter_input.value().is_empty());
}

// ──────────────────────────────────────────────
// FormCancel テスト
// ──────────────────────────────────────────────

#[test]
fn form_cancel_sets_normal_mode_when_in_form() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::CreateS3Bucket,
        fields: vec![FormField {
            label: "Bucket Name".to_string(),
            input: Input::default(),
            required: true,
        }],
        focused_field: 0,
    });
    app.dispatch(Action::FormCancel);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

// ──────────────────────────────────────────────
// DangerConfirmCancel テスト
// ──────────────────────────────────────────────

#[test]
fn danger_confirm_cancel_sets_normal_mode() {
    let mut app = app_with_ec2_tab();
    app.active_tab_mut().unwrap().mode = Mode::DangerConfirm(DangerConfirmContext {
        action: DangerAction::TerminateEc2("i-001".to_string()),
        input: Input::default(),
    });
    app.dispatch(Action::DangerConfirmCancel);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

// ──────────────────────────────────────────────
// ProfileSelector テスト
// ──────────────────────────────────────────────

fn test_profiles() -> Vec<SsoProfile> {
    vec![
        SsoProfile {
            name: "dev-account".to_string(),
            region: Some("ap-northeast-1".to_string()),
            sso_start_url: "https://dev.awsapps.com/start".to_string(),
            sso_session: None,
        },
        SsoProfile {
            name: "staging".to_string(),
            region: Some("us-east-1".to_string()),
            sso_start_url: "https://staging.awsapps.com/start".to_string(),
            sso_session: None,
        },
    ]
}

fn app_with_profile_selector() -> App {
    App::new_with_profile_selector(test_profiles(), DeletePermissions::None)
}

#[test]
fn new_with_profile_selector_initializes_with_profile_selector_when_profiles_given() {
    let app = app_with_profile_selector();

    assert!(app.profile.is_none());
    assert!(app.profile_selector.is_some());
    assert!(!app.show_dashboard);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.profiles.len(), 2);
    assert_eq!(ps.filtered_profiles.len(), 2);
    assert_eq!(ps.selected_index, 0);
    assert_eq!(ps.mode, Mode::Normal);
}

#[test]
fn complete_profile_selection_transitions_to_dashboard_when_profile_selected() {
    let mut app = app_with_profile_selector();

    app.complete_profile_selection(
        "dev-account".to_string(),
        Some("ap-northeast-1".to_string()),
    );

    assert_eq!(app.profile, Some("dev-account".to_string()));
    assert_eq!(app.region, Some("ap-northeast-1".to_string()));
    assert!(app.profile_selector.is_none());
    assert!(app.show_dashboard);
}

#[test]
fn dispatch_move_down_on_profile_selector_increments_index() {
    let mut app = app_with_profile_selector();
    app.dispatch(Action::MoveDown);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.selected_index, 1);
}

#[test]
fn dispatch_move_up_on_profile_selector_decrements_index() {
    let mut app = app_with_profile_selector();
    app.profile_selector.as_mut().unwrap().selected_index = 1;
    app.dispatch(Action::MoveUp);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.selected_index, 0);
}

#[test]
fn dispatch_quit_on_profile_selector_sets_should_quit() {
    let mut app = app_with_profile_selector();
    app.dispatch(Action::Quit);
    assert!(app.should_quit);
}

#[test]
fn dispatch_start_filter_on_profile_selector_sets_filter_mode() {
    let mut app = app_with_profile_selector();
    app.dispatch(Action::StartFilter);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.mode, Mode::Filter);
}

#[test]
fn dispatch_confirm_filter_on_profile_selector_sets_normal_mode() {
    let mut app = app_with_profile_selector();
    app.profile_selector.as_mut().unwrap().mode = Mode::Filter;
    app.dispatch(Action::ConfirmFilter);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.mode, Mode::Normal);
}

#[test]
fn dispatch_cancel_filter_on_profile_selector_resets_filter() {
    let mut app = app_with_profile_selector();
    let ps = app.profile_selector.as_mut().unwrap();
    ps.mode = Mode::Filter;
    ps.filter_input = "dev".into();
    ps.apply_filter();

    app.dispatch(Action::CancelFilter);

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.mode, Mode::Normal);
    assert!(ps.filter_input.value().is_empty());
    assert_eq!(ps.filtered_profiles.len(), 2);
}

#[test]
fn dispatch_filter_handle_input_on_profile_selector_updates_filter() {
    let mut app = app_with_profile_selector();
    app.profile_selector.as_mut().unwrap().mode = Mode::Filter;

    app.dispatch(Action::FilterHandleInput(
        tui_input::InputRequest::InsertChar('d'),
    ));
    app.dispatch(Action::FilterHandleInput(
        tui_input::InputRequest::InsertChar('e'),
    ));
    app.dispatch(Action::FilterHandleInput(
        tui_input::InputRequest::InsertChar('v'),
    ));

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.filter_input.value(), "dev");
    assert_eq!(ps.filtered_profiles.len(), 1);
    assert_eq!(ps.filtered_profiles[0].name, "dev-account");
}

#[test]
fn dispatch_move_to_top_on_profile_selector_sets_index_to_zero() {
    let mut app = app_with_profile_selector();
    app.profile_selector.as_mut().unwrap().selected_index = 1;
    app.dispatch(Action::MoveToTop);

    assert_eq!(app.profile_selector.as_ref().unwrap().selected_index, 0);
}

#[test]
fn dispatch_move_to_bottom_on_profile_selector_sets_index_to_last() {
    let mut app = app_with_profile_selector();
    app.dispatch(Action::MoveToBottom);

    assert_eq!(app.profile_selector.as_ref().unwrap().selected_index, 1);
}

#[test]
fn dispatch_cancel_sso_login_resets_logging_in_state() {
    let mut app = app_with_profile_selector();
    let ps = app.profile_selector.as_mut().unwrap();
    ps.logging_in = true;
    ps.login_output = vec!["line1".to_string()];

    app.dispatch(Action::CancelSsoLogin);

    let ps = app.profile_selector.as_ref().unwrap();
    assert!(!ps.logging_in);
    assert!(ps.login_output.is_empty());
}

#[test]
fn handle_event_sso_login_output_appends_line_to_login_output() {
    let mut app = app_with_profile_selector();
    app.profile_selector.as_mut().unwrap().logging_in = true;

    app.handle_event(AppEvent::SsoLoginOutput("Opening browser...".to_string()));
    app.handle_event(AppEvent::SsoLoginOutput(
        "Waiting for authorization...".to_string(),
    ));

    let ps = app.profile_selector.as_ref().unwrap();
    assert_eq!(ps.login_output.len(), 2);
    assert_eq!(ps.login_output[0], "Opening browser...");
    assert_eq!(ps.login_output[1], "Waiting for authorization...");
}

#[test]
fn handle_event_sso_login_completed_ok_transitions_to_dashboard() {
    let mut app = app_with_profile_selector();
    let ps = app.profile_selector.as_mut().unwrap();
    ps.logging_in = true;
    ps.login_output = vec!["line1".to_string()];

    app.handle_event(AppEvent::SsoLoginCompleted(Ok((
        "dev-account".to_string(),
        Some("ap-northeast-1".to_string()),
    ))));

    assert_eq!(app.profile, Some("dev-account".to_string()));
    assert_eq!(app.region, Some("ap-northeast-1".to_string()));
    assert!(app.profile_selector.is_none());
    assert!(app.show_dashboard);
}

#[test]
fn handle_event_sso_login_completed_err_resets_state_and_shows_error() {
    let mut app = app_with_profile_selector();
    let ps = app.profile_selector.as_mut().unwrap();
    ps.logging_in = true;
    ps.login_output = vec!["line1".to_string()];

    app.handle_event(AppEvent::SsoLoginCompleted(Err(AppError::AwsApi(
        "login failed".to_string(),
    ))));

    let ps = app.profile_selector.as_ref().unwrap();
    assert!(!ps.logging_in);
    assert!(ps.login_output.is_empty());
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.title, "SSO Login Failed");
    assert_eq!(msg.body, "AWS API error: login failed");
}

// ──────────────────────────────────────────────
// SsmConnect テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_ssm_connect_returns_ssm_connect_when_instance_running() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Running)],
    );
    let side_effect = app.dispatch(Action::SsmConnect);
    assert_eq!(
        side_effect,
        SideEffect::SsmConnect {
            instance_id: "i-001".to_string(),
        }
    );
}

#[test]
fn dispatch_ssm_connect_returns_none_and_shows_error_when_instance_stopped() {
    let mut app = app_with_ec2_tab();
    set_ec2_instances(
        &mut app,
        vec![create_test_instance("i-001", "web", InstanceState::Stopped)],
    );
    let side_effect = app.dispatch(Action::SsmConnect);
    assert_eq!(side_effect, SideEffect::None);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

#[test]
fn dispatch_ssm_connect_returns_none_when_no_instance_selected() {
    let mut app = app_with_ec2_tab();
    let side_effect = app.dispatch(Action::SsmConnect);
    assert_eq!(side_effect, SideEffect::None);
}

// ──────────────────────────────────────────────
// ProfileSelector テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_enter_on_profile_selector_returns_start_sso_login_when_token_not_found() {
    let mut app = app_with_profile_selector();

    // テスト環境ではSSOトークンキャッシュが存在しないのでNotFoundになる
    let side_effect = app.dispatch(Action::Enter);

    assert_eq!(
        side_effect,
        SideEffect::StartSsoLogin {
            profile_name: "dev-account".to_string(),
            region: Some("ap-northeast-1".to_string()),
        }
    );
    let ps = app.profile_selector.as_ref().unwrap();
    assert!(ps.logging_in);
    assert!(ps.login_output.is_empty());
}
