use rcm_core::DaemonState;
use rcm_ui_kit::view_model::{MountItemViewModel, SidebarDestination};
use rcm_ui::app::AppController;

/// Traceability: FR-UI-03, FR-UI-04, NFR-AC-01
/// Tests progressive disclosure navigation, command palette search filtering, and dashboard stats
#[tokio::test]
async fn test_fr_ui_03_ui_04_command_palette_and_dashboard_stats() {
    let app = AppController::new();

    // 1. Initial destination is Home (Dashboard)
    {
        let state = app.state();
        let s = state.read().await;
        assert_eq!(s.current_destination, SidebarDestination::Home);
        assert!(!s.command_palette_open);
    }

    // 2. Navigation
    app.switch_destination(SidebarDestination::Mounts).await;
    {
        let state = app.state();
        let s = state.read().await;
        assert_eq!(s.current_destination, SidebarDestination::Mounts);
    }

    // 3. Command palette toggling (FR-UI-03)
    app.toggle_command_palette().await;
    {
        let state = app.state();
        let s = state.read().await;
        assert!(s.command_palette_open);
    }

    // 4. Command palette search filtering (FR-UI-04)
    app.set_command_palette_query("remotes").await;
    let items = app.get_command_palette_items().await;
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i.destination == Some(SidebarDestination::Remotes)));

    // 5. Dashboard view model aggregation
    {
        let state = app.state();
        let mut s = state.write().await;
        s.daemon_state = DaemonState::Ready {
            execute_id: "exec-1".to_string(),
            version: "v1.75.1".to_string(),
            pid: 1234,
            addr: "http://127.0.0.1:5572".to_string(),
        };
        s.mounts.push(MountItemViewModel {
            id: rcm_core::Id::new(),
            name: "GDrive".to_string(),
            remote: "gdrive:Media".to_string(),
            target: "G:".to_string(),
            preset: "Streaming".to_string(),
            is_mounted: true,
            cache_used_bytes: Some(1024 * 1024 * 50),
            upload_queue_count: 0,
        });
    }

    let dashboard = app.get_dashboard_view_model().await;
    assert!(dashboard.daemon_state.is_ready());
    assert_eq!(dashboard.active_mounts_count, 1);
    assert_eq!(dashboard.active_serves_count, 0);
}
