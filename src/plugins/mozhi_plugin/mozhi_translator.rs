use curl::easy::Easy;
use curl::easy::List;
use serde::{Deserialize, Serialize};
use std::error::Error;

const ENGINES: phf::Map<&'static str, &'static str> = phf::phf_map! {
    "deepl" => "DeepL",
    "duckduckgo" => "DuckDuckGo",
    "google" => "GoogleTranslate",
    "mymemory" => "MyMemory",
    "reverso" => "Reverso",
    "yandex" => "Yandex",
};

#[derive(Debug)]
pub struct Translation {
    pub engine: String,
    pub translation: String,
}

// Mozhi API Translation Response
#[derive(Serialize, Deserialize, Clone, Debug)]
struct EngineTranslation {
    engine: String,
    #[serde(rename = "translated-text")]
    translated_text: String,
}

pub fn translate(sentence: &str) -> Result<Vec<Translation>, Box<dyn Error>> {
    tracing::info!("Trying to translate with Mozhi.");
    tracing::debug!("Input text for Mozhi translation is: {}.", sentence);

    let mut easy = Easy::new();
    easy.get(true)?;
    let mut list = List::new();
    list.append("Content-Type: application/json")?;
    easy.http_headers(list)?;

    let mut translations: Vec<Translation> = Vec::new();
    if let Ok(response) = query_translations(&mut easy, sentence) {
        for engine_translation in response {
            if (!engine_translation.translated_text.is_empty()) {
                translations.push(Translation {
                    engine: ENGINES
                        .get(&engine_translation.engine)
                        .unwrap_or(&engine_translation.engine.as_str())
                        .to_string(),
                    translation: engine_translation.translated_text,
                });
            }
        }
    }

    translations.sort_unstable_by_key(|translation| translation.engine.clone());

    return Ok(translations);
}

fn query_translations(
    easy: &mut Easy,
    sentence: &str,
) -> Result<Vec<EngineTranslation>, Box<dyn Error>> {
    tracing::trace!("Querying translations for sentence: {}.", sentence);

    let encoded_sentence: String = easy.url_encode(sentence.as_bytes());
    easy.url(&format!(
        "https://translate.projectsegfau.lt/api/translate?engine=all&from=ja&to=en&text={}",
        encoded_sentence
    ))?;

    let mut buf: Vec<u8> = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer.write_function(|data| {
            buf.extend_from_slice(data);
            Ok(data.len())
        })?;
        transfer.perform()?;
    }

    let json: Vec<EngineTranslation> =
        serde_json::from_str(String::from_utf8(buf.to_vec())?.as_str())?;

    Ok(json)
}
