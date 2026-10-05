use rcm_ui::app::AppController;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Launching Rclone Manager (RCM) Desktop UI...");

    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    let pipe_name = format!("rcm-{}", username);

    let app = AppController::new();

    // Attempt connecting to the background resident agent (DM-7)
    match app.connect_to_agent(&pipe_name).await {
        Ok(_) => println!("Connected to rcm-agent via IPC."),
        Err(e) => println!("Notice: rcm-agent is not running ({}). Start it with 'rcmctl start' or run 'rcm-agent'.", e),
    }

    let dashboard = app.get_dashboard_view_model().await;
    println!("Dashboard initialized. Daemon state: {:?}", dashboard.daemon_state);

    Ok(())
}
