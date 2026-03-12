use flate2::read::GzDecoder;
use std::error::Error;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use tar::Archive;
use xz2::read::XzDecoder;

const JUMANDIC_URL: &str =
    "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/jumandic-mecab-7_0.tar.xz";
const JMDICT_SIMPLIFIED_URL: &str = "https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20260202123847/jmdict-eng-3.6.2+20260202123847.json.tgz";
const LEEDS_FREQUENCIES_URL: &str = "https://github.com/hingston/japanese/blob/78a5f64e872e4a2ad430adfd124c98f5f0a1619b/44492-japanese-words-latin-lines-removed.txt";
const JMDICT_FURIGANA_URL: &str = "https://github.com/Doublevil/JmdictFurigana/releases/download/2.3.1%2B2026-01-25/JmdictFurigana.json";

pub fn fetch_jumandic(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let response = reqwest::blocking::get(JUMANDIC_URL)?;

    let xz_decoder = XzDecoder::new(response);
    let mut archive = Archive::new(xz_decoder);

    for entry_result in archive.entries()? {
        let entry = entry_result?;
        let path = entry.path()?;

        if path.ends_with("system.dic.zst") {
            let mut zstd_decoder = zstd::stream::read::Decoder::new(entry)?;

            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(destination_path)?;

            io::copy(&mut zstd_decoder, &mut out_file)?;

            return Ok(());
        }
    }

    Err(Box::from("No system dictionary found in archive"))
}

pub fn fetch_jmdict_simplified(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let response = reqwest::blocking::get(JMDICT_SIMPLIFIED_URL)?;

    let gz_decoder = GzDecoder::new(response);
    let mut archive = Archive::new(gz_decoder);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?;

        if path.ends_with("jmdict-eng-3.6.2.json") {
            if let Some(parent) = destination_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(destination_path)?;

            io::copy(&mut entry, &mut out_file)?;

            return Ok(());
        }
    }

    Err(Box::from("No JSON file found in .tgz archive"))
}

pub fn fetch_leeds_frequencies(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut response = reqwest::blocking::get(LEEDS_FREQUENCIES_URL)?;

    if let Some(parent) = destination_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out_file = File::create(destination_path)?;

    io::copy(&mut response, &mut out_file)?;

    Ok(())
}

pub fn fetch_jmdict_furigana(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let mut response = reqwest::blocking::get(JMDICT_FURIGANA_URL)?;

    if let Some(parent) = destination_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out_file = File::create(destination_path)?;

    io::copy(&mut response, &mut out_file)?;

    Ok(())
}

pub fn cleanup_files() {
    if let Some(mut data_dir_path) = dirs::data_dir() {
        data_dir_path = data_dir_path.join("popup_dictionary").join("dicts");

        let leeds_frequency_path = data_dir_path.clone().join("leeds-corpus-frequency.txt");
        try_remove_file(leeds_frequency_path);

        let jmdict_furigana_path = data_dir_path.clone().join("jmdict-furigana.json");
        try_remove_file(jmdict_furigana_path);

        let jmdict_simplified_path = data_dir_path.clone().join("jmdict-simplified.json");
        try_remove_file(jmdict_simplified_path);
    } else {
        tracing::warn!(
            "Could not cleanup files: No valid data path found in environment variables."
        );
    }
}

fn try_remove_file(path: PathBuf) {
    if let Err(e) = std::fs::remove_file(&path) {
        tracing::warn!("Could not cleanup {} due to error: {e}", path.display());
    } else {
        tracing::info!("Cleaned up {}.", path.display());
    }
}
