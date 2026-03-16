#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Console::AttachConsole;

use clap::Parser;
use image::DynamicImage;
use image::ImageReader;
use std::io::Cursor;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::{
    EnvFilter, Layer, fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt,
};

mod tray;

/// Simple Popup dictionary
#[derive(Parser, Debug)]
#[command(name = "popup dictionary", version, about, long_about = None, arg_required_else_help(false))]
struct Args {
    #[clap(flatten)]
    modes: Modes,

    #[clap(flatten)]
    options: Options,
}

#[derive(clap::Args, Debug)]
#[command(next_help_heading = "Modes")]
#[group(required = false, multiple = false)]
struct Modes {
    /// Provide input text manually
    #[arg(short = 't', long = "text", value_name = "STRING")]
    text: Option<String>,

    /// Get input text from primary clipboard/selection
    #[arg(short = 'p', long = "primary")]
    #[cfg(target_os = "linux")]
    primary: bool,

    /// Get input text from secondary clipboard/selection (x11)
    #[arg(short = 's', long = "secondary")]
    #[cfg(target_os = "linux")]
    secondary: bool,

    /// Get input text from clipboard
    #[arg(short = 'b', long = "clipboard")]
    clipboard: bool,

    /// Watch clipboard for newly copied text or image data
    #[arg(short = 'w', long = "watch")]
    watch: bool,

    /// Use OCR mode. Reads image from path if provided, otherwise takes image data from stdin
    #[arg(short = 'o', long = "ocr", value_name = "PATH")]
    ocr: Option<Option<PathBuf>>,
}

#[derive(clap::Args, Debug)]
#[group(required = false, multiple = true)]
struct Options {
    /// Initial plugin to load. Available: "kihon", "jotoba"
    #[arg(long = "initial-plugin", value_name = "PLUGIN", help_heading = None)]
    initial_plugin: Option<String>,

    /// Try to open the window at the mouse cursor. Unlikely to work on wayland
    #[arg(short = 'm', long = "at-mouse", help_heading = None)]
    open_at_cursor: bool,

    /// Display input text in a text-box instead of in one line
    #[arg(short = 'f', long = "full-text", help_heading = None)]
    wrapped: bool,

    /// Initial window width in pixels. Default: 450
    #[arg(long = "width", value_name = "PIXELS", help_heading = None)]
    initial_width: Option<u16>,

    /// Initial window height in pixels. Default: 450
    #[arg(long = "height", value_name = "PIXELS", help_heading = None)]
    initial_height: Option<u16>,

    /// Show a tray icon
    #[arg(long = "tray", help_heading = None)]
    show_tray_icon: bool,

    /// Enable verbose logging to terminal/console
    #[arg(long = "verbose", help_heading = None)]
    verbose: bool,

    #[cfg(target_os = "linux")]
    /// Enable logging to a file. A path to a file or directory can optionally be provided. Default: ~/.local/share/popup_dictionary/log.txt
    #[arg(long = "log-file", value_name = "PATH", help_heading = None)]
    log_file: Option<Option<PathBuf>>,

    #[cfg(target_os = "windows")]
    /// Enable logging to a file. A path to a folder or file can optionally be provided. Default: %APPDATA%\popup_dictionary\log.txt
    #[arg(long = "log-file", value_name = "PATH", help_heading = None)]
    log_file: Option<Option<PathBuf>>,

    #[arg(long = "font", help_heading = None)]
    /// Specify the name of a font installed on your system to be used for the UI. Default: Noto Sans CJK JP
    font: Option<String>,
}

const ATTACH_PARENT_PROCESS: u32 = u32::MAX;

