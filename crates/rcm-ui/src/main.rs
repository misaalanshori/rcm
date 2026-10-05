use gpui_kit::*;
use rcm_ui::app::AppController;
use rcm_ui::gpui_window::RcmDesktopWindow;

fn main() {
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    let pipe_name = format!("rcm-{}", username);

    let app = AppController::new();
    let _guard = app.tokio_handle().enter();

    // Start native GPUI Desktop Application (SRDD §1.1, §2.4, §6.1, §7.11)
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        let window_options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some("Rclone Manager".into()),
                ..Default::default()
            }),
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1080.0), px(720.0)),
                cx,
            ))),
            ..Default::default()
        };

        let app_clone = app.clone();

        match gpui_kit::open_window(window_options, cx, move |_window, cx| {
            cx.new(|cx| {
                let mut win = RcmDesktopWindow::new(app_clone);
                win.refresh_state(cx);
                win
            })
        }) {
            Ok(_) => println!("Successfully opened GPUI window"),
            Err(e) => eprintln!("Failed to open GPUI window: {:?}", e),
        }

        // Spawn background connection/bootstrap task on App
        cx.spawn(async move |_cx| {
            let _ = app.ensure_connected_and_ready(&pipe_name).await;
        })
        .detach();
    });
}
