use super::*;
use crate::action::Action;
use crate::aws::ecr_model::{Image, Repository};
use crate::aws::ecs_model::{Cluster, ContainerLogConfig};
use crate::aws::logs_model::LogEvent;
use crate::aws::model::{InstanceState, SecurityGroup, SecurityGroupRule};
use crate::aws::s3_model::{Bucket, S3Object};
use crate::aws::secrets_model::{Secret, SecretDetail};
use crate::aws::vpc_model::{Subnet, Vpc};
use crate::cli::DeletePermissions;
use crate::config::SsoProfile;
use crate::error::AppError;
use crate::event::TabEvent;
use crate::tab::{EcsNavLevel, LogViewState, ServiceData, TabId, TabView};
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
    if let ServiceData::Ec2 { instances, .. } = &mut tab.data {
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
    if let ServiceData::Ec2 { instances, .. } = &tab.data {
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
fn handle_create_sets_form_mode_when_ecr_list() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecr);
    app.active_tab_mut().unwrap().loading = false;
    app.dispatch(Action::Create);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.kind, FormKind::CreateEcrRepository);
        assert_eq!(ctx.fields.len(), 2);
        assert_eq!(ctx.fields[0].label, "Repository Name");
        assert_eq!(ctx.fields[1].input.value(), "MUTABLE");
    } else {
        panic!("Expected Form mode");
    }
}

#[test]
fn handle_delete_sets_danger_confirm_when_ecr_with_permission() {
    let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
    app.create_tab(ServiceKind::Ecr);
    if let Some(tab) = app.active_tab_mut() {
        if let ServiceData::Ecr { repositories, .. } = &mut tab.data {
            repositories.set_items(vec![crate::aws::ecr_model::Repository {
                repository_name: "myapp/web".to_string(),
                repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web"
                    .to_string(),
                registry_id: "123456789012".to_string(),
                created_at: None,
                image_tag_mutability: "MUTABLE".to_string(),
            }]);
        }
    }
    app.dispatch(Action::Delete);
    if let Mode::DangerConfirm(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(
            ctx.action,
            DangerAction::DeleteEcrRepository("myapp/web".to_string())
        );
    } else {
        panic!("Expected DangerConfirm mode");
    }
}

#[test]
fn handle_delete_shows_permission_denied_when_ecr_no_permission() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecr);
    if let Some(tab) = app.active_tab_mut() {
        if let ServiceData::Ecr { repositories, .. } = &mut tab.data {
            repositories.set_items(vec![crate::aws::ecr_model::Repository {
                repository_name: "myapp/web".to_string(),
                repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web"
                    .to_string(),
                registry_id: "123456789012".to_string(),
                created_at: None,
                image_tag_mutability: "MUTABLE".to_string(),
            }]);
        }
    }
    app.dispatch(Action::Delete);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.title, "Permission Denied");
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

