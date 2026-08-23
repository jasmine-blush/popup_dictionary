use std::error::Error;

use reqwest::blocking::Client;
use serde_json::json;
use tracing::{debug, warn};

use crate::{
    plugin::Token,
    plugins::generic_plugin::generic_helper::{GenericCategory, GenericDefinition, GenericWord},
};

pub fn add_note(
    sentence: &str,
    surface: &str,
    definition: &GenericDefinition,
) -> Result<(), Box<dyn Error>> {
    let mut sentence = sentence.replace(surface, &format!("<b>{}</b>", surface));

    let word = definition.get_word();

    let mut meanings = String::new();
    let mut counter: usize = 0;
    let mut prev_categories: String = String::new();
    for meaning in definition.get_meanings() {
        // Display tags only if they aren't the same as previous meanings
        let categories: String = meaning.get_categories().join("");
        if categories != prev_categories {
            prev_categories = categories.to_owned();

            let mut tags: String = String::from("<div>");
            if !meanings.is_empty() {
                tags.insert_str(0, "</ol></div>");
            }
            for category in meaning.get_categories() {
                tags.push_str(&format!("
<span data-sc-code=\"{}\" style=\"font-weight: bold; font-size: 0.8em; color: white; background-color: rgb(86, 86, 86); vertical-align: text-bottom; border-radius: 0.3em; margin-right: 0.25em; padding: 0.2em 0.3em; word-break: keep-all; cursor: help;\" title=\"{}\">{}</span>"
                    , category, GenericCategory::get_info(category), category));
            }
            meanings.push_str(&format!("{}<ol>", tags));
        }
        if counter == 0 {
            counter = 1;
        }

        let mut list_for_multiple_glosses = String::new();
        if meaning.get_glosses().len() < 2 {
            list_for_multiple_glosses
                .push_str("style=\"padding-left: 0px; list-style-type: none;\"");
        }
        let circled_counter = char::from_u32(0x2460 + (counter - 1) as u32).unwrap_or('\u{2460}');
        let mut glosses: String = String::from(&format!(
            "
<li style=\"padding-left: 0.25em; list-style-type: &quot;{}&quot;;\">
    <ul data-sc-content=\"glossary\" {}>",
            circled_counter, list_for_multiple_glosses
        ));
        for gloss in meaning.get_glosses() {
            glosses.push_str(&format!("<li>{}</li>", gloss));
        }
        glosses.push_str("</ul>");

        if let Some(infos) = meaning.get_infos() {
            if !infos.is_empty() {
                glosses.push_str(&format!(
                    "
<div data-sc-content=\"extra-info\" style=\"margin-left: 0.5em;\">
    <div data-sc-content=\"sense-note\" style=\"background-color: color-mix(in srgb, goldenrod 5%, transparent); border-color: goldenrod; border-style: none none none solid; border-radius: 0.4rem; border-width: calc(3em / var(--font-size-no-units, 14)); margin-top: 0.5rem; margin-bottom: 0.5rem; padding: 0.5rem;\">
        <div style=\"font-style: italic; font-size: 0.8em; color: rgb(119, 119, 119);\">Note</div>
        <div style=\"margin-left: 0.5rem;\">{}</div>
    </div>
</div>", infos.join("; ")
                ));
            }
        }

        glosses.push_str("</li>");

        meanings.push_str(&glosses);

        counter += 1;
    }
    if !meanings.is_empty() {
        meanings.push_str("</ol></div>");
    }

    let primary_definition = format!(
        "
<ol>
    <li data-details=\"{}\">
        <span class=\"dict-group__tag-list\">
            <span class=\"dict-group__tag dict-group__tag--name\">
                <span class=\"dict-group__tag-inner\">★</span>
            </span>
            <span class=\"dict-group__tag dict-group__tag--dict\">
                <span class=\"dict-group__tag-inner\">{}</span>
            </span>
        </span>
        <span class=\"dict-group__glossary\">
            <span>
                {}
            </span>
        </span>
    </li>
</ol>
",
        "Kihon", "Kihon", meanings
    );

    let frequencies = if let Some(frequencies) = definition.get_frequencies() {
        let str = frequencies
            .iter()
            .map(|f| {
                format!(
                    "
<div class=\"frequencies__group\" data-details=\"{}\">
    <div class=\"frequencies__number\">
        <span class=\"frequencies__number-inner\">
            {}
        </span>
    </div>
    <div class=\"frequencies__dictionary\">
        <span class=\"frequencies__dictionary-inner\">
            {}
        </span>
    </div>
</div>",
                    f.source, f.rank, f.source
                )
            })
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
                    "PAOverride": "-1",
                    "PrimaryDefinition": primary_definition,
                    "Sentence": sentence,
                    "FrequenciesStylized": frequencies,
                    "FrequencySort": frequency_sort,
                    "PASilence": "[sound:_silence.wav]",
                    "WordReadingHiragana": word.get_kana(),
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
                furigana_string.push_str(&format!("{}", furigana.base));
                if furigana.base != furigana.reading {
                    furigana_string.push_str(&format!("[{}]", furigana.reading));
                }
            }
            return furigana_string;
        }
    }
    word.get_kana().to_owned()
}
