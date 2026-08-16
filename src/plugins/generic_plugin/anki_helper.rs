use std::error::Error;

use reqwest::blocking::Client;
use serde_json::json;
use tracing::{debug, warn};

use crate::{
    plugin::Token,
    plugins::generic_plugin::generic_helper::{GenericDefinition, GenericWord},
};

pub fn add_note(
    sentence: &str,
    surface: &str,
    definition: &GenericDefinition,
) -> Result<(), Box<dyn Error>> {
    let mut sentence = sentence.replace(surface, &format!("<b>{}</b>", surface));

    let word = definition.get_word();
    let meanings = definition
        .get_meanings()
        .iter()
        .map(|m| m.get_glosses().join(", "))
        .collect::<Vec<String>>()
        .join("<br>");
    let frequencies = if let Some(frequencies) = definition.get_frequencies() {
        let str = frequencies
            .iter()
            .map(|f| format!("{}<br>{}<br>", f.rank, f.source))
            .collect::<Vec<String>>()
            .join("");

        str.trim_end_matches("<br>").to_owned()
    } else {
        "".to_owned()
    };
    let frequency_sort = if let Some(frequencies) = definition.get_frequencies() {
        if let Some(frequency) = frequencies.get(0) {
            &frequency.rank.to_string()
        } else {
            ""
        }
    } else {
        ""
    };
    let anki_payload = json!({
        "action": "addNote",
        "version": 6,
        "params": {
            "note": {
                "deckName": "JPMN-DumpM",
                "modelName": "JP Mining Note",
                "fields": {
                    "Key": word.get_surface(),
                    "Word": word.get_surface(),
                    "WordReading": get_word_furigana_string(word),
                    "PrimaryDefinition": meanings,
                    "Sentence": sentence,
                    "FrequenciesStylized": frequencies,
                    "FrequencySort": frequency_sort
                },
                "options": {
                    "allowDuplicate": true,
                },
                "tags": ["kihon"],
            },
        },
    });

    let client = Client::new();
    let response = client
        .post("http://127.0.0.1:8765")
        .json(&anki_payload)
        .send()?;

    let response_json: serde_json::Value = response.json()?;

    if let Some(error) = response_json.get("error").and_then(|e| e.as_str()) {
        warn!("Error adding card to Anki: {}", error);
    } else {
        debug!("Successfully added card! ID: {}", response_json["result"]);
    }

    Ok(())
}

// Returns the word's kana if there are no kanji, or the kanji in Anki's furigana format.
// Furigana are placed in square brackets after each section.
// A whitespace is used before a section if necessary to separate the furigana from the previous characters.
// e.g. "ごじ_開[あ]ける" (the _ indicates a whitespace in this comment)
fn get_word_furigana_string(word: &GenericWord) -> String {
    if let Some(kanji) = word.get_kanji() {
        let furigana = &kanji.1;
        if furigana.len() > 0 {
            let mut furigana_string = String::new();
            for furigana in furigana {
                if !furigana_string.is_empty() && !furigana_string.ends_with("]") {
                    furigana_string.push_str(" ");
                }
                furigana_string.push_str(&format!("{}[{}]", furigana.base, furigana.reading));
            }
            return furigana_string;
        }
    }
    word.get_kana().to_owned()
}
