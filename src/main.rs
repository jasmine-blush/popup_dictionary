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

    /// Enable verbose logging
    #[arg(long = "verbose", help_heading = None)]
    verbose: bool,
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

    #[cfg(debug_assertions)]
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    #[cfg(not(debug_assertions))]
    if cli.options.verbose {
        env_logger::builder()
            .filter_level(log::LevelFilter::max())
            .init();
    } else {
        env_logger::init();
    }

    let config: popup_dictionary::app::Config = popup_dictionary::app::Config {
        initial_plugin: cli.options.initial_plugin,
        open_at_cursor: cli.options.open_at_cursor,
        wrapped: cli.options.wrapped,
        initial_width: cli.options.initial_width.unwrap_or(450),
        initial_height: cli.options.initial_height.unwrap_or(450),
        show_tray_icon: cli.options.show_tray_icon,
    };

    #[cfg(target_os = "linux")]
    {
        if config.show_tray_icon {
            crate::tray::spawn_tray_icon();
        }

        if let Some(text) = &cli.modes.text {
            if let Err(e) = popup_dictionary::run(&text, config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.primary {
            if let Err(e) = popup_dictionary::primary(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.secondary {
            if let Err(e) = popup_dictionary::secondary(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.clipboard {
            if let Err(e) = popup_dictionary::clipboard(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.watch {
            if let Err(e) = popup_dictionary::watch(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if let Some(ocr_path) = cli.modes.ocr {
            match get_image_for_ocr(ocr_path) {
                Ok(image) => {
                    if let Err(e) = popup_dictionary::ocr(image, config) {
                        eprintln!("Error: {e}");
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    eprintln!("Error: OCR mode requires path or image data to be provided.\n{e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            // Default to watch mode with tray icon if no mode set
            if !config.show_tray_icon {
                crate::tray::spawn_tray_icon();
            }
            if let Err(e) = popup_dictionary::watch(config) {
                eprintln!("Error: {e}");
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
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.clipboard {
            if let Err(e) = popup_dictionary::clipboard(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if cli.modes.watch {
            if let Err(e) = popup_dictionary::watch(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        } else if let Some(ocr_path) = cli.modes.ocr {
            match get_image_for_ocr(ocr_path) {
                Ok(image) => {
                    if let Err(e) = popup_dictionary::ocr(image, config) {
                        eprintln!("Error: {e}");
                        return ExitCode::FAILURE;
                    }
                }
                Err(e) => {
                    eprintln!("Error: OCR mode requires path or image data to be provided.\n{e}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            // Default to watch mode with tray icon if no mode set
            if !config.show_tray_icon {
                crate::tray::spawn_tray_icon();
            }
            if let Err(e) = popup_dictionary::watch(config) {
                eprintln!("Error: {e}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
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
