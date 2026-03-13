#[cfg(target_os = "linux")]
use arboard::GetExtLinux;

use arboard::Clipboard;
use image::DynamicImage;
use image::ImageBuffer;
use image::ImageReader;
use image::Rgba;
use regex::Regex;
use std::error::Error;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::app::run_app;
use crate::tesseract::{check_tesseract, ocr_image};

pub mod app;
mod plugin;
mod plugins;
mod tesseract;
mod window_helper;

pub fn run(sentence: &str, config: app::Config) -> Result<(), Box<dyn Error>> {
    let sentence: String = sentence.chars().filter(|c| !c.is_whitespace()).collect();

    if sentence.is_empty() {
        return Err(Box::from("Input text must be at least one character."));
    }

    if !contains_japanese(&sentence) {
        return Err(Box::from("Input text must contain japanese text."));
    }

    tracing::info!("Input looks good. Launching dictionary app.");
    run_app(&sentence, config)?;

    Ok(())
}

fn contains_japanese(text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();

    let re = RE.get_or_init(|| {
        Regex::new(concat!(
            r"[",
            r"\p{scx=Hiragana}",
            r"\p{scx=Katakana}",
            r"\p{scx=Han}", // Kanji, Hanzi, Hanja
            r"]"
        ))
        .expect("Regex compilation failed")
    });

    re.is_match(text)
}

#[cfg(target_os = "linux")]
pub fn primary(config: app::Config) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run primary mode.");

    let mut clipboard: Clipboard = Clipboard::new()?;
    let sentence: String = clipboard
        .get()
        .clipboard(arboard::LinuxClipboardKind::Primary)
        .text()?;

    tracing::debug!("Text received from primary selection.");
    run(&sentence, config)
}

#[cfg(target_os = "linux")]
pub fn secondary(config: app::Config) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run secondary mode.");

    let mut clipboard: Clipboard = Clipboard::new()?;
    let sentence: String = clipboard
        .get()
        .clipboard(arboard::LinuxClipboardKind::Secondary)
        .text()?;

    tracing::debug!("Text received from secondary selection.");
    run(&sentence, config)
}

pub fn clipboard(config: app::Config) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run clipboard mode.");

    let mut clipboard: Clipboard = Clipboard::new()?;
    let sentence: String = clipboard.get().text()?;

    tracing::debug!("Text received from main clipboard.");
    run(&sentence, config)
}

/*
pub fn copy(initial_plugin: &Option<String>) -> Result<(), Box<dyn Error>> {
    // send Ctrl+C (twice as workaround for not always registering)
    let mut enigo: Enigo = Enigo::new(&enigo::Settings::default())?;
    enigo.set_delay(100);
    enigo.key(enigo::Key::Control, enigo::Direction::Press)?;
    enigo.key(enigo::Key::Unicode('c'), enigo::Direction::Click)?;
    std::thread::sleep(core::time::Duration::from_millis(100));
    enigo.key(enigo::Key::Control, enigo::Direction::Release)?;
    std::thread::sleep(core::time::Duration::from_millis(100));
    enigo.key(enigo::Key::Control, enigo::Direction::Press)?;
    enigo.key(enigo::Key::Unicode('c'), enigo::Direction::Click)?;
    std::thread::sleep(core::time::Duration::from_millis(100));
    enigo.key(enigo::Key::Control, enigo::Direction::Release)?;
    std::thread::sleep(core::time::Duration::from_millis(100));

    clipboard(initial_plugin)
}

*/

struct ClipboardContent {
    image: Option<arboard::ImageData<'static>>,
    text: Option<String>,
}