fn main() -> ExitCode {
    // Try to attach to console on Windows
    #[cfg(target_os = "windows")]
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) != 0 {
            use std::fs::File;
            use std::io::{stderr, stdout};
            use std::os::windows::io::FromRawHandle;
        }
    }

    let cli: Args = Args::parse();

    init_logging(cli.options.verbose, cli.options.log_file);
    tracing::info!(
        "{} {} starting.",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    let config: popup_dictionary::app::Config = popup_dictionary::app::Config {
        initial_plugin: cli.options.initial_plugin,
        open_at_cursor: cli.options.open_at_cursor,
        wrapped: cli.options.wrapped,
        initial_width: cli.options.initial_width.unwrap_or(450),
        initial_height: cli.options.initial_height.unwrap_or(450),
        show_tray_icon: cli.options.show_tray_icon,
        font: cli.options.font.unwrap_or(String::from("Noto Sans CJK JP")),
    };

    #[cfg(target_os = "linux")]
    {
        if config.show_tray_icon {
            crate::tray::spawn_tray_icon();
        }

        if let Some(text) = &cli.modes.text {
            if let Err(e) = popup_dictionary::run(&text, config) {
                tracing::error!("Failed while running text mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.primary {
            if let Err(e) = popup_dictionary::primary(config) {
                tracing::error!("Failed while running primary mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.secondary {
            if let Err(e) = popup_dictionary::secondary(config) {
                tracing::error!("Failed while running secondary mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.clipboard {
            if let Err(e) = popup_dictionary::clipboard(config) {
                tracing::error!("Failed while running clipboard mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.watch {
            if let Err(e) = popup_dictionary::watch(config) {
                tracing::error!("Failed while running watch mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if let Some(ocr_path) = cli.modes.ocr {
            match get_image_for_ocr(ocr_path) {
                Ok(image) => {
                    if let Err(e) = popup_dictionary::ocr(image, config) {
                        tracing::error!("Failed while running ocr mode due to error: {e}");
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Could not find path or image data to run OCR mode with error: {e}"
                    );
                    return ExitCode::FAILURE;
                }
            }
        } else {
            tracing::info!("No mode specified. Defaulting to watch mode with tray icon.");
            // Default to watch mode with tray icon if no mode set
            if !config.show_tray_icon {
                crate::tray::spawn_tray_icon();
            }
            if let Err(e) = popup_dictionary::watch(config) {
                tracing::error!("Failed while running watch mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        if config.show_tray_icon {
            crate::tray::spawn_tray_icon();
        }

        if let Some(text) = &cli.modes.text {
            if let Err(e) = popup_dictionary::run(&text, config) {
                tracing::error!("Failed while running text mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.clipboard {
            if let Err(e) = popup_dictionary::clipboard(config) {
                tracing::error!("Failed while running clipboard mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.watch {
            if let Err(e) = popup_dictionary::watch(config) {
                tracing::error!("Failed while running watch mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        } else if let Some(ocr_path) = cli.modes.ocr {
            match get_image_for_ocr(ocr_path) {
                Ok(image) => {
                    if let Err(e) = popup_dictionary::ocr(image, config) {
                        tracing::error!("Failed while running ocr mode due to error: {e}");
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Could not find path or image data to run OCR mode with error: {e}"
                    );
                    return ExitCode::FAILURE;
                }
            }
        } else {
            tracing::info!("No mode specified. Defaulting to watch mode with tray icon.");
            // Default to watch mode with tray icon if no mode set
            if !config.show_tray_icon {
                crate::tray::spawn_tray_icon();
            }
            if let Err(e) = popup_dictionary::watch(config) {
                tracing::error!("Failed while running watch mode due to error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}

fn init_logging(verbose: bool, log_file: Option<Option<PathBuf>>) {
    let default_filter = if cfg!(debug_assertions) {
        "debug"
    } else if verbose {
        "info"
    } else {
        "warn"
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let terminal_logger = tracing_subscriber::fmt::layer()
        .with_target(cfg!(debug_assertions))
        .with_writer(std::io::stderr);

    let mut log_file_error: Option<String> = None;

    let file_logger = if let Some(log_path) = log_file {
        let result: Result<std::fs::File, String> = (|| {
            let path = match log_path {
                Some(p) if p.is_dir() => p.join("log.txt"),
                Some(p) => p,
                None => {
                    let base = match dirs::data_dir() {
                        Some(path) => path,
                        None => Err("No valid data path found in environment variables.")?,
                    };
                    base.join("popup_dictionary").join("log.txt")
                }
            };

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Log directory could not be created with error: {e}"))?;
            }

            std::fs::File::create(&path)
                .map_err(|e| format!("Log file could not be created with error: {e}"))
        })();

        match result {
            Ok(file) => Some(
                tracing_subscriber::fmt::layer()
                    .with_ansi(false)
                    .with_writer(std::sync::Arc::new(file))
                    .with_filter(EnvFilter::new("trace")),
            ),
            Err(e) => {
                log_file_error = Some(e);
                None
            }
        }
    } else {
        None
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(terminal_logger)
        .with(file_logger)
        .init();

    tracing::debug!("Logger initialized.");

    if let Some(e) = log_file_error {
        tracing::warn!("Log file unavailable due to error: {e}");
    }
}

fn get_image_for_ocr(ocr_arg: Option<PathBuf>) -> Result<DynamicImage, Box<dyn std::error::Error>> {
    match ocr_arg {
        Some(path) => Ok(image::open(path)?),
        None => {
            let mut buffer = Vec::new();
            std::io::stdin().read_to_end(&mut buffer)?;

            let img: DynamicImage = ImageReader::new(Cursor::new(buffer))
                .with_guessed_format()?
                .decode()?;

            Ok(img)
        }
    }
}
