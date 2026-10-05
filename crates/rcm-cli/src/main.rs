use clap::{Parser, Subcommand};
use rcm_core::DaemonState;
use rcm_ipc::IpcClient;
use rcm_platform::credential::get_credential;

#[derive(Parser, Debug)]
#[command(name = "rcmctl", about = "Rclone Manager control CLI and helper")]
struct Cli {
    #[arg(long, help = "Custom IPC pipe/socket name")]
    pipe: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Show current daemon status")]
    Status,

    #[command(about = "Start rclone daemon")]
    Start,

    #[command(about = "Stop rclone daemon")]
    Stop,

    #[command(about = "Restart rclone daemon")]
    Restart,

    #[command(about = "Trigger desired-state reconciliation")]
    Reconcile,

    #[command(about = "Print rclone configuration password from keyring (for --password-command)")]
    PrintConfigPass {
        #[arg(default_value = "rclone_config_password", help = "Keyring credential key")]
        key: String,
    },

    #[command(about = "List configured mount profiles")]
    ListMounts,

    #[command(about = "List configured serve profiles")]
    ListServes,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // The print-config-pass command is invoked directly by rclone and must NOT require a running agent! (CF-8, §7.12)
    if let Commands::PrintConfigPass { key } = cli.command {
        match get_credential(&key) {
            Ok(pass) => {
                println!("{}", pass);
                return Ok(());
            }
            Err(e) => {
                eprintln!("Error retrieving password for key '{}': {}", key, e);
                std::process::exit(1);
            }
        }
    }

    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    let pipe_name = cli.pipe.unwrap_or_else(|| format!("rcm-{}", username));

    let client = match IpcClient::connect(&pipe_name).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to connect to rcm-agent via IPC ({}). Is rcm-agent running?", e);
            std::process::exit(1);
        }
    };

    match cli.command {
        Commands::Status => {
            let res = client.call("daemon.status", serde_json::json!({})).await?;
            let state: DaemonState = serde_json::from_value(res)?;
            match state {
                DaemonState::Ready { version, pid, addr, .. } => {
                    println!("Daemon Status: READY");
                    println!("  Version: {}", version);
                    println!("  PID:     {}", pid);
                    println!("  Address: {}", addr);
                }
                DaemonState::Stopped => println!("Daemon Status: STOPPED"),
                DaemonState::Starting => println!("Daemon Status: STARTING"),
                DaemonState::Stopping => println!("Daemon Status: STOPPING"),
                DaemonState::Degraded { reason } => println!("Daemon Status: DEGRADED ({})", reason),
                DaemonState::Crashed { exit_code, last_error } => {
                    println!("Daemon Status: CRASHED (code: {:?}, error: {})", exit_code, last_error)
                }
                DaemonState::Failed { reason } => println!("Daemon Status: FAILED ({})", reason),
            }
        }
        Commands::Start => {
            client.call("daemon.start", serde_json::json!({})).await?;
            println!("Daemon start command sent successfully.");
        }
        Commands::Stop => {
            client.call("daemon.stop", serde_json::json!({})).await?;
            println!("Daemon stop command sent successfully.");
        }
        Commands::Restart => {
            client.call("daemon.restart", serde_json::json!({})).await?;
            println!("Daemon restart command sent successfully.");
        }
        Commands::Reconcile => {
            let res = client.call("reconcile.now", serde_json::json!({})).await?;
            println!("Reconciliation complete: {}", res);
        }
        Commands::ListMounts => {
            let res = client.call("profiles.list_mounts", serde_json::json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        Commands::ListServes => {
            let res = client.call("profiles.list_serves", serde_json::json!({})).await?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        Commands::PrintConfigPass { .. } => unreachable!(),
    }

    Ok(())
}