#[test]
fn form_submit_shows_error_when_scale_ecs_service_with_non_numeric() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    let mut input = Input::default();
    for c in "abc".chars() {
        input.handle(tui_input::InputRequest::InsertChar(c));
    }
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::ScaleEcsService,
        fields: vec![FormField {
            label: "Desired Count".to_string(),
            input,
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
fn form_submit_shows_error_when_scale_ecs_service_with_negative() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    let mut input = Input::default();
    for c in "-1".chars() {
        input.handle(tui_input::InputRequest::InsertChar(c));
    }
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::ScaleEcsService,
        fields: vec![FormField {
            label: "Desired Count".to_string(),
            input,
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
fn form_submit_returns_form_submit_side_effect_when_scale_ecs_service() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    let mut input = Input::default();
    input.handle(tui_input::InputRequest::InsertChar('3'));
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::ScaleEcsService,
        fields: vec![FormField {
            label: "Desired Count".to_string(),
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

#[test]
fn form_submit_returns_form_submit_side_effect_when_scale_ecs_service_with_zero() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    let mut input = Input::default();
    input.handle(tui_input::InputRequest::InsertChar('0'));
    app.active_tab_mut().unwrap().mode = Mode::Form(FormContext {
        kind: FormKind::ScaleEcsService,
        fields: vec![FormField {
            label: "Desired Count".to_string(),
            input,
            required: true,
        }],
        focused_field: 0,
    });
    let result = app.dispatch(Action::FormSubmit);
    assert!(matches!(result, SideEffect::FormSubmit(_)));
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
    if let ServiceData::Ec2 { instances, .. } = &mut tab.data {
        instances.set_items(vec![
            create_test_instance("i-001", "web", InstanceState::Running),
            create_test_instance("i-002", "api", InstanceState::Stopped),
        ]);
    }
    tab.filter_input = Input::from("web");
    tab.loading = false;
    app.apply_filter();
    let tab = app.active_tab().unwrap();
    if let ServiceData::Ec2 { instances, .. } = &tab.data {
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
    assert_eq!(
        app.active_tab().unwrap().detail_tab,
        DetailTab::SecurityGroups
    );
    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Metrics);
    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(app.active_tab().unwrap().detail_tab, DetailTab::Overview);
}

// ──────────────────────────────────────────────
// ECS ServiceDetail タブ切り替えテスト
// ──────────────────────────────────────────────

fn ecs_service_detail_tab(app: &App) -> crate::tui::views::ecs_service_detail::EcsServiceDetailTab {
    let ServiceData::Ecs { nav_level, .. } = &app.active_tab().unwrap().data else {
        panic!("Expected ECS service data");
    };
    let Some(crate::tab::EcsNavLevel::ServiceDetail { detail_tab, .. }) = nav_level else {
        panic!("Expected ServiceDetail nav level");
    };
    detail_tab.clone()
}

#[test]
fn switch_detail_tab_toggles_ecs_service_detail_tab() {
    use crate::tui::views::ecs_service_detail::EcsServiceDetailTab;

    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);

    assert_eq!(ecs_service_detail_tab(&app), EcsServiceDetailTab::Tasks);

    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(
        ecs_service_detail_tab(&app),
        EcsServiceDetailTab::Deployments
    );

    app.dispatch(Action::SwitchDetailTab);
    assert_eq!(ecs_service_detail_tab(&app), EcsServiceDetailTab::Tasks);
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
    if let ServiceData::Ec2 { instances, .. } = &mut tab.data {
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
// ECS Exec テスト
// ──────────────────────────────────────────────

fn create_test_container(name: &str, status: &str) -> crate::aws::ecs_model::Container {
    crate::aws::ecs_model::Container {
        name: name.to_string(),
        image: format!("123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/{name}:latest"),
        last_status: status.to_string(),
        exit_code: None,
        health_status: None,
        reason: None,
    }
}

fn create_test_task(
    task_id: &str,
    cluster_arn: &str,
    containers: Vec<crate::aws::ecs_model::Container>,
) -> crate::aws::ecs_model::Task {
    crate::aws::ecs_model::Task {
        task_arn: format!("arn:aws:ecs:ap-northeast-1:123456789012:task/production/{task_id}"),
        cluster_arn: cluster_arn.to_string(),
        task_definition_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/web:10"
            .to_string(),
        last_status: "RUNNING".to_string(),
        desired_status: "RUNNING".to_string(),
        cpu: Some("256".to_string()),
        memory: Some("512".to_string()),
        launch_type: Some("FARGATE".to_string()),
        platform_version: Some("1.4.0".to_string()),
        health_status: None,
        connectivity: None,
        availability_zone: None,
        started_at: None,
        stopped_at: None,
        stopped_reason: None,
        containers,
    }
}

fn create_test_ecs_service(enable_exec: bool) -> crate::aws::ecs_model::Service {
    crate::aws::ecs_model::Service {
        service_name: "web-service".to_string(),
        service_arn: "arn:aws:ecs:ap-northeast-1:123456789012:service/production/web-service"
            .to_string(),
        cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
        status: "ACTIVE".to_string(),
        desired_count: 1,
        running_count: 1,
        pending_count: 0,
        task_definition: "arn:aws:ecs:ap-northeast-1:123456789012:task-definition/web:10"
            .to_string(),
        launch_type: Some("FARGATE".to_string()),
        scheduling_strategy: Some("REPLICA".to_string()),
        created_at: None,
        health_check_grace_period_seconds: None,
        deployment_status: None,
        enable_execute_command: enable_exec,
        deployments: vec![],
    }
}

/// ECSタブを作成し、nav_levelをTaskDetailに設定したAppを返す
fn app_with_ecs_task_detail(
    service: crate::aws::ecs_model::Service,
    tasks: Vec<crate::aws::ecs_model::Task>,
) -> App {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecs);
    let tab = app.active_tab_mut().unwrap();
    if let ServiceData::Ecs {
        services: svcs,
        tasks: t,
        nav_level,
        ..
    } = &mut tab.data
    {
        *svcs = vec![service];
        *t = tasks;
        *nav_level = Some(crate::tab::EcsNavLevel::TaskDetail {
            service_index: 0,
            task_index: 0,
        });
    }
    tab.loading = false;
    app
}

#[test]
fn dispatch_ecs_exec_returns_none_when_not_task_detail() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecs);
    // nav_level is None (ClusterList)
    let side_effect = app.dispatch(Action::EcsExec);
    assert_eq!(side_effect, SideEffect::None);
}

#[test]
fn dispatch_ecs_exec_shows_error_when_exec_command_disabled() {
    let service = create_test_ecs_service(false);
    let tasks = vec![create_test_task(
        "abc123",
        &service.cluster_arn,
        vec![create_test_container("web", "RUNNING")],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(side_effect, SideEffect::None);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

#[test]
fn dispatch_ecs_exec_shows_error_when_no_running_containers() {
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        &service.cluster_arn,
        vec![create_test_container("web", "STOPPED")],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(side_effect, SideEffect::None);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

#[test]
fn dispatch_ecs_exec_returns_ecs_exec_when_single_running_container() {
    let cluster_arn = "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production";
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        cluster_arn,
        vec![create_test_container("web", "RUNNING")],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(
        side_effect,
        SideEffect::EcsExec {
            cluster_arn: cluster_arn.to_string(),
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/abc123".to_string(),
            container_name: "web".to_string(),
        }
    );
}

#[test]
fn dispatch_ecs_exec_shows_container_select_when_multiple_running_containers() {
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        &service.cluster_arn,
        vec![
            create_test_container("web", "RUNNING"),
            create_test_container("sidecar", "RUNNING"),
            create_test_container("stopped-one", "STOPPED"),
        ],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(side_effect, SideEffect::None);
    let tab = app.active_tab().unwrap();
    assert_eq!(
        tab.mode,
        Mode::ContainerSelect(ContainerSelectState::new(
            vec!["web".to_string(), "sidecar".to_string()],
            ContainerSelectPurpose::EcsExec,
        ))
    );
}

#[test]
fn dispatch_container_select_confirm_returns_ecs_exec_when_purpose_is_ecs_exec() {
    let cluster_arn = "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production";
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        cluster_arn,
        vec![
            create_test_container("web", "RUNNING"),
            create_test_container("sidecar", "RUNNING"),
        ],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);

    // ContainerSelectモードを設定（sidecarを選択）
    let tab = app.active_tab_mut().unwrap();
    let mut cs_state = ContainerSelectState::new(
        vec!["web".to_string(), "sidecar".to_string()],
        ContainerSelectPurpose::EcsExec,
    );
    cs_state.selected_index = 1;
    tab.mode = Mode::ContainerSelect(cs_state);

    let side_effect = app.dispatch(Action::ContainerSelectConfirm);

    assert_eq!(
        side_effect,
        SideEffect::EcsExec {
            cluster_arn: cluster_arn.to_string(),
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/abc123".to_string(),
            container_name: "sidecar".to_string(),
        }
    );
    // モードはNormalに戻る
    let tab = app.active_tab().unwrap();
    assert_eq!(tab.mode, Mode::Normal);
}

fn app_with_ecs_service_detail(
    service: crate::aws::ecs_model::Service,
    tasks: Vec<crate::aws::ecs_model::Task>,
) -> App {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecs);
    let tab = app.active_tab_mut().unwrap();
    if let ServiceData::Ecs {
        services: svcs,
        tasks: t,
        nav_level,
        ..
    } = &mut tab.data
    {
        *svcs = vec![service];
        *t = tasks;
        *nav_level = Some(crate::tab::EcsNavLevel::ServiceDetail {
            service_index: 0,
            detail_tab: crate::tui::views::ecs_service_detail::EcsServiceDetailTab::Tasks,
        });
    }
    tab.loading = false;
    app
}

#[test]
fn dispatch_ecs_exec_returns_ecs_exec_when_service_detail_single_container() {
    let cluster_arn = "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production";
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        cluster_arn,
        vec![create_test_container("web", "RUNNING")],
    )];
    let mut app = app_with_ecs_service_detail(service, tasks);
    // detail_tag_index = 0 (最初のタスクを選択中)

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(
        side_effect,
        SideEffect::EcsExec {
            cluster_arn: cluster_arn.to_string(),
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/abc123".to_string(),
            container_name: "web".to_string(),
        }
    );
}

#[test]
fn dispatch_ecs_exec_shows_container_select_when_service_detail_multiple_containers() {
    let cluster_arn = "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production";
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        cluster_arn,
        vec![
            create_test_container("web", "RUNNING"),
            create_test_container("sidecar", "RUNNING"),
        ],
    )];
    let mut app = app_with_ecs_service_detail(service, tasks);

    let side_effect = app.dispatch(Action::EcsExec);

    assert_eq!(side_effect, SideEffect::None);
    let tab = app.active_tab().unwrap();
    assert!(matches!(
        &tab.mode,
        Mode::ContainerSelect(state) if state.all_names == vec!["web".to_string(), "sidecar".to_string()]
            && state.selected_index == 0
            && state.purpose == ContainerSelectPurpose::EcsExec
    ));
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

// ──────────────────────────────────────────────
// handle_tab_event テストヘルパー
// ──────────────────────────────────────────────

fn create_test_repository(name: &str) -> Repository {
    Repository {
        repository_name: name.to_string(),
        repository_uri: format!("123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/{name}"),
        registry_id: "123456789012".to_string(),
        created_at: None,
        image_tag_mutability: "MUTABLE".to_string(),
    }
}

fn create_test_image(digest: &str) -> Image {
    Image {
        image_digest: digest.to_string(),
        image_tags: vec!["latest".to_string()],
        pushed_at: None,
        image_size_bytes: Some(1024),
    }
}

fn create_test_cluster(name: &str) -> Cluster {
    Cluster {
        cluster_name: name.to_string(),
        cluster_arn: format!("arn:aws:ecs:ap-northeast-1:123456789012:cluster/{name}"),
        status: "ACTIVE".to_string(),
        running_tasks_count: 1,
        pending_tasks_count: 0,
        active_services_count: 1,
        registered_container_instances_count: 0,
    }
}

fn create_test_bucket(name: &str) -> Bucket {
    Bucket {
        name: name.to_string(),
        creation_date: None,
    }
}

fn create_test_s3_object(key: &str) -> S3Object {
    S3Object {
        key: key.to_string(),
        size: Some(1024),
        last_modified: None,
        storage_class: Some("STANDARD".to_string()),
        is_prefix: false,
    }
}

fn create_test_vpc(id: &str) -> Vpc {
    Vpc {
        vpc_id: id.to_string(),
        name: "test-vpc".to_string(),
        cidr_block: "10.0.0.0/16".to_string(),
        state: "available".to_string(),
        is_default: false,
        owner_id: "123456789012".to_string(),
        tags: HashMap::new(),
    }
}

fn create_test_subnet(id: &str, vpc_id: &str) -> Subnet {
    Subnet {
        subnet_id: id.to_string(),
        name: "test-subnet".to_string(),
        vpc_id: vpc_id.to_string(),
        cidr_block: "10.0.1.0/24".to_string(),
        availability_zone: "ap-northeast-1a".to_string(),
        available_ip_count: 250,
        state: "available".to_string(),
        is_default: false,
        map_public_ip_on_launch: false,
    }
}

fn create_test_secret(name: &str) -> Secret {
    Secret {
        name: name.to_string(),
        arn: format!("arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:{name}-AbCdEf"),
        description: None,
        last_changed_date: None,
        last_accessed_date: None,
        tags: HashMap::new(),
    }
}

fn create_test_secret_detail(name: &str) -> SecretDetail {
    SecretDetail {
        name: name.to_string(),
        arn: format!("arn:aws:secretsmanager:ap-northeast-1:123456789012:secret:{name}-AbCdEf"),
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
    }
}

fn app_with_tab(service: ServiceKind) -> App {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(service);
    app
}

fn active_tab_id(app: &App) -> crate::tab::TabId {
    app.active_tab().unwrap().id
}

// ──────────────────────────────────────────────
// handle_tab_event: InstancesLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_instances_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ec2);
    let tab_id = active_tab_id(&app);
    let instances = vec![
        create_test_instance("i-001", "web-1", InstanceState::Running),
        create_test_instance("i-002", "web-2", InstanceState::Stopped),
    ];

    app.handle_tab_event(tab_id, TabEvent::InstancesLoaded(Ok(instances)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ec2 { instances, .. } = &tab.data {
        assert_eq!(instances.len(), 2);
        assert_eq!(instances.filtered[0].instance_id, "i-001");
    } else {
        panic!("Expected Ec2 ServiceData");
    }
    assert!(app.message.is_none());
}

#[test]
fn handle_tab_event_instances_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ec2);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::InstancesLoaded(Ok(vec![])));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No instances found");
}

