use crate::common::spawn_app;

#[test]
fn dashboard_shows_services_when_started() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services")
        .expect("Dashboard should show 'All Services'");
    let contents = h.screen_contents();
    assert!(
        contents.contains("EC2"),
        "Dashboard should list EC2 service"
    );
    h.send_char('q').unwrap();
    h.wait_exit().unwrap();
}

#[test]
fn quit_exits_when_q_pressed() {
    let mut h = spawn_app(80, 24);
    h.wait_for_text("All Services").unwrap();
    h.send_char('q').unwrap();
    let status = h.wait_exit().unwrap();
    assert!(status.success(), "Process should exit cleanly");
}
