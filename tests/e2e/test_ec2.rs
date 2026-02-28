use std::time::Duration;

use crate::common::spawn_app;

#[test]
fn ec2_list_shows_instances_when_selected() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    // EC2は最初の項目なのでそのままEnterで選択
    h.send_enter().unwrap();
    // データのロード完了を待つ（モックインスタンス名が表示されるまで）
    h.wait_for_text("web-server-1")
        .expect("EC2 list should show mock instances");
    let contents = h.screen_contents();
    assert!(
        contents.contains("i-"),
        "EC2 list should show mock instance IDs, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ec2_detail_shows_info_when_entered() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("EC2 Instances").unwrap();
    // 最初のインスタンスを選択して詳細へ
    h.send_enter().unwrap();
    h.wait_for_text("i-0abc1234def56789a")
        .expect("Detail should show instance ID");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn back_returns_to_list_when_escape() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("EC2 Instances").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("i-0abc1234def56789a").unwrap();
    // Escで一覧に戻る
    h.send_esc().unwrap();
    h.wait_for_text("EC2 Instances")
        .expect("Should return to EC2 list");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn filter_narrows_list_when_typed() {
    let mut h = spawn_app(120, 24);
    h.wait_for_text("All Services").unwrap();
    h.send_enter().unwrap();
    // データのロード完了を待つ
    h.wait_for_text("web-server-1").unwrap();
    // フィルターモード開始
    h.send_slash().unwrap();
    // "web" と入力してフィルタリング
    h.send_text("web").unwrap();
    h.send_enter().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));
    let contents = h.screen_contents();
    assert!(
        contents.contains("web-server"),
        "Filter should narrow to web-server instance, got:\n{}",
        contents
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ec2_detail_shows_sg_tab_when_switched() {
    let mut h = spawn_app(100, 30);
    h.wait_for_text("All Services").unwrap();
    h.send_enter().unwrap();
    // データのロード完了を待つ
    h.wait_for_text("web-server-1").unwrap();
    // 最初のインスタンスの詳細へ
    h.send_enter().unwrap();
    // 詳細ビュー固有のテキストを待つ
    h.wait_for_text("[Overview]")
        .expect("Should enter detail view");
    // Tabキーで Overview → Tags → SecurityGroups
    h.send_tab().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    h.send_tab().unwrap();
    // SecurityGroupsタブのデータ読み込みを待つ
    h.wait_for_text("[Security Groups]")
        .expect("Security Groups tab should be active");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn ec2_detail_shows_metrics_tab_when_switched() {
    let mut h = spawn_app(100, 30);
    h.wait_for_text("All Services").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("web-server-1").unwrap();
    h.send_enter().unwrap();
    h.wait_for_text("[Overview]")
        .expect("Should enter detail view");
    // Tabキーで Overview → Tags → SecurityGroups → Metrics
    h.send_tab().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    h.send_tab().unwrap();
    std::thread::sleep(Duration::from_millis(500));
    h.send_tab().unwrap();
    // Metricsタブの表示を待つ
    h.wait_for_text("[Metrics]")
        .expect("Metrics tab should be active");
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}