#[test]
fn handle_tab_event_instances_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ec2);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::InstancesLoaded(Err(AppError::AwsApi("API error".to_string()))),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: RepositoriesLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_repositories_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);
    let repos = vec![create_test_repository("my-app")];

    app.handle_tab_event(tab_id, TabEvent::RepositoriesLoaded(Ok(repos)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr { repositories, .. } = &tab.data {
        assert_eq!(repositories.len(), 1);
        assert_eq!(repositories.filtered[0].repository_name, "my-app");
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_repositories_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::RepositoriesLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No repositories found");
}

#[test]
fn handle_tab_event_repositories_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::RepositoriesLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: ImagesLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_images_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);
    let images = vec![create_test_image("sha256:abc123")];

    app.handle_tab_event(tab_id, TabEvent::ImagesLoaded(Ok(images)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr { images, .. } = &tab.data {
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].image_digest, "sha256:abc123");
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_images_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::ImagesLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No images found");
}

#[test]
fn handle_tab_event_images_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ecr);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::ImagesLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: ClustersLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_clusters_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);
    let clusters = vec![create_test_cluster("production")];

    app.handle_tab_event(tab_id, TabEvent::ClustersLoaded(Ok(clusters)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecs { clusters, .. } = &tab.data {
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters.filtered[0].cluster_name, "production");
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_clusters_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::ClustersLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No clusters found");
}

#[test]
fn handle_tab_event_clusters_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::ClustersLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: EcsServicesLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_ecs_services_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);
    let services = vec![create_test_ecs_service(true)];

    app.handle_tab_event(tab_id, TabEvent::EcsServicesLoaded(Ok(services)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecs { services, .. } = &tab.data {
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].service_name, "web-service");
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_ecs_services_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::EcsServicesLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No services found");
}

#[test]
fn handle_tab_event_ecs_services_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::EcsServicesLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: EcsTasksLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_ecs_tasks_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);
    let tasks = vec![create_test_task(
        "abc123",
        "arn:aws:ecs:ap-northeast-1:123456789012:cluster/prod",
        vec![create_test_container("web", "RUNNING")],
    )];

    app.handle_tab_event(tab_id, TabEvent::EcsTasksLoaded(Ok(tasks)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecs { tasks, .. } = &tab.data {
        assert_eq!(tasks.len(), 1);
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_ecs_tasks_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::EcsTasksLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No tasks found");
}

#[test]
fn handle_tab_event_ecs_tasks_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ecs);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::EcsTasksLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: BucketsLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_buckets_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::S3);
    let tab_id = active_tab_id(&app);
    let buckets = vec![create_test_bucket("my-bucket")];

    app.handle_tab_event(tab_id, TabEvent::BucketsLoaded(Ok(buckets)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::S3 { buckets, .. } = &tab.data {
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets.filtered[0].name, "my-bucket");
    } else {
        panic!("Expected S3 ServiceData");
    }
}

#[test]
fn handle_tab_event_buckets_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::S3);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::BucketsLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No buckets found");
}

#[test]
fn handle_tab_event_buckets_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::S3);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::BucketsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: ObjectsLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_objects_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::S3);
    let tab_id = active_tab_id(&app);
    let objects = vec![create_test_s3_object("file.txt")];

    app.handle_tab_event(tab_id, TabEvent::ObjectsLoaded(Ok(objects)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::S3 { objects, .. } = &tab.data {
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, "file.txt");
    } else {
        panic!("Expected S3 ServiceData");
    }
}

#[test]
fn handle_tab_event_objects_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::S3);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::ObjectsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: VpcsLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_vpcs_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);
    let vpcs = vec![create_test_vpc("vpc-001")];

    app.handle_tab_event(tab_id, TabEvent::VpcsLoaded(Ok(vpcs)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Vpc { vpcs, .. } = &tab.data {
        assert_eq!(vpcs.len(), 1);
        assert_eq!(vpcs.filtered[0].vpc_id, "vpc-001");
    } else {
        panic!("Expected Vpc ServiceData");
    }
}

#[test]
fn handle_tab_event_vpcs_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::VpcsLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No VPCs found");
}

#[test]
fn handle_tab_event_vpcs_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::VpcsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: SubnetsLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_subnets_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);
    let subnets = vec![create_test_subnet("subnet-001", "vpc-001")];

    app.handle_tab_event(tab_id, TabEvent::SubnetsLoaded(Ok(subnets)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Vpc { subnets, .. } = &tab.data {
        assert_eq!(subnets.len(), 1);
        assert_eq!(subnets[0].subnet_id, "subnet-001");
    } else {
        panic!("Expected Vpc ServiceData");
    }
}

#[test]
fn handle_tab_event_subnets_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::SubnetsLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No subnets found");
}

#[test]
fn handle_tab_event_subnets_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::SubnetsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: SecretsLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_secrets_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);
    let secrets = vec![create_test_secret("my-secret")];

    app.handle_tab_event(tab_id, TabEvent::SecretsLoaded(Ok(secrets)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Secrets { secrets, .. } = &tab.data {
        assert_eq!(secrets.len(), 1);
        assert_eq!(secrets.filtered[0].name, "my-secret");
    } else {
        panic!("Expected Secrets ServiceData");
    }
}

#[test]
fn handle_tab_event_secrets_loaded_shows_info_when_empty() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::SecretsLoaded(Ok(vec![])));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Info);
    assert_eq!(msg.body, "No secrets found");
}

