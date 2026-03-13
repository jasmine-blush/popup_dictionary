#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use std::{
    error::Error,
    io::Write,
    process::{Child, Command, Stdio},
};

const TESS_PATH_WINDOWS: &str = "C:\\Program Files\\Tesseract-OCR\\tesseract.exe";
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn check_tesseract() -> Result<String, Box<dyn Error>> {
    #[cfg(target_os = "linux")]
    Command::new("tesseract").arg("--version").output()?;

    // tesseract installer on Windows doesn't set PATH automatically, check default dir without PATH
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = Command::new("tesseract")
            .arg("--version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
        {
            tracing::debug!("Could not find Tesseract in PATH, checking default install dir.");

            Command::new(TESS_PATH_WINDOWS)
                .arg("--version")
                .creation_flags(CREATE_NO_WINDOW)
                .output()?;
            return Ok(String::from(
                "C:\\Program Files\\Tesseract-OCR\\tesseract.exe",
            ));
        }
    }
    return Ok(String::from("tesseract"));
}

pub fn ocr_image(tess_command: &str, image_data: &[u8]) -> Result<String, Box<dyn Error>> {
    tracing::debug!("Attempting OCR with Tesseract.");

    #[cfg(target_os = "linux")]
    let ver_conf_command = Command::new(&tess_command)
        .arg("stdin")
        .arg("stdout")
        .arg("-l")
        .arg("jpn_vert")
        .arg("--psm")
        .arg("5")
        .arg("tsv")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    #[cfg(target_os = "windows")]
    let ver_conf_command = Command::new(&tess_command)
        .arg("stdin")
        .arg("stdout")
        .arg("-l")
        .arg("jpn_vert")
        .arg("--psm")
        .arg("5")
        .arg("tsv")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()?;
    match get_conf(ver_conf_command, image_data) {
        Ok(ver_conf) => {
            #[cfg(target_os = "linux")]
            let hor_conf_command = Command::new(&tess_command)
                .arg("stdin")
                .arg("stdout")
                .arg("-l")
                .arg("jpn")
                .arg("tsv")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            #[cfg(target_os = "windows")]
            let hor_conf_command = Command::new(&tess_command)
                .arg("stdin")
                .arg("stdout")
                .arg("-l")
                .arg("jpn")
                .arg("tsv")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
            let hor_conf: f32 = get_conf(hor_conf_command, image_data)?;

            tracing::debug!(
                "Tesseract horizontal confidence: {}, vertical confidence: {}.",
                hor_conf,
                ver_conf
            );
            if hor_conf >= ver_conf {
                #[cfg(target_os = "linux")]
                let hor_command = Command::new(&tess_command)
                    .arg("stdin")
                    .arg("stdout")
                    .arg("-l")
                    .arg("jpn")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                #[cfg(target_os = "windows")]
                let hor_command = Command::new(&tess_command)
                    .arg("stdin")
                    .arg("stdout")
                    .arg("-l")
                    .arg("jpn")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()?;
                let text = get_text(hor_command, image_data)?;
                Ok(text)
            } else {
                #[cfg(target_os = "linux")]
                let ver_command = Command::new(&tess_command)
                    .arg("stdin")
                    .arg("stdout")
                    .arg("-l")
                    .arg("jpn_vert")
                    .arg("--psm")
                    .arg("5")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                #[cfg(target_os = "windows")]
                let ver_command = Command::new(&tess_command)
                    .arg("stdin")
                    .arg("stdout")
                    .arg("-l")
                    .arg("jpn_vert")
                    .arg("--psm")
                    .arg("5")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()?;
                let text = get_text(ver_command, image_data)?;
                Ok(text)
            }
        }
        Err(_) => {
            tracing::warn!(
                "Could not check for vertical text with Tesseract. Make sure you have vertical Japanese language data installed if you want vertical text support."
            );
            #[cfg(target_os = "linux")]
            let hor_command = Command::new(&tess_command)
                .arg("stdin")
                .arg("stdout")
                .arg("-l")
                .arg("jpn")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            #[cfg(target_os = "windows")]
            let hor_command = Command::new(&tess_command)
                .arg("stdin")
                .arg("stdout")
                .arg("-l")
                .arg("jpn")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
            let text = get_text(hor_command, image_data)?;
            Ok(text)
        }
    }
}

fn get_conf(mut child: Child, image_data: &[u8]) -> Result<f32, Box<dyn Error>> {
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(image_data)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(Box::from(
            "Error when trying to call tesseract for confidence.",
        ));
    }
    let tsv = String::from_utf8_lossy(&output.stdout);

    let mut total_conf: f32 = 0.0;
    let mut count: i32 = 0;

    for line in tsv.lines().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() >= 11 {
            if let Ok(conf) = fields[10].parse::<f32>() {
                if conf >= 0.0 {
                    total_conf += conf;
                    count += 1;
                }
            }
        }
    }

    Ok(total_conf / count as f32)
}

fn get_text(mut child: Child, image_data: &[u8]) -> Result<String, Box<dyn Error>> {
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(image_data)?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(Box::from("Error when trying to call tesseract for text."));
    }
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(text)
}
