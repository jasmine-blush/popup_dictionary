#[cfg(target_os = "linux")]
use arboard::GetExtLinux;

use arboard::Clipboard;
use image::DynamicImage;
use image::GenericImageView;
use image::ImageBuffer;
use image::ImageReader;
use image::Rgba;
use regex::Regex;
use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use crate::app::run_app;
use crate::manga_ocr::MangaOcr;
use crate::tesseract::{check_tesseract, ocr_image};

pub mod app;
mod font_helper;
mod manga_ocr;
mod plugin;
mod plugins;
mod tesseract;
mod window_helper;

fn open_app(
    sentence: &str,
    config: app::Config,
    new_sentence_mutex: Option<Arc<Mutex<Option<String>>>>,
    paused: Option<Arc<AtomicBool>>,
    do_paste: Option<Arc<AtomicBool>>,
) -> Result<(), Box<dyn Error>> {
    let valid_sentence: String = validate_sentence(&sentence)?;

    tracing::info!("Input looks good. Launching dictionary app.");
    run_app(
        &valid_sentence,
        config,
        new_sentence_mutex,
        paused,
        do_paste,
    )?;

    Ok(())
}

fn validate_sentence(sentence: &str) -> Result<String, Box<dyn Error>> {
    let sentence: String = sentence.chars().filter(|c| !c.is_whitespace()).collect();

    if sentence.is_empty() {
        return Err(Box::from("Input text must be at least one character."));
    }

    if !contains_japanese(&sentence) {
        return Err(Box::from("Input text must contain japanese text."));
    }

    return Ok(sentence);
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

pub fn run(sentence: &str, config: app::Config) -> Result<(), Box<dyn Error>> {
    open_app(&sentence, config, None, None, None)?;

    Ok(())
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

fn decode_clipboard_image(image: arboard::ImageData) -> Option<DynamicImage> {
    match ImageReader::new(Cursor::new(image.bytes.as_ref())).with_guessed_format() {
        Ok(reader) => match reader.decode() {
            Ok(dynamic_image) => {
                return Some(dynamic_image);
            }
            Err(e) => {
                tracing::debug!("Could not decode image data due to error: {e}");
            }
        },
        Err(e) => {
            tracing::warn!("Could not read image data due to error: {e}");
        }
    }

    tracing::debug!("Trying to parse image data as raw pixel buffer instead.");
    ImageBuffer::<Rgba<u8>, _>::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )
    .map(DynamicImage::ImageRgba8)
}

pub fn watch(
    config: app::Config,
    paused: Arc<AtomicBool>,
    ocr_model: Arc<AtomicUsize>,
    do_paste: Arc<AtomicBool>,
    keep_open: bool,
) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run watch mode.");

    if keep_open {
        tracing::info!("Keep-open mode enabled. Launching clipboard watcher in background.");

        let new_sentence_channel: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let channel_for_thread = Arc::clone(&new_sentence_channel);

        let (first_sender, first_receiver) = std::sync::mpsc::channel::<String>();

        let app_is_running = Arc::new(AtomicBool::new(false));
        let app_is_running_clone = Arc::clone(&app_is_running);

        let paused_clone = Arc::clone(&paused);
        let ocr_model_clone = Arc::clone(&ocr_model);
        let do_paste_clone = Arc::clone(&do_paste);

        std::thread::spawn(move || {
            let mut clipboard: Clipboard = match Clipboard::new() {
                Ok(clip) => clip,
                Err(e) => {
                    tracing::error!("Could not create clipboard due to error: {e}.");
                    return;
                }
            };
            let mut initial_content: ClipboardContent = get_clipboard_content(&mut clipboard);

            let mut manga_ocr: Option<MangaOcr> = None;

            tracing::info!("Watching...");
            let mut was_paused = false;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));

                if do_paste_clone.load(Ordering::Relaxed) {
                    initial_content = ClipboardContent {
                        image: None,
                        text: None,
                    };
                    do_paste_clone.store(false, Ordering::Relaxed);
                } else {
                    if paused_clone.load(Ordering::Relaxed) {
                        was_paused = true;
                        continue;
                    }
                    if was_paused {
                        // Replace initial_content with current here to prevent acting on clipboard content
                        // that was copied while paused.
                        initial_content = get_clipboard_content(&mut clipboard);
                        was_paused = false;
                    }
                }

                let current_content: ClipboardContent = get_clipboard_content(&mut clipboard);
                if !clipboard_content_differs(&initial_content, &current_content) {
                    continue;
                }

                tracing::info!("New clipboard content detected.");

                let maybe_sentence: Option<String> = if let Some(image) = current_content.image {
                    tracing::debug!("Found image data in main clipboard.");

                    let ocr_idx = ocr_model_clone.load(Ordering::Relaxed);
                    match decode_clipboard_image(image) {
                        Some(dynamic_image) => {
                            match ocr_to_sentence(dynamic_image, ocr_idx, &mut manga_ocr) {
                                Ok(sentence) => Some(sentence),
                                Err(e) => {
                                    tracing::warn!("OCR failed with error: {e}.");
                                    None
                                }
                            }
                        }
                        None => {
                            tracing::warn!("Could not decode image data in clipboard.");
                            None
                        }
                    }
                } else if let Some(sentence) = current_content.text {
                    tracing::debug!("Found text in main clipboard.");

                    Some(sentence)
                } else {
                    None
                };

                if let Some(sentence) = maybe_sentence {
                    if !app_is_running_clone.load(Ordering::SeqCst) {
                        if first_sender.send(sentence).is_err() {
                            tracing::info!("Could not send sentence to main thread.");
                            return;
                        }
                    } else {
                        match validate_sentence(&sentence) {
                            Ok(valid_sentence) => {
                                *channel_for_thread.lock().unwrap() = Some(valid_sentence);
                            }
                            Err(e) => {
                                tracing::warn!("Not updating sentence due to error: {e}");
                            }
                        };
                    }
                }

                initial_content = get_clipboard_content(&mut clipboard);
            }
        });

        loop {
            match first_receiver.recv() {
                Ok(first_sentence) => {
                    tracing::info!("Opening window with first sentence.");

                    app_is_running.store(true, Ordering::SeqCst);

                    if let Err(e) = open_app(
                        &first_sentence,
                        config.clone(),
                        Some(Arc::clone(&new_sentence_channel)),
                        Some(Arc::clone(&paused)),
                        Some(Arc::clone(&do_paste)),
                    ) {
                        tracing::warn!(
                            "Failed while running in keep-open watch mode due to error: {e}"
                        );
                    }

                    app_is_running.store(false, Ordering::SeqCst);

                    if let Ok(mut lock) = new_sentence_channel.lock() {
                        *lock = None;
                    }

                    tracing::info!("Window closed, continuing to watch.");
                }
                Err(e) => {
                    return Err(Box::from(format!(
                        "Keep-open watcher thread exited before finding any valid content: {e}"
                    )));
                }
            }
        }
    } else {
        let mut clipboard: Clipboard = Clipboard::new()?;
        let mut initial_content: ClipboardContent = get_clipboard_content(&mut clipboard);

        let mut manga_ocr: Option<MangaOcr> = None;

        tracing::info!("Watching...");
        let mut was_paused = false;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));

            if do_paste.load(Ordering::Relaxed) {
                initial_content = ClipboardContent {
                    image: None,
                    text: None,
                };
                do_paste.store(false, Ordering::Relaxed);
            } else {
                if paused.load(Ordering::Relaxed) {
                    was_paused = true;
                    continue;
                }
                if was_paused {
                    // Replace initial_content with current here to prevent acting on clipboard content
                    // that was copied while paused.
                    initial_content = get_clipboard_content(&mut clipboard);
                    was_paused = false;
                }
            }

            let current_content: ClipboardContent = get_clipboard_content(&mut clipboard);
            if clipboard_content_differs(&initial_content, &current_content) {
                tracing::info!("New clipboard content detected.");

                if let Some(image) = current_content.image {
                    tracing::debug!("Found image data in main clipboard.");

                    match decode_clipboard_image(image) {
                        Some(dynamic_image) => {
                            if let Err(e) = ocr(
                                dynamic_image,
                                config.clone(),
                                ocr_model.load(Ordering::Relaxed),
                                &mut manga_ocr,
                            ) {
                                tracing::warn!(
                                    "Failed while running OCR mode in watch mode due to error: {e}"
                                );
                            }
                        }
                        None => {
                            tracing::warn!("Could not decode image data in clipboard.");
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

fn ocr_to_sentence(
    image: DynamicImage,
    ocr_model: usize,
    manga_ocr: &mut Option<MangaOcr>,
) -> Result<String, Box<dyn Error>> {
    let sentence = if ocr_model == 0 {
        tracing::info!("Using Tesseract.");

        // Tesseract
        let tess_command: String = match check_tesseract() {
            Ok(command) => command,
            Err(e) => {
                return Err(Box::from(format!("Could not find Tesseract: {e}")));
            }
        };

        // scale image so the smaller dimension (w/h) is at least 100px, don't scale more than 4x
        // (somewhat arbitrary number) as that reduces accuracy again
        let (width, height) = image.dimensions();
        let scaling = (((100.0 / (width.min(height) as f32)) as u32) + 1).min(4);
        let image = image.resize(
            width * scaling,
            height * scaling,
            image::imageops::FilterType::Nearest,
        );

        let mut image_data = Vec::new();
        image.write_to(
            &mut std::io::Cursor::new(&mut image_data),
            image::ImageFormat::Png,
        )?;

        ocr_image(&tess_command, &image_data)?
    } else if ocr_model == 1 {
        tracing::info!("Using MangaOCR.");

        // MangaOCR
        if manga_ocr.is_none() {
            *manga_ocr = Some(crate::manga_ocr::MangaOcr::new()?);
        }
        manga_ocr
            .as_mut()
            .expect("MangaOCR model could not be created.")
            .ocr_image(&image)?
    } else {
        tracing::error!("Invalid OCR engine {} is selected.", ocr_model);
        String::new()
    };

    Ok(sentence)
}

pub fn ocr(
    image: DynamicImage,
    config: app::Config,
    ocr_model: usize,
    manga_ocr: &mut Option<MangaOcr>,
) -> Result<(), Box<dyn Error>> {
    tracing::info!("Attempting to run OCR mode.");

    let sentence = ocr_to_sentence(image, ocr_model, manga_ocr)?;

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