#[test]
fn handle_tab_event_secrets_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::SecretsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: SecretDetailLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_secret_detail_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);
    let detail = Box::new(create_test_secret_detail("my-secret"));

    app.handle_tab_event(tab_id, TabEvent::SecretDetailLoaded(Ok(detail)));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Secrets { detail, .. } = &tab.data {
        assert!(detail.is_some());
        assert_eq!(detail.as_ref().unwrap().name, "my-secret");
    } else {
        panic!("Expected Secrets ServiceData");
    }
}

#[test]
fn handle_tab_event_secret_detail_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::SecretDetailLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: SecretValueLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_secret_value_loaded_sets_value_and_visible_when_ok() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);
    // まず detail を設定しておく
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Secrets { detail, .. } = &mut tab.data {
            *detail = Some(Box::new(create_test_secret_detail("my-secret")));
        }
    }

    app.handle_tab_event(
        tab_id,
        TabEvent::SecretValueLoaded(Ok("supersecret".to_string())),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Secrets {
        detail,
        value_visible,
        ..
    } = &tab.data
    {
        assert!(*value_visible);
        assert_eq!(
            detail.as_ref().unwrap().secret_value,
            Some("supersecret".to_string())
        );
    } else {
        panic!("Expected Secrets ServiceData");
    }
}

#[test]
fn handle_tab_event_secret_value_loaded_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::SecretsManager);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::SecretValueLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: ActionCompleted テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_action_completed_shows_success_and_sets_loading_when_ok() {
    let mut app = app_with_tab(ServiceKind::Ec2);
    let tab_id = active_tab_id(&app);
    {
        let tab = app.active_tab_mut().unwrap();
        tab.loading = false;
    }

    app.handle_tab_event(
        tab_id,
        TabEvent::ActionCompleted(Ok("Instance started".to_string())),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Success);
    assert_eq!(msg.body, "Instance started");
    // ActionCompleted成功時はリロードのためloading=trueになる
    let tab = app.active_tab().unwrap();
    assert!(tab.loading);
}

#[test]
fn handle_tab_event_action_completed_shows_error_when_err() {
    let mut app = app_with_tab(ServiceKind::Ec2);
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::ActionCompleted(Err(AppError::AwsApi("action failed".to_string()))),
    );

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: NavigateVpcLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_navigate_vpc_loaded_sets_data_when_ok() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);
    let vpcs = vec![create_test_vpc("vpc-target")];
    let subnets = vec![create_test_subnet("subnet-001", "vpc-target")];

    app.handle_tab_event(tab_id, TabEvent::NavigateVpcLoaded(Ok((vpcs, subnets))));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Vpc { vpcs, subnets } = &tab.data {
        assert_eq!(vpcs.filtered[0].vpc_id, "vpc-target");
        assert_eq!(subnets.len(), 1);
    } else {
        panic!("Expected Vpc ServiceData");
    }
}

#[test]
fn handle_tab_event_navigate_vpc_loaded_rolls_back_stack_when_err() {
    let mut app = app_with_tab(ServiceKind::Vpc);
    let tab_id = active_tab_id(&app);
    // ナビゲーションスタックにエントリを積む
    {
        let tab = app.active_tab_mut().unwrap();
        tab.selected_index = 5; // ナビゲーション前と異なる値
        tab.navigation_stack.push(NavigationEntry {
            view: (ServiceKind::Vpc, TabView::Detail),
            selected_index: 2,
            detail_tag_index: 3,
            detail_tab: DetailTab::Overview,
            label: "vpc-001".to_string(),
        });
    }

    app.handle_tab_event(
        tab_id,
        TabEvent::NavigateVpcLoaded(Err(AppError::AwsApi("VPC not found".to_string()))),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    // スタックが巻き戻されて元のインデックスが復元される
    assert_eq!(tab.selected_index, 2);
    assert_eq!(tab.detail_tag_index, 3);
    assert!(tab.navigation_stack.is_empty());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: EcsLogConfigsLoaded テスト
// ──────────────────────────────────────────────

fn app_with_ecs_task_detail_for_logs() -> App {
    let cluster_arn = "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production";
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        cluster_arn,
        vec![create_test_container("web", "RUNNING")],
    )];
    app_with_ecs_task_detail(service, tasks)
}

#[test]
fn handle_tab_event_ecs_log_configs_loaded_shows_error_when_empty_configs() {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(tab_id, TabEvent::EcsLogConfigsLoaded(Ok(vec![])));

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.body, "No awslogs configuration found");
}

#[test]
fn handle_tab_event_ecs_log_configs_loaded_enters_log_view_when_single_config() {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab_id = active_tab_id(&app);
    let configs = vec![ContainerLogConfig {
        container_name: "web".to_string(),
        log_group: Some("/ecs/production/web".to_string()),
        stream_prefix: Some("ecs".to_string()),
        region: Some("ap-northeast-1".to_string()),
    }];

    app.handle_tab_event(tab_id, TabEvent::EcsLogConfigsLoaded(Ok(configs)));

    let tab = app.active_tab().unwrap();
    // ログ取得中なので loading=true
    assert!(tab.loading);
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        match nav_level.as_ref().unwrap() {
            EcsNavLevel::LogView {
                log_state,
                service_index,
                task_index,
            } => {
                assert_eq!(*service_index, 0);
                assert_eq!(*task_index, 0);
                assert_eq!(log_state.container_name, "web");
                assert_eq!(log_state.log_group, "/ecs/production/web");
                assert_eq!(log_state.log_stream, "ecs/web/abc123");
                assert!(log_state.auto_scroll);
                assert!(log_state.events.is_empty());
            }
            other => panic!("Expected LogView, got {:?}", other),
        }
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_ecs_log_configs_loaded_shows_error_when_no_log_group() {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab_id = active_tab_id(&app);
    let configs = vec![ContainerLogConfig {
        container_name: "web".to_string(),
        log_group: None, // ロググループなし
        stream_prefix: None,
        region: None,
    }];

    app.handle_tab_event(tab_id, TabEvent::EcsLogConfigsLoaded(Ok(configs)));

    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.body, "No log group configured");
}

#[test]
fn handle_tab_event_ecs_log_configs_loaded_shows_container_select_when_multiple_configs() {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab_id = active_tab_id(&app);
    let configs = vec![
        ContainerLogConfig {
            container_name: "web".to_string(),
            log_group: Some("/ecs/web".to_string()),
            stream_prefix: Some("ecs".to_string()),
            region: None,
        },
        ContainerLogConfig {
            container_name: "sidecar".to_string(),
            log_group: Some("/ecs/sidecar".to_string()),
            stream_prefix: Some("ecs".to_string()),
            region: None,
        },
    ];

    app.handle_tab_event(tab_id, TabEvent::EcsLogConfigsLoaded(Ok(configs)));

    let tab = app.active_tab().unwrap();
    assert_eq!(
        tab.mode,
        Mode::ContainerSelect(ContainerSelectState::new(
            vec!["web".to_string(), "sidecar".to_string()],
            ContainerSelectPurpose::ShowLogs,
        ))
    );
    // pending_log_configs に保存される
    assert!(app.pending_log_configs.is_some());
    let (stored_tab_id, stored_configs) = app.pending_log_configs.as_ref().unwrap();
    assert_eq!(*stored_tab_id, tab_id);
    assert_eq!(stored_configs.len(), 2);
}

