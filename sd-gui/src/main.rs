#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::path::PathBuf;

use clap::Parser;
use sd_core::LP_BACKEND;

#[derive(Parser)]
#[command(
    help_template = "\
{before-help}{name} {version}
{author-with-newline}
{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
",
    author,
    version,
    about
)]
/// String diagram visualiser
///
/// Homepage: <https://calintat.github.io/sd-visualiser>
///
/// Please report bugs at <https://github.com/calintat/sd-visualiser/issues>.
struct Args {
    /// Read in a chil file
    #[arg(long, value_name = "FILE")]
    chil: Option<PathBuf>,

    /// Read in a spartan file
    #[arg(long, value_name = "FILE")]
    spartan: Option<PathBuf>,
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).

    use anyhow::anyhow;
    tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .with_thread_names(true)
        .init();
    tracing::info!("lp solver: {}", LP_BACKEND);

    let args = Args::parse();

    let native_options = eframe::NativeOptions {
        maximized: true,
        ..Default::default()
    };

    let file = if let Some(path) = args.chil {
        let code = std::fs::read_to_string(path)?;
        Some((code, sd_gui::UiLanguage::Chil))
    } else if let Some(path) = args.spartan {
        let code = std::fs::read_to_string(path)?;
        Some((code, sd_gui::UiLanguage::Spartan))
    } else {
        None
    };
    eframe::run_native(
        "SD Visualiser",
        native_options,
        Box::new(|cc| {
            let mut app = sd_gui::App::new(cc);

            if let Some((code, language)) = file {
                app.set_file(&code, Some(language));
            }

            Box::new(app)
        }),
    )
    .map_err(|err| anyhow!("{}", err))?;

    Ok(())
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    tracing_wasm::set_as_global_default();
    tracing::info!("lp solver: {}", LP_BACKEND);

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| Box::new(sd_gui::App::new(cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
