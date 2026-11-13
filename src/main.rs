mod config;
mod curves;
mod desktop;
mod devices;
mod driver;
mod engine;
mod governor;
#[cfg(all(feature = "gui", unix))]
mod gui;
mod i18n;
#[cfg(unix)]
mod ipc;
mod kraken_x;
mod lighting;
mod permissions;
mod protocol;
mod sensors;
mod smart_device;
mod specs;
#[cfg(all(feature = "tray", target_os = "linux"))]
mod tray;

use anyhow::{Context, Result, bail};
use clap::Parser;
use engine::Request;
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(
    version,
    about = "NZXT CAM — native cooling, lighting and monitoring for Ubuntu 24.04+"
)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, help = "Run the controller in the foreground without a GUI")]
    headless: bool,
    #[arg(long, help = "Start with the window hidden in the tray")]
    minimized: bool,
    #[arg(long, help = "Print the running controller state as JSON")]
    status: bool,
    #[arg(long, help = "List supported devices without opening them")]
    list_devices: bool,
    #[arg(long, help = "Print one sensor reading as JSON without a controller")]
    sensors: bool,
    #[arg(
        long,
        value_names = ["CHANNEL", "JSON_FILE"],
        num_args = 2,
        help = "Apply and save a cooling profile, e.g. kraken_x:SERIAL/pump profile.json"
    )]
    set_cooling: Option<Vec<String>>,
    #[arg(
        long,
        value_names = ["CHANNEL", "JSON_FILE"],
        num_args = 2,
        help = "Apply and save lighting, e.g. kraken_x:SERIAL/ring lighting.json"
    )]
    set_lighting: Option<Vec<String>>,
    #[arg(long, help = "Review the udev rule plan for device access")]
    prepare_permissions: bool,
    #[arg(long, help = "Review the plan that removes the udev rule")]
    prepare_permissions_removal: bool,
    #[arg(long, value_name = "PLAN_ID", help = "Confirm a reviewed plan")]
    confirm_plan: Option<u64>,
    #[arg(long, help = "Discard the pending plan")]
    cancel_plan: bool,
    #[arg(long)]
    install_autostart: bool,
    #[arg(long)]
    remove_autostart: bool,
    #[arg(long, help = "Ask the running controller to quit")]
    quit: bool,
    #[arg(long, hide = true)]
    permissions_helper: Option<String>,
}

fn print_reply(mut reply: engine::Reply, config_path: &Path) -> Result<()> {
    let language = reply.state.config.ui.language.clone();
    if let Ok(catalog) = i18n::load(&language, config_path) {
        i18n::activate(catalog);
    }
    reply.message = i18n::tr(&reply.message);
    println!("{}", serde_json::to_string_pretty(&reply)?);
    if !reply.ok {
        bail!("{}", reply.message);
    }
    Ok(())
}

#[cfg(unix)]
fn request(request: Request, config_path: &Path) -> Result<()> {
    print_reply(ipc::send(&request)?, config_path)
}

#[cfg(not(unix))]
fn request(_: Request, _: &Path) -> Result<()> {
    bail!("O controlador só está disponível no Linux")
}

fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("Não foi possível ler {path}"))?;
    serde_json::from_str(&text).with_context(|| format!("JSON inválido em {path}"))
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Some(action) = &args.permissions_helper {
        // Executado pelo pkexec, antes de qualquer GUI ou IPC.
        return permissions::helper(action);
    }
    let config_path = match args.config.clone() {
        Some(path) => path,
        None => config::default_path()?,
    };
    if args.list_devices {
        let found = devices::discover(Path::new("/sys"), Path::new("/dev"));
        println!("{}", serde_json::to_string_pretty(&found)?);
        return Ok(());
    }
    if args.sensors {
        let mut sensors = sensors::Sensors::default();
        sensors.read();
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!("{}", serde_json::to_string_pretty(&sensors.read())?);
        return Ok(());
    }
    if args.install_autostart {
        desktop::install_autostart()?;
        println!("{}", desktop::autostart_path()?.display());
        return Ok(());
    }
    if args.remove_autostart {
        return desktop::remove_autostart();
    }
    let pending = if args.status {
        Some(Request::Status)
    } else if args.quit {
        Some(Request::Quit)
    } else if args.prepare_permissions {
        Some(Request::PreparePermissions {
            action: permissions::Action::Install,
        })
    } else if args.prepare_permissions_removal {
        Some(Request::PreparePermissions {
            action: permissions::Action::Remove,
        })
    } else if let Some(id) = args.confirm_plan {
        Some(Request::ConfirmPlan { id })
    } else if args.cancel_plan {
        Some(Request::CancelPlan)
    } else if let Some(values) = &args.set_cooling {
        Some(Request::SetCooling {
            channel: values[0].clone(),
            cooling: read_json(&values[1])?,
        })
    } else if let Some(values) = &args.set_lighting {
        Some(Request::SetLighting {
            channel: values[0].clone(),
            lighting: read_json(&values[1])?,
        })
    } else {
        None
    };
    if let Some(pending) = pending {
        return request(pending, &config_path);
    }
    run(args, config_path)
}

#[cfg(unix)]
fn run(args: Args, config_path: PathBuf) -> Result<()> {
    let config = config::Config::load(&config_path)?;
    if let Ok(catalog) = i18n::load(&config.ui.language, &config_path) {
        i18n::activate(catalog);
    }
    #[cfg(feature = "gui")]
    if !args.headless {
        // Uma segunda abertura apenas mostra a janela existente.
        if ipc::send(&Request::Show).is_ok() {
            return Ok(());
        }
        let controller = ipc::Controller::spawn(&config_path)?;
        let minimized = args.minimized || config.ui.start_minimized;
        return gui::run(controller, minimized);
    }
    let controller = ipc::Controller::spawn(&config_path)?;
    let tx = controller.tx.clone();
    ctrlc::set_handler(move || {
        let _ = tx.send(Request::Quit);
    })?;
    eprintln!("NZXT CAM em execução sem janela; use --status ou --quit.");
    let _ = args.minimized;
    controller.wait();
    Ok(())
}

#[cfg(not(unix))]
fn run(_: Args, _: PathBuf) -> Result<()> {
    bail!("O NZXT CAM para Ubuntu só executa no Linux; use este build apenas para testes")
}