#[test]
fn handle_tab_event_ecs_log_configs_loaded_shows_error_when_err() {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::EcsLogConfigsLoaded(Err(AppError::AwsApi("fail".to_string()))),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: EcsLogEventsLoaded テスト
// ──────────────────────────────────────────────

fn app_with_ecs_log_view() -> App {
    let mut app = app_with_ecs_task_detail_for_logs();
    let tab = app.active_tab_mut().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &mut tab.data {
        *nav_level = Some(EcsNavLevel::LogView {
            service_index: 0,
            task_index: 0,
            log_state: Box::new(LogViewState {
                container_name: "web".to_string(),
                log_group: "/ecs/web".to_string(),
                log_stream: "ecs/web/abc123".to_string(),
                events: Vec::new(),
                next_forward_token: None,
                auto_scroll: true,
                scroll_offset: 0,
                scroll_x: 0,
                search_query: String::new(),
                search_matches: Vec::new(),
                current_match_index: None,
            }),
        });
    }
    app
}

fn create_test_log_event(msg: &str) -> LogEvent {
    LogEvent {
        timestamp: 1700000000000,
        formatted_time: "2023-11-14T22:13:20Z".to_string(),
        message: msg.to_string(),
    }
}

#[test]
fn handle_tab_event_ecs_log_events_loaded_appends_events_when_ok() {
    let mut app = app_with_ecs_log_view();
    let tab_id = active_tab_id(&app);
    let events = vec![
        create_test_log_event("log line 1"),
        create_test_log_event("log line 2"),
    ];

    app.handle_tab_event(
        tab_id,
        TabEvent::EcsLogEventsLoaded(Ok((events, Some("next-token".to_string())))),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.events.len(), 2);
        assert_eq!(state.next_forward_token, Some("next-token".to_string()));
        // auto_scroll=true なので scroll_offset は末尾
        assert_eq!(state.scroll_offset, 1); // len - 1
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_ecs_log_events_loaded_recomputes_search_when_query_exists() {
    let mut app = app_with_ecs_log_view();
    let tab_id = active_tab_id(&app);
    // 検索クエリを設定
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.search_query = "error".to_string();
        }
    }
    let events = vec![
        create_test_log_event("info: ok"),
        create_test_log_event("error: something failed"),
        create_test_log_event("info: done"),
    ];

    app.handle_tab_event(tab_id, TabEvent::EcsLogEventsLoaded(Ok((events, None))));

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.events.len(), 3);
        // "error" を含む行は index 1
        assert_eq!(state.search_matches, vec![1]);
    } else {
        panic!("Expected Ecs ServiceData");
    }
}

#[test]
fn handle_tab_event_ecs_log_events_loaded_shows_error_when_err() {
    let mut app = app_with_ecs_log_view();
    let tab_id = active_tab_id(&app);

    app.handle_tab_event(
        tab_id,
        TabEvent::EcsLogEventsLoaded(Err(AppError::AwsApi("log fetch failed".to_string()))),
    );

    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// dispatch: ShowLogs テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_show_logs_sets_loading_when_tab_active() {
    let mut app = app_with_ecs_task_detail_for_logs();
    {
        let tab = app.active_tab_mut().unwrap();
        tab.loading = false;
    }

    let side_effect = app.dispatch(Action::ShowLogs);

    assert_eq!(side_effect, SideEffect::None);
    let tab = app.active_tab().unwrap();
    assert!(tab.loading);
}

// ──────────────────────────────────────────────
// dispatch: LogScroll* テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_log_scroll_down_increments_scroll_offset_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("line 1"),
                create_test_log_event("line 2"),
                create_test_log_event("line 3"),
            ];
            state.scroll_offset = 0;
            state.auto_scroll = false;
        }
    }

    app.dispatch(Action::LogScrollDown);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.scroll_offset, 1);
    }
}

#[test]
fn dispatch_log_scroll_up_decrements_scroll_offset_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("line 1"),
                create_test_log_event("line 2"),
            ];
            state.scroll_offset = 1;
            state.auto_scroll = false;
        }
    }

    app.dispatch(Action::LogScrollUp);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.scroll_offset, 0);
    }
}

#[test]
fn dispatch_log_scroll_to_top_sets_offset_to_zero_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("line 1"),
                create_test_log_event("line 2"),
            ];
            state.scroll_offset = 1;
        }
    }

    app.dispatch(Action::LogScrollToTop);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.scroll_offset, 0);
        assert!(!state.auto_scroll);
    }
}

#[test]
fn dispatch_log_scroll_to_bottom_sets_offset_to_last_and_enables_auto_scroll() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("line 1"),
                create_test_log_event("line 2"),
                create_test_log_event("line 3"),
            ];
            state.scroll_offset = 0;
            state.auto_scroll = false;
        }
    }

    app.dispatch(Action::LogScrollToBottom);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.scroll_offset, 2); // len - 1
        assert!(state.auto_scroll);
    }
}

// ──────────────────────────────────────────────
// dispatch: LogToggleAutoScroll テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_log_toggle_auto_scroll_toggles_state_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.auto_scroll = true;
            state.events = vec![create_test_log_event("line 1")];
        }
    }

    app.dispatch(Action::LogToggleAutoScroll);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert!(!state.auto_scroll);
    }
}

// ──────────────────────────────────────────────
// dispatch: LogSearch* テスト
// ──────────────────────────────────────────────

#[test]
fn dispatch_log_search_next_moves_to_next_match_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("error: first"),
                create_test_log_event("info: ok"),
                create_test_log_event("error: second"),
            ];
            state.search_query = "error".to_string();
            state.search_matches = vec![0, 2];
            state.current_match_index = Some(0);
            state.scroll_offset = 0;
        }
    }

    app.dispatch(Action::LogSearchNext);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.current_match_index, Some(1));
        assert_eq!(state.scroll_offset, 2);
    }
}

#[test]
fn dispatch_log_search_prev_moves_to_previous_match_when_in_log_view() {
    let mut app = app_with_ecs_log_view();
    {
        let tab = app.active_tab_mut().unwrap();
        if let ServiceData::Ecs { nav_level, .. } = &mut tab.data
            && let Some(nav) = nav_level
            && let Some(state) = nav.log_state_mut()
        {
            state.events = vec![
                create_test_log_event("error: first"),
                create_test_log_event("info: ok"),
                create_test_log_event("error: second"),
            ];
            state.search_query = "error".to_string();
            state.search_matches = vec![0, 2];
            state.current_match_index = Some(1);
            state.scroll_offset = 2;
        }
    }

    app.dispatch(Action::LogSearchPrev);

    let tab = app.active_tab().unwrap();
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        let state = nav_level.as_ref().unwrap().log_state().unwrap();
        assert_eq!(state.current_match_index, Some(0));
        assert_eq!(state.scroll_offset, 0);
    }
}

// ──────────────────────────────────────────────
// dispatch: ContainerSelect テスト
// ──────────────────────────────────────────────

fn app_with_container_select(purpose: ContainerSelectPurpose) -> App {
    let service = create_test_ecs_service(true);
    let tasks = vec![create_test_task(
        "abc123",
        &service.cluster_arn,
        vec![
            create_test_container("web", "RUNNING"),
            create_test_container("sidecar", "RUNNING"),
        ],
    )];
    let mut app = app_with_ecs_task_detail(service, tasks);
    let tab = app.active_tab_mut().unwrap();
    tab.mode = Mode::ContainerSelect(ContainerSelectState::new(
        vec!["web".to_string(), "sidecar".to_string()],
        purpose,
    ));
    app
}

#[test]
fn dispatch_container_select_down_increments_selected_index() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);

    app.dispatch(Action::ContainerSelectDown);

    let tab = app.active_tab().unwrap();
    assert!(matches!(
        &tab.mode,
        Mode::ContainerSelect(state) if state.selected_index == 1
    ));
}

