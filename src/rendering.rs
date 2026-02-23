use awsui::app::{App, ConfirmAction, Mode};
use awsui::service::ServiceKind;
use awsui::tab::TabView;
use awsui::tui::components::dialog::{ConfirmDialog, MessageDialog};
use awsui::tui::components::form_dialog::{DangerConfirmDialog, FormDialog};
use awsui::tui::components::help::HelpPopup;
use awsui::tui::components::tab_bar::TabBar;
use ratatui::Frame;

pub(crate) fn render(frame: &mut Frame, app: &App, spinner_tick: usize) {
    // プロファイル選択画面
    if let Some(ps) = &app.profile_selector {
        awsui::tui::views::profile_select::render(frame, ps, spinner_tick);
        // グローバルオーバーレイ（メッセージダイアログなど）
        render_global_overlays(frame, app);
        return;
    }

    if app.show_dashboard {
        awsui::tui::views::dashboard::render(frame, app);
    } else if let Some(tab) = app.active_tab() {
        // タブが2つ以上の場合のみタブバーを表示
        if app.tabs.len() > 1 {
            let area = frame.area();
            let chunks = ratatui::layout::Layout::vertical([
                ratatui::layout::Constraint::Length(1), // タブバー
                ratatui::layout::Constraint::Min(1),    // コンテンツ
            ])
            .split(area);

            let tab_bar = TabBar::new(&app.tabs, app.active_tab_index);
            frame.render_widget(tab_bar, chunks[0]);

            // コンテンツ部分を描画（サブフレーム的にclipする）
            render_tab_content(frame, app, tab, spinner_tick, chunks[1]);
        } else {
            render_tab_content(frame, app, tab, spinner_tick, frame.area());
        }

        // モーダルオーバーレイ（タブ固有モード）
        render_tab_overlays(frame, tab);

        // グローバルオーバーレイ
        render_global_overlays(frame, app);
        return;
    }

    // グローバルオーバーレイ（ダッシュボード時）
    render_global_overlays(frame, app);
}

