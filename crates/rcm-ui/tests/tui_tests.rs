use rcm_ui::app::AppController;
use rcm_ui_kit::view_model::SidebarDestination;

#[tokio::test]
async fn test_ui_destination_switching_and_view_rendering() {
    let app = AppController::new();

    // Verify initial destination
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Home
    );

    // Switch to Remotes
    app.switch_destination(SidebarDestination::Remotes).await;
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Remotes
    );

    // Switch to Mounts
    app.switch_destination(SidebarDestination::Mounts).await;
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Mounts
    );

    // Switch to Serves
    app.switch_destination(SidebarDestination::Serves).await;
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Serves
    );

    // Switch to Files
    app.switch_destination(SidebarDestination::Files).await;
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Files
    );

    // Switch to Settings
    app.switch_destination(SidebarDestination::Settings).await;
    assert_eq!(
        app.state().read().await.current_destination,
        SidebarDestination::Settings
    );
}