#[test]
fn dispatch_container_select_down_clamps_at_max() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);
    let tab = app.active_tab_mut().unwrap();
    if let Mode::ContainerSelect(state) = &mut tab.mode {
        state.selected_index = 1;
    }

    app.dispatch(Action::ContainerSelectDown);

    let tab = app.active_tab().unwrap();
    assert!(matches!(
        &tab.mode,
        Mode::ContainerSelect(state) if state.selected_index == 1
    ));
}

#[test]
fn dispatch_container_select_up_decrements_selected_index() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);
    let tab = app.active_tab_mut().unwrap();
    if let Mode::ContainerSelect(state) = &mut tab.mode {
        state.selected_index = 1;
    }

    app.dispatch(Action::ContainerSelectUp);

    let tab = app.active_tab().unwrap();
    assert!(matches!(
        &tab.mode,
        Mode::ContainerSelect(state) if state.selected_index == 0
    ));
}

#[test]
fn dispatch_container_select_up_clamps_at_zero() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);

    app.dispatch(Action::ContainerSelectUp);

    let tab = app.active_tab().unwrap();
    assert!(matches!(
        &tab.mode,
        Mode::ContainerSelect(state) if state.selected_index == 0
    ));
}

#[test]
fn dispatch_container_select_cancel_returns_to_normal_mode() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);

    app.dispatch(Action::ContainerSelectCancel);

    let tab = app.active_tab().unwrap();
    assert_eq!(tab.mode, Mode::Normal);
}

#[test]
fn dispatch_container_select_confirm_enters_log_view_when_purpose_is_show_logs() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);
    let tab_id = active_tab_id(&app);
    app.pending_log_configs = Some((
        tab_id,
        vec![
            ContainerLogConfig {
                container_name: "web".to_string(),
                log_group: Some("/ecs/web".to_string()),
                stream_prefix: Some("ecs".to_string()),
                region: None,
            },
            ContainerLogConfig {
                container_name: "sidecar".to_string(),
                log_group: Some("/ecs/sidecar".to_string()),
                stream_prefix: Some("ecs".to_string()),
                region: None,
            },
        ],
    ));

    let side_effect = app.dispatch(Action::ContainerSelectConfirm);

    assert_eq!(side_effect, SideEffect::None);
    let tab = app.active_tab().unwrap();
    assert_eq!(tab.mode, Mode::Normal);
    if let ServiceData::Ecs { nav_level, .. } = &tab.data {
        match nav_level.as_ref().unwrap() {
            EcsNavLevel::LogView { log_state, .. } => {
                assert_eq!(log_state.container_name, "web");
                assert_eq!(log_state.log_group, "/ecs/web");
                assert_eq!(log_state.log_stream, "ecs/web/abc123");
            }
            other => panic!("Expected LogView, got {:?}", other),
        }
    } else {
        panic!("Expected Ecs ServiceData");
    }
    assert!(app.pending_log_configs.is_none());
}

#[test]
fn dispatch_container_select_handle_input_filters_names() {
    let mut app = app_with_container_select(ContainerSelectPurpose::ShowLogs);

    app.dispatch(Action::ContainerSelectHandleInput(
        tui_input::InputRequest::InsertChar('w'),
    ));

    let tab = app.active_tab().unwrap();
    if let Mode::ContainerSelect(state) = &tab.mode {
        assert_eq!(state.filter_input.value(), "w");
        assert_eq!(state.filtered_names, vec!["web".to_string()]);
        assert_eq!(state.selected_index, 0);
    } else {
        panic!("Expected ContainerSelect mode");
    }
}

// ──────────────────────────────────────────────
// ForceDeploy テスト
// ──────────────────────────────────────────────

#[test]
fn force_deploy_sets_confirm_mode_when_ecs_service_detail() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    app.dispatch(Action::ForceDeploy);
    assert_eq!(
        app.active_tab().unwrap().mode,
        Mode::Confirm(ConfirmAction::ForceDeployEcsService {
            service_name: "web-service".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
        })
    );
}

// ──────────────────────────────────────────────
// ScaleService テスト
// ──────────────────────────────────────────────

#[test]
fn scale_service_sets_form_mode_when_ecs_service_detail() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    app.dispatch(Action::ScaleService);

    let tab = app.active_tab().unwrap();
    let Mode::Form(ctx) = &tab.mode else {
        panic!("Expected Form mode");
    };
    assert_eq!(ctx.kind, FormKind::ScaleEcsService);
    assert_eq!(ctx.fields.len(), 1);
    assert_eq!(ctx.fields[0].label, "Desired Count");
    assert_eq!(ctx.fields[0].input.value(), "1"); // desired_count of test service
    assert!(ctx.fields[0].required);
}

// ──────────────────────────────────────────────
// StopTask テスト
// ──────────────────────────────────────────────

#[test]
fn stop_task_sets_confirm_mode_when_ecs_service_detail_with_tasks() {
    let service = create_test_ecs_service(false);
    let task = create_test_task("abc123", &service.cluster_arn, vec![]);
    let mut app = app_with_ecs_service_detail(service, vec![task]);
    app.dispatch(Action::StopTask);
    assert_eq!(
        app.active_tab().unwrap().mode,
        Mode::Confirm(ConfirmAction::StopEcsTask {
            task_arn: "arn:aws:ecs:ap-northeast-1:123456789012:task/production/abc123".to_string(),
            cluster_arn: "arn:aws:ecs:ap-northeast-1:123456789012:cluster/production".to_string(),
        })
    );
}

#[test]
fn stop_task_does_nothing_when_no_tasks() {
    let service = create_test_ecs_service(false);
    let mut app = app_with_ecs_service_detail(service, vec![]);
    app.dispatch(Action::StopTask);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

// ──────────────────────────────────────────────
// SecurityGroupsLoaded handle_event テスト
// ──────────────────────────────────────────────

fn create_test_security_groups() -> Vec<SecurityGroup> {
    vec![
        SecurityGroup {
            group_id: "sg-001".to_string(),
            group_name: "web-sg".to_string(),
            description: "Web security group".to_string(),
            inbound_rules: vec![SecurityGroupRule {
                protocol: "tcp".to_string(),
                port_range: "443".to_string(),
                source_or_destination: "0.0.0.0/0".to_string(),
                description: Some("HTTPS".to_string()),
            }],
            outbound_rules: vec![SecurityGroupRule {
                protocol: "-1".to_string(),
                port_range: "All".to_string(),
                source_or_destination: "0.0.0.0/0".to_string(),
                description: None,
            }],
        },
        SecurityGroup {
            group_id: "sg-002".to_string(),
            group_name: "db-sg".to_string(),
            description: "Database security group".to_string(),
            inbound_rules: vec![],
            outbound_rules: vec![],
        },
    ]
}

#[test]
fn handle_event_stores_security_groups_when_loaded_ok() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    let sgs = create_test_security_groups();
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::SecurityGroupsLoaded(Ok(sgs.clone())),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ec2 {
        security_groups, ..
    } = &tab.data
    {
        assert_eq!(security_groups.len(), 2);
        assert_eq!(security_groups[0].group_id, "sg-001");
        assert_eq!(security_groups[1].group_id, "sg-002");
    } else {
        panic!("Expected Ec2 ServiceData");
    }
}

#[test]
fn handle_event_clears_loading_when_security_groups_loaded_ok() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::SecurityGroupsLoaded(Ok(vec![])),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
}

#[test]
fn handle_event_shows_error_when_security_groups_loaded_err() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::SecurityGroupsLoaded(Err(AppError::AwsApi("access denied".to_string()))),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// EC2 CloudWatch Metrics テスト
// ──────────────────────────────────────────────