fn render_tab_content(
    frame: &mut Frame,
    app: &App,
    tab: &awsui::tab::Tab,
    spinner_tick: usize,
    area: ratatui::layout::Rect,
) {
    match app.current_view() {
        Some((ServiceKind::Ec2, TabView::List)) => {
            awsui::tui::views::ec2_list::render(frame, app, spinner_tick, area)
        }
        Some((ServiceKind::Ec2, TabView::Detail)) => {
            awsui::tui::views::ec2_detail::render(frame, app, area)
        }
        Some((ServiceKind::Ecr, TabView::List)) => {
            if let awsui::tab::ServiceData::Ecr { repositories, .. } = &tab.data {
                let props = awsui::tui::views::ecr_list::EcrListProps {
                    repositories: &repositories.filtered,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                    profile: app.profile.as_deref(),
                    region: app.region.as_deref(),
                };
                awsui::tui::views::ecr_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::Ecr, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Ecr {
                repositories,
                images,
                ..
            } = &tab.data
                && let Some(repo) = repositories.filtered.get(tab.selected_index)
            {
                awsui::tui::views::ecr_detail::render(
                    frame,
                    repo,
                    images,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        Some((ServiceKind::Ecs, TabView::List)) => {
            if let awsui::tab::ServiceData::Ecs { clusters, .. } = &tab.data {
                let props = awsui::tui::views::ecs_list::EcsListProps {
                    clusters: &clusters.filtered,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                };
                awsui::tui::views::ecs_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::Ecs, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Ecs {
                clusters,
                services,
                tasks,
                nav_level,
                ..
            } = &tab.data
                && let Some(cluster) = clusters.filtered.get(tab.selected_index)
            {
                match nav_level {
                    Some(awsui::tab::EcsNavLevel::LogView { log_state, .. }) => {
                        awsui::tui::views::ecs_log::render(
                            frame,
                            log_state,
                            tab.loading,
                            spinner_tick,
                            &tab.mode,
                            tab.filter_input.value(),
                            area,
                        );
                    }
                    Some(awsui::tab::EcsNavLevel::TaskDetail { task_index, .. }) => {
                        if let Some(task) = tasks.get(*task_index) {
                            awsui::tui::views::ecs_task_detail::render(frame, task, area);
                        }
                    }
                    Some(awsui::tab::EcsNavLevel::ServiceDetail { service_index }) => {
                        if let Some(service) = services.get(*service_index) {
                            awsui::tui::views::ecs_service_detail::render(
                                frame,
                                service,
                                tasks,
                                tab.detail_tag_index,
                                tab.loading,
                                spinner_tick,
                                area,
                            );
                        }
                    }
                    _ => {
                        // ClusterDetail or None
                        awsui::tui::views::ecs_detail::render(
                            frame,
                            cluster,
                            services,
                            tab.detail_tag_index,
                            tab.loading,
                            spinner_tick,
                            area,
                        );
                    }
                }
            }
        }
        Some((ServiceKind::S3, TabView::List)) => {
            if let awsui::tab::ServiceData::S3 { buckets, .. } = &tab.data {
                let props = awsui::tui::views::s3_list::S3ListProps {
                    buckets: &buckets.filtered,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                    profile: app.profile.as_deref(),
                    region: app.region.as_deref(),
                };
                awsui::tui::views::s3_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::S3, TabView::Detail)) => {
            if let awsui::tab::ServiceData::S3 {
                objects,
                selected_bucket,
                current_prefix,
                ..
            } = &tab.data
                && let Some(bucket_name) = selected_bucket
            {
                awsui::tui::views::s3_detail::render(
                    frame,
                    bucket_name,
                    objects,
                    current_prefix,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    area,
                );
            }
        }
        Some((ServiceKind::Vpc, TabView::List)) => {
            if let awsui::tab::ServiceData::Vpc { vpcs, .. } = &tab.data {
                let props = awsui::tui::views::vpc_list::VpcListProps {
                    vpcs: &vpcs.filtered,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                    profile: app.profile.as_deref(),
                    region: app.region.as_deref(),
                };
                awsui::tui::views::vpc_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::Vpc, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Vpc { vpcs, subnets, .. } = &tab.data
                && let Some(vpc) = vpcs.filtered.get(tab.selected_index)
            {
                awsui::tui::views::vpc_detail::render(
                    frame,
                    vpc,
                    subnets,
                    tab.detail_tag_index,
                    tab.loading,
                    spinner_tick,
                    area,
                );
            }
        }
        Some((ServiceKind::SecretsManager, TabView::List)) => {
            if let awsui::tab::ServiceData::Secrets { secrets, .. } = &tab.data {
                let props = awsui::tui::views::secrets_list::SecretsListProps {
                    secrets: &secrets.filtered,
                    selected_index: tab.selected_index,
                    filter_input: &tab.filter_input,
                    mode: &tab.mode,
                    loading: tab.loading,
                    spinner_tick,
                    profile: app.profile.as_deref(),
                    region: app.region.as_deref(),
                };
                awsui::tui::views::secrets_list::render(frame, &props, area);
            }
        }
        Some((ServiceKind::SecretsManager, TabView::Detail)) => {
            if let awsui::tab::ServiceData::Secrets {
                detail,
                detail_tab,
                value_visible,
                ..
            } = &tab.data
                && let Some(detail) = detail
            {
                awsui::tui::views::secrets_detail::render(
                    frame,
                    detail,
                    tab.detail_tag_index,
                    detail_tab,
                    *value_visible,
                    app.profile.as_deref(),
                    app.region.as_deref(),
                    area,
                );
            }
        }
        None => {}
    }
}

fn render_tab_overlays(frame: &mut Frame, tab: &awsui::tab::Tab) {
    match &tab.mode {
        Mode::Confirm(action) => {
            let msg = match action {
                ConfirmAction::Start(id) => format!("Start instance {}?", id),
                ConfirmAction::Stop(id) => format!("Stop instance {}?", id),
                ConfirmAction::Reboot(id) => format!("Reboot instance {}?", id),
            };
            let dialog = ConfirmDialog::new(&msg);
            frame.render_widget(dialog, frame.area());
        }
        Mode::Form(ctx) => {
            let dialog = FormDialog::new(ctx);
            frame.render_widget(dialog, frame.area());
        }
        Mode::DangerConfirm(ctx) => {
            let dialog = DangerConfirmDialog::new(ctx);
            frame.render_widget(dialog, frame.area());
        }
        Mode::ContainerSelect {
            names, selected, ..
        } => {
            let popup = awsui::tui::components::dialog::centered_rect(
                50,
                (names.len() as u16 + 5).min(20),
                frame.area(),
            );
            frame.render_widget(ratatui::widgets::Clear, popup);
            let block = ratatui::widgets::Block::default()
                .title(" Select Container ")
                .borders(ratatui::widgets::Borders::ALL)
                .style(awsui::tui::theme::active());
            let inner = block.inner(popup);
            frame.render_widget(block, popup);
            let selector =
                awsui::tui::components::list_selector::ListSelector::new("", names, *selected);
            frame.render_widget(selector, inner);
        }
        _ => {}
    }
}

fn render_global_overlays(frame: &mut Frame, app: &App) {
    // サービスピッカーオーバーレイ
    if let Some(picker) = &app.service_picker {
        let widget = awsui::tui::components::service_picker::ServicePicker::new(picker);
        frame.render_widget(widget, frame.area());
    }

    if let Some(msg) = &app.message {
        let dialog = MessageDialog::new(msg);
        frame.render_widget(dialog, frame.area());
    }
    if app.show_help
        && let Some(view) = app.current_view()
    {
        let help = HelpPopup::new(view);
        frame.render_widget(help, frame.area());
    }
}