pub fn watch(config: app::Config) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run watch mode.");

    let mut clipboard: Clipboard = Clipboard::new()?;
    let mut initial_content: ClipboardContent = get_clipboard_content(&mut clipboard);

    tracing::info!("Watching...");
    loop {
        std::thread::sleep(std::time::Duration::from_millis(200));
        let current_content: ClipboardContent = get_clipboard_content(&mut clipboard);
        if clipboard_content_differs(&initial_content, &current_content) {
            tracing::info!("New clipboard content detected.");

            if let Some(image) = current_content.image {
                tracing::debug!("Found image data in main clipboard.");

                let image_data = image.clone();
                let mut success: bool = false;
                match ImageReader::new(Cursor::new(image_data.bytes)).with_guessed_format() {
                    Ok(data) => match data.decode() {
                        Ok(dynamic_image) => {
                            success = true;
                            if let Err(e) = ocr(dynamic_image, config.clone()) {
                                tracing::warn!(
                                    "Failed while running OCR mode in watch mode due to error: {e}"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Could not decode image data due to error: {e}");
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Could not read image data due to error: {e}");
                    }
                };

                if !success {
                    tracing::debug!("Trying to parse image data as raw pixel buffer instead.");

                    let image_data = image.clone();
                    if let Some(buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(
                        image_data.width as u32,
                        image_data.height as u32,
                        image_data.bytes.into_owned(),
                    ) {
                        let dynamic_image = DynamicImage::ImageRgba8(buffer);
                        if let Err(e) = ocr(dynamic_image, config.clone()) {
                            tracing::warn!(
                                "Failed while running OCR mode in watch mode due to error: {e}"
                            );
                        }
                    } else {
                        tracing::debug!(
                            "Image buffer not big enough for from_raw. This is weird..."
                        );
                    }
                }
            } else if let Some(sentence) = current_content.text {
                tracing::debug!("Found text in main clipboard.");
                if let Err(e) = run(&sentence, config.clone()) {
                    tracing::warn!(
                        "Failed while running text mode in watch mode due to error: {e}"
                    );
                }
            }

            // Getting clipboard content again here instead of replacing with current_content
            // makes sure that clipboard changes while app was running aren't acted on
            initial_content = get_clipboard_content(&mut clipboard);
        }
    }

    Ok(())
}

fn get_clipboard_content(clipboard: &mut Clipboard) -> ClipboardContent {
    ClipboardContent {
        image: if let Ok(image) = clipboard.get_image() {
            Some(image)
        } else {
            None
        },
        text: if let Ok(text) = clipboard.get_text() {
            Some(text)
        } else {
            None
        },
    }
}

fn clipboard_content_differs(first: &ClipboardContent, second: &ClipboardContent) -> bool {
    if first.image.is_some() && second.image.is_some() {
        first.image.as_ref().unwrap().bytes != second.image.as_ref().unwrap().bytes
    } else if second.image.is_some() {
        true
    } else if first.text.is_some() && second.text.is_some() {
        first.text != second.text
    } else if second.text.is_some() {
        true
    } else {
        false
    }
}

pub fn ocr(image: DynamicImage, config: app::Config) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run OCR mode.");

    let tess_command: String = match check_tesseract() {
        Ok(command) => command,
        Err(e) => {
            return Err(Box::from(format!("Could not find Tesseract: {e}")));
        }
    };

    let mut image_data = Vec::new();
    image.write_to(
        &mut std::io::Cursor::new(&mut image_data),
        image::ImageFormat::Png,
    )?;

    let sentence = ocr_image(&tess_command, &image_data)?;

    run(&sentence, config)

    /*
    let image = image.to_rgb8();
    let width: i32 = image.width() as i32;
    let height: i32 = image.height() as i32;
    const BYTES_PER_PIXEL: i32 = 3;
    let bytes_per_line: i32 = width * BYTES_PER_PIXEL;
    let image_data: &[u8] = &image.into_raw();

    let tessdata_dir: PathBuf = get_tessdata_dir();
    let tessdata_dir: &str = tessdata_dir.to_str().unwrap();
    let tess: TesseractAPI = TesseractAPI::new();

    // try horizontal ocr
    tess.init(tessdata_dir, "jpn")?;
    tess.set_image(image_data, width, height, BYTES_PER_PIXEL, bytes_per_line)?;
    let mut sentence: String = tess.get_utf8_text()?;
    let horizontal_conf: i32 = tess.mean_text_conf()?;

    // try vertical ocr
    tess.clear()?;
    tess.init(tessdata_dir, "jpn_vert")?;
    tess.set_page_seg_mode(tesseract_rs::TessPageSegMode::PSM_SINGLE_BLOCK_VERT_TEXT)?;
    tess.set_image(image_data, width, height, BYTES_PER_PIXEL, bytes_per_line)?;

    // compare confidences
    println!(
        "horz: {}, vert: {}",
        tess.mean_text_conf()?,
        horizontal_conf
    );
    if tess.mean_text_conf()? > horizontal_conf {
        sentence = tess.get_utf8_text()?;
    }

    tess.end()?;

    run(&sentence, initial_plugin)*/
}

/*
// from tesseract-rs docs
fn get_tessdata_dir() -> PathBuf {
    match std::env::var("TESSDATA_PREFIX") {
        Ok(dir) => {
            let path = PathBuf::from(dir);
            println!("Using TESSDATA_PREFIX directory: {:?}", path);
            path
        }
        Err(_) => {
            let default_dir = get_default_tessdata_dir();
            println!(
                "TESSDATA_PREFIX not set, using default directory: {:?}",
                default_dir
            );
            default_dir
        }
    }
}

// from tesseract-rs docs
fn get_default_tessdata_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(home_dir)
            .join("Library")
            .join("Application Support")
            .join("tesseract-rs")
            .join("tessdata")
    } else if cfg!(target_os = "linux") {
        let home_dir = std::env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(home_dir)
            .join(".tesseract-rs")
            .join("tessdata")
    } else if cfg!(target_os = "windows") {
        PathBuf::from(std::env::var("APPDATA").expect("APPDATA environment variable not set"))
            .join("tesseract-rs")
            .join("tessdata")
    } else {
        panic!("Unsupported operating system");
    }
}
*/