#[test]
fn handle_event_stores_metrics_when_loaded_ok() {
    use crate::aws::cloudwatch_model::{MetricDataPoint, MetricResult};
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    let metrics = vec![
        MetricResult {
            label: "CPUUtilization".to_string(),
            data_points: vec![MetricDataPoint {
                timestamp: 1700000000.0,
                value: 10.0,
            }],
        },
        MetricResult {
            label: "NetworkIn".to_string(),
            data_points: vec![MetricDataPoint {
                timestamp: 1700000000.0,
                value: 1024.0,
            }],
        },
    ];
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::MetricsLoaded(Ok(metrics)),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ec2 { metrics, .. } = &tab.data {
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0].label, "CPUUtilization");
        assert_eq!(metrics[1].label, "NetworkIn");
    } else {
        panic!("Expected Ec2 ServiceData");
    }
}

#[test]
fn handle_event_shows_error_when_metrics_loaded_err() {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::MetricsLoaded(Err(AppError::AwsApi("throttled".to_string()))),
    ));
    let tab = app.active_tab().unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// EC2 Security Group ナビゲーションテスト
// ──────────────────────────────────────────────

fn app_with_ec2_sg_detail() -> App {
    let mut app = app_with_ec2_tab();
    let tab_id = app.active_tab().unwrap().id;
    // インスタンスデータをセット
    let instance = create_test_instance("i-001", "web", InstanceState::Running);
    if let Some(tab) = app.active_tab_mut() {
        if let ServiceData::Ec2 { instances, .. } = &mut tab.data {
            instances.set_items(vec![instance]);
        }
        tab.tab_view = TabView::Detail;
        tab.detail_tab = DetailTab::SecurityGroups;
    }
    // SGデータをロード
    app.handle_event(AppEvent::TabEvent(
        tab_id,
        TabEvent::SecurityGroupsLoaded(Ok(create_test_security_groups())),
    ));
    app
}

#[test]
fn enter_sets_selected_sg_index_when_sg_tab() {
    let mut app = app_with_ec2_sg_detail();
    app.dispatch(Action::Enter);
    let tab = app.active_tab().unwrap();
    if let ServiceData::Ec2 {
        selected_sg_index, ..
    } = &tab.data
    {
        assert_eq!(*selected_sg_index, Some(0));
    } else {
        panic!("Expected Ec2 ServiceData");
    }
}

#[test]
fn back_clears_selected_sg_index_when_in_sg_rules() {
    let mut app = app_with_ec2_sg_detail();
    // Enter → ルール詳細
    app.dispatch(Action::Enter);
    // Esc → SG一覧に戻る
    app.dispatch(Action::Back);
    let tab = app.active_tab().unwrap();
    if let ServiceData::Ec2 {
        selected_sg_index, ..
    } = &tab.data
    {
        assert_eq!(*selected_sg_index, None);
    } else {
        panic!("Expected Ec2 ServiceData");
    }
    // タブビューはDetailのまま
    assert_eq!(tab.tab_view, TabView::Detail);
}

#[test]
fn back_returns_to_list_when_sg_tab_no_selection() {
    let mut app = app_with_ec2_sg_detail();
    // selected_sg_indexがNoneの状態でEsc → リストに戻る
    app.dispatch(Action::Back);
    let tab = app.active_tab().unwrap();
    assert_eq!(tab.tab_view, TabView::List);
}

// ──────────────────────────────────────────────
// ECR Detail (Image Delete) テスト
// ──────────────────────────────────────────────

#[test]
fn handle_delete_sets_danger_confirm_when_ecr_detail_with_permission() {
    let mut app = App::with_delete_permissions("dev".to_string(), None, DeletePermissions::All);
    app.create_tab(ServiceKind::Ecr);
    if let Some(tab) = app.active_tab_mut() {
        if let ServiceData::Ecr {
            repositories,
            images,
            ..
        } = &mut tab.data
        {
            repositories.set_items(vec![crate::aws::ecr_model::Repository {
                repository_name: "myapp/web".to_string(),
                repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web"
                    .to_string(),
                registry_id: "123456789012".to_string(),
                created_at: None,
                image_tag_mutability: "MUTABLE".to_string(),
            }]);
            *images = vec![crate::aws::ecr_model::Image {
                image_digest: "sha256:abc123".to_string(),
                image_tags: vec!["latest".to_string()],
                pushed_at: None,
                image_size_bytes: Some(1024),
            }];
        }
        tab.tab_view = TabView::Detail;
        tab.detail_tag_index = 0;
    }
    app.dispatch(Action::Delete);
    if let Mode::DangerConfirm(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(
            ctx.action,
            DangerAction::DeleteEcrImage {
                repository_name: "myapp/web".to_string(),
                image_digest: "sha256:abc123".to_string(),
            }
        );
    } else {
        panic!("Expected DangerConfirm mode");
    }
}

#[test]
fn handle_delete_shows_permission_denied_when_ecr_detail_no_permission() {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecr);
    if let Some(tab) = app.active_tab_mut() {
        if let ServiceData::Ecr {
            repositories,
            images,
            ..
        } = &mut tab.data
        {
            repositories.set_items(vec![crate::aws::ecr_model::Repository {
                repository_name: "myapp/web".to_string(),
                repository_uri: "123456789012.dkr.ecr.ap-northeast-1.amazonaws.com/myapp/web"
                    .to_string(),
                registry_id: "123456789012".to_string(),
                created_at: None,
                image_tag_mutability: "MUTABLE".to_string(),
            }]);
            *images = vec![crate::aws::ecr_model::Image {
                image_digest: "sha256:abc123".to_string(),
                image_tags: vec!["latest".to_string()],
                pushed_at: None,
                image_size_bytes: Some(1024),
            }];
        }
        tab.tab_view = TabView::Detail;
        tab.detail_tag_index = 0;
    }
    app.dispatch(Action::Delete);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
    assert_eq!(msg.title, "Permission Denied");
}

// ──────────────────────────────────────────────
// handle_tab_event: LifecyclePolicyLoaded テスト
// ──────────────────────────────────────────────

fn create_ecr_app_with_tab() -> (App, TabId) {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::Ecr);
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    (app, tab_id)
}

