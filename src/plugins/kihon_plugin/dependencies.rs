use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs::File;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use tar::Archive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

const JUMANDIC_URL: &str =
    "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/jumandic-mecab-7_0.tar.xz";
const JUMANDIC_HASH: &str = "5816204d559bb5e24eec5c5f24211394b19daf56d31a54d3333f309ffac5b5b5";

const JMDICT_SIMPLIFIED_URL: &str = "https://github.com/scriptin/jmdict-simplified/releases/download/3.6.2%2B20260202123847/jmdict-eng-3.6.2+20260202123847.json.tgz";
const JMDICT_SIMPLIFIED_HASH: &str =
    "7a8b282f8ec20a616606da81c64cf3da3b0d05260767af9a8cf20cc0230fd177";

const LEEDS_FREQUENCIES_URL: &str = "https://github.com/hingston/japanese/raw/78a5f64e872e4a2ad430adfd124c98f5f0a1619b/44492-japanese-words-latin-lines-removed.txt";
const LEEDS_FREQUENCIES_HASH: &str =
    "770d95b7b79451614d73bcb0625555888797b76970420af5f3dd66b1767acd83";

const BCCWJ_COMBINED_URL: &str = "https://github.com/Kuuuube/yomitan-dictionaries/raw/d6fde809e3f26eb5aed6d41896f332179044998c/dictionaries/BCCWJ_SUW_LUW_combined.zip";
const BCCWJ_COMBINED_HASH: &str =
    "e2315f451b4348db830187f1641355fd81f7944ab649e9d8ead62e5d9c7e27a2";

const JMDICT_FURIGANA_URL: &str = "https://github.com/Doublevil/JmdictFurigana/releases/download/2.3.1%2B2026-01-25/JmdictFurigana.json";
const JMDICT_FURIGANA_HASH: &str =
    "fb0d0deca666e68acaf65a0cc1e278605d1b8391ff4e491f9728d934aafb5b69";

const MANGA_OCR_ENCODER_URL: &str = "https://huggingface.co/mayocream/manga-ocr-onnx/resolve/b96b1b61dc24a8f5e6dd858a83966eaa367a8519/encoder_model.onnx";
const MANGA_OCR_ENCODER_HASH: &str =
    "15fa8155fe9bc1a7d25d9bb353debaa4def033d0174e907dbd2dd6d995def85f";

const MANGA_OCR_DECODER_URL: &str = "https://huggingface.co/mayocream/manga-ocr-onnx/resolve/b96b1b61dc24a8f5e6dd858a83966eaa367a8519/decoder_model.onnx";
const MANGA_OCR_DECODER_HASH: &str =
    "ef7765261e9d1cdc34d89356986c2bbc2a082897f753a89605ae80fdfa61f5e8";

const MANGA_OCR_VOCAB_URL: &str = "https://huggingface.co/mayocream/manga-ocr-onnx/resolve/b96b1b61dc24a8f5e6dd858a83966eaa367a8519/vocab.txt";
const MANGA_OCR_VOCAB_HASH: &str =
    "5cb5c5586d98a2f331d9f8828e4586479b0611bfba5d8c3b6dadffc84d6a36a3";

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

            if let Err(e) = verify_file_hash(destination_path, JUMANDIC_HASH) {
                try_remove_file(destination_path.clone());
                return Err(Box::from(e));
            }

            return Ok(());
        }
    }

    Err(Box::from("No system dictionary found in archive"))
}

pub fn get_jmdict_simplified() -> Result<String, Box<dyn Error>> {
    let response = reqwest::blocking::get(JMDICT_SIMPLIFIED_URL)?;

    let gz_decoder = GzDecoder::new(response);
    let mut archive = Archive::new(gz_decoder);

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let path = entry.path()?;

        if path.ends_with("jmdict-eng-3.6.2.json") {
            let mut buffer = Vec::new();

            entry.read_to_end(&mut buffer)?;

            verify_buf_hash(&buffer, JMDICT_SIMPLIFIED_HASH)?;

            let content =
                String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 sequence: {}", e))?;

            return Ok(content);
        }
    }

    Err(Box::from("No JSON file found in .tgz archive"))
}

