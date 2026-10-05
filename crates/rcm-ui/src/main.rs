use rcm_ui::app::AppController;
use rcm_ui::tui::TuiApp;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    let pipe_name = format!("rcm-{}", username);

    let app = AppController::new();

    // Zero-friction auto-connect & bootstrap:
    // Automatically ensures rcm-agent is running (auto-spawning in background if needed)
    // and ensures rclone is downloaded, verified, and running!
    if let Err(e) = app.ensure_connected_and_ready(&pipe_name).await {
        eprintln!("Initialization notice: {}. Continuing to interface...", e);
    }

    // Launch interactive UI
    let mut tui = TuiApp::new(app);
    tui.run().await?;

    Ok(())
}