#[test]
fn handle_tab_event_lifecycle_policy_loaded_sets_data_when_ok() {
    let (mut app, tab_id) = create_ecr_app_with_tab();
    app.handle_tab_event(
        tab_id,
        TabEvent::LifecyclePolicyLoaded(Ok(Some(r#"{"rules":[{"rulePriority":1}]}"#.to_string()))),
    );
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr {
        lifecycle_policy, ..
    } = &tab.data
    {
        assert_eq!(
            *lifecycle_policy,
            Some(Some(r#"{"rules":[{"rulePriority":1}]}"#.to_string()))
        );
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_lifecycle_policy_loaded_sets_none_when_no_policy() {
    let (mut app, tab_id) = create_ecr_app_with_tab();
    app.handle_tab_event(tab_id, TabEvent::LifecyclePolicyLoaded(Ok(None)));
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr {
        lifecycle_policy, ..
    } = &tab.data
    {
        assert_eq!(*lifecycle_policy, Some(None));
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_lifecycle_policy_loaded_shows_error_when_err() {
    let (mut app, tab_id) = create_ecr_app_with_tab();
    app.handle_tab_event(
        tab_id,
        TabEvent::LifecyclePolicyLoaded(Err(AppError::AwsApi("access denied".to_string()))),
    );
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// ──────────────────────────────────────────────
// handle_tab_event: ScanResultLoaded テスト
// ──────────────────────────────────────────────

#[test]
fn handle_tab_event_scan_result_loaded_sets_data_when_ok() {
    use crate::aws::ecr_model::{FindingSeverity, ImageScanResult, ScanFinding};
    let (mut app, tab_id) = create_ecr_app_with_tab();
    let scan = ImageScanResult {
        findings: vec![ScanFinding {
            name: "CVE-2024-0001".to_string(),
            severity: FindingSeverity::High,
            description: "Test vulnerability".to_string(),
            uri: "https://example.com".to_string(),
        }],
        severity_counts: vec![(FindingSeverity::High, 1)],
    };
    app.handle_tab_event(tab_id, TabEvent::ScanResultLoaded(Ok(Some(scan.clone()))));
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr { scan_result, .. } = &tab.data {
        assert_eq!(*scan_result, Some(Some(scan)));
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_scan_result_loaded_sets_none_when_no_scan() {
    let (mut app, tab_id) = create_ecr_app_with_tab();
    app.handle_tab_event(tab_id, TabEvent::ScanResultLoaded(Ok(None)));
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::Ecr { scan_result, .. } = &tab.data {
        assert_eq!(*scan_result, Some(None));
    } else {
        panic!("Expected Ecr ServiceData");
    }
}

#[test]
fn handle_tab_event_scan_result_loaded_shows_error_when_err() {
    let (mut app, tab_id) = create_ecr_app_with_tab();
    app.handle_tab_event(
        tab_id,
        TabEvent::ScanResultLoaded(Err(AppError::AwsApi("access denied".to_string()))),
    );
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// --- BucketSettingsLoaded tests ---

fn create_s3_app_with_tab() -> (App, TabId) {
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::S3);
    let tab_id = app.active_tab().unwrap().id;
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = true;
    }
    (app, tab_id)
}

#[test]
fn handle_tab_event_bucket_settings_loaded_sets_data_when_ok() {
    use crate::aws::s3_model::BucketSettings;
    let (mut app, tab_id) = create_s3_app_with_tab();
    let settings = BucketSettings {
        region: "us-east-1".to_string(),
        versioning: "Enabled".to_string(),
        encryption: "AES256".to_string(),
    };
    app.handle_tab_event(tab_id, TabEvent::BucketSettingsLoaded(Ok(settings.clone())));
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::S3 {
        bucket_settings, ..
    } = &tab.data
    {
        assert_eq!(*bucket_settings, Some(Some(settings)));
    } else {
        panic!("Expected S3 ServiceData");
    }
}

#[test]
fn handle_tab_event_bucket_settings_loaded_shows_error_when_err() {
    let (mut app, tab_id) = create_s3_app_with_tab();
    app.handle_tab_event(
        tab_id,
        TabEvent::BucketSettingsLoaded(Err(AppError::AwsApi("access denied".to_string()))),
    );
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// --- ObjectContentLoaded tests ---

#[test]
fn handle_tab_event_object_content_loaded_sets_preview_when_ok() {
    use crate::aws::s3_model::ObjectContent;
    let (mut app, tab_id) = create_s3_app_with_tab();
    // プレビューをロード中状態にする
    if let Some(tab) = app.find_tab_mut(tab_id) {
        if let ServiceData::S3 { object_preview, .. } = &mut tab.data {
            *object_preview = Some(None); // ロード中
        }
    }
    let content = ObjectContent {
        content_type: "text/plain".to_string(),
        body: "Hello, World!".to_string(),
        size: 13,
    };
    app.handle_tab_event(tab_id, TabEvent::ObjectContentLoaded(Ok(content.clone())));
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::S3 {
        object_preview,
        preview_scroll,
        ..
    } = &tab.data
    {
        assert_eq!(*object_preview, Some(Some(content)));
        assert_eq!(*preview_scroll, 0);
    } else {
        panic!("Expected S3 ServiceData");
    }
}

#[test]
fn handle_tab_event_object_content_loaded_shows_error_and_clears_preview_when_err() {
    let (mut app, tab_id) = create_s3_app_with_tab();
    // プレビューをロード中状態にする
    if let Some(tab) = app.find_tab_mut(tab_id) {
        if let ServiceData::S3 { object_preview, .. } = &mut tab.data {
            *object_preview = Some(None);
        }
    }
    app.handle_tab_event(
        tab_id,
        TabEvent::ObjectContentLoaded(Err(AppError::AwsApi(
            "Binary files cannot be previewed".to_string(),
        ))),
    );
    let tab = app.find_tab(tab_id).unwrap();
    assert!(!tab.loading);
    if let ServiceData::S3 { object_preview, .. } = &tab.data {
        assert_eq!(*object_preview, None);
    } else {
        panic!("Expected S3 ServiceData");
    }
    assert!(app.message.is_some());
    let msg = app.message.as_ref().unwrap();
    assert_eq!(msg.level, MessageLevel::Error);
}

// --- Download tests ---

fn create_s3_detail_app_with_object() -> App {
    use crate::aws::s3_model::S3Object;
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::S3);
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = false;
        tab.tab_view = crate::tab::TabView::Detail;
        if let ServiceData::S3 {
            selected_bucket,
            objects,
            ..
        } = &mut tab.data
        {
            *selected_bucket = Some("my-bucket".to_string());
            *objects = vec![S3Object {
                key: "docs/readme.txt".to_string(),
                size: Some(1024),
                last_modified: Some("2024-01-01".to_string()),
                storage_class: Some("STANDARD".to_string()),
                is_prefix: false,
            }];
        }
    }
    app
}

#[test]
fn handle_download_sets_form_mode_when_s3_detail_with_file_selected() {
    let mut app = create_s3_detail_app_with_object();
    app.dispatch(Action::Download);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.kind, FormKind::DownloadS3Object);
        assert_eq!(ctx.fields.len(), 1);
        assert_eq!(ctx.fields[0].label, "Save Directory");
        assert!(ctx.fields[0].required);
    } else {
        panic!("Expected Form mode with DownloadS3Object");
    }
}

#[test]
fn handle_download_does_nothing_when_s3_detail_with_prefix_selected() {
    use crate::aws::s3_model::S3Object;
    let mut app = App::new("dev".to_string(), None);
    app.create_tab(ServiceKind::S3);
    if let Some(tab) = app.active_tab_mut() {
        tab.loading = false;
        tab.tab_view = crate::tab::TabView::Detail;
        if let ServiceData::S3 {
            selected_bucket,
            objects,
            ..
        } = &mut tab.data
        {
            *selected_bucket = Some("my-bucket".to_string());
            *objects = vec![S3Object {
                key: "docs/".to_string(),
                size: Some(0),
                last_modified: Some("".to_string()),
                storage_class: Some("".to_string()),
                is_prefix: true,
            }];
        }
    }
    app.dispatch(Action::Download);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

#[test]
fn handle_download_does_nothing_when_not_s3_detail() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::Download);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}

// --- Upload tests ---

#[test]
fn handle_upload_sets_form_mode_when_s3_detail() {
    let mut app = create_s3_detail_app_with_object();
    app.dispatch(Action::Upload);
    if let Mode::Form(ctx) = &app.active_tab().unwrap().mode {
        assert_eq!(ctx.kind, FormKind::UploadS3Object);
        assert_eq!(ctx.fields.len(), 1);
        assert_eq!(ctx.fields[0].label, "Local File Path");
        assert!(ctx.fields[0].required);
    } else {
        panic!("Expected Form mode with UploadS3Object");
    }
}

#[test]
fn handle_upload_does_nothing_when_not_s3_detail() {
    let mut app = app_with_ec2_tab();
    app.dispatch(Action::Upload);
    assert_eq!(app.active_tab().unwrap().mode, Mode::Normal);
}