pub fn get_bccwj_combined() -> Result<String, Box<dyn Error>> {
    let response = reqwest::blocking::get(BCCWJ_COMBINED_URL)?.bytes()?;

    let zip_reader = Cursor::new(response);
    let mut archive = ZipArchive::new(zip_reader)?;

    if let Ok(mut file) = archive.by_name("term_meta_bank_1.json") {
        let mut buffer = Vec::new();

        file.read_to_end(&mut buffer)?;

        verify_buf_hash(&buffer, BCCWJ_COMBINED_HASH)?;

        let content =
            String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 sequence: {}", e))?;

        return Ok(content);
    }

    Err(Box::from("No JSON file found in .zip archive"))
}

pub fn get_leeds_frequencies() -> Result<String, Box<dyn Error>> {
    fetch_string(LEEDS_FREQUENCIES_URL, LEEDS_FREQUENCIES_HASH)
}

pub fn get_jmdict_furigana() -> Result<String, Box<dyn Error>> {
    fetch_string(JMDICT_FURIGANA_URL, JMDICT_FURIGANA_HASH)
}

pub fn fetch_manga_ocr_encoder(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    fetch_file(
        destination_path,
        MANGA_OCR_ENCODER_URL,
        MANGA_OCR_ENCODER_HASH,
    )?;

    Ok(())
}

pub fn fetch_manga_ocr_decoder(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    fetch_file(
        destination_path,
        MANGA_OCR_DECODER_URL,
        MANGA_OCR_DECODER_HASH,
    )?;

    Ok(())
}

pub fn fetch_manga_ocr_vocab(destination_path: &PathBuf) -> Result<(), Box<dyn Error>> {
    fetch_file(destination_path, MANGA_OCR_VOCAB_URL, MANGA_OCR_VOCAB_HASH)?;

    Ok(())
}

fn try_remove_file(path: PathBuf) {
    if let Err(e) = std::fs::remove_file(&path) {
        tracing::warn!("Could not cleanup {} due to error: {e}", path.display());
    } else {
        tracing::info!("Cleaned up {}.", path.display());
    }
}

fn fetch_file(
    destination_path: &PathBuf,
    url: &str,
    expected_hex: &str,
) -> Result<(), Box<dyn Error>> {
    let mut response = reqwest::blocking::get(url)?;

    if let Some(parent) = destination_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out_file = File::create(destination_path)?;

    io::copy(&mut response, &mut out_file)?;

    if let Err(e) = verify_file_hash(destination_path, expected_hex) {
        try_remove_file(destination_path.clone());
        return Err(Box::from(e));
    }

    Ok(())
}

fn fetch_string(url: &str, expected_hex: &str) -> Result<String, Box<dyn Error>> {
    let mut response = reqwest::blocking::get(url)?;

    let mut buffer = Vec::new();

    response.read_to_end(&mut buffer)?;

    verify_buf_hash(&buffer, expected_hex)?;

    let content =
        String::from_utf8(buffer).map_err(|e| format!("Invalid UTF-8 sequence: {}", e))?;

    return Ok(content);
}

fn verify_file_hash(path: &Path, expected_hex: &str) -> Result<(), Box<dyn Error>> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();

    let mut buffer = [0u8; 8192]; // don't read entire file into RAM at once
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let actual_hex = hex::encode(hasher.finalize());

    if actual_hex != expected_hex {
        return Err(format!(
            "Hash mismatch for {:?}. Expected {}, actual {}",
            path, expected_hex, actual_hex
        )
        .into());
    }
    Ok(())
}

fn verify_buf_hash(buffer: &Vec<u8>, expected_hex: &str) -> Result<(), Box<dyn Error>> {
    let mut hasher = Sha256::new();

    hasher.update(buffer);

    let actual_hex = hex::encode(hasher.finalize());

    if actual_hex != expected_hex {
        return Err(format!(
            "Hash mismatch for buffer. Expected {}, actual {}",
            expected_hex, actual_hex
        )
        .into());
    }
    Ok(())
}
