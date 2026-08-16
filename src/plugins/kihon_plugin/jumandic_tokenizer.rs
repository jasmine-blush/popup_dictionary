use std::collections::HashSet;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use vibrato::dictionary::LexType;
use vibrato::{Dictionary, Tokenizer};

use crate::plugin::{Token, Validity};

const CONJ_FORMS: phf::Map<&'static str, &'static str> = phf::phf_map! {
    "*" => "*",
    "タ形" => "Past",
    "ダ列タ形" => "Past",
    "タ系連用テ形" => "Te-form",
    "ダ列タ系連用テ形" => "Te-form",
    "タ系連用タリ形" => "Tari-form",
    "命令形" => "Imperative",
    "意志形" => "Volitional",
    "基本条件形" => "", // 行けば
};

pub fn get_conjform(form: &str) -> &str {
    match CONJ_FORMS.get(form) {
        Some(description) => description,
        None => form,
    }
}

pub fn tokenize(
    query: &String,
    dictionary: &crate::plugins::kihon_plugin::jmdict_dictionary::Dictionary,
) -> Result<Vec<Token>, Box<dyn Error>> {
    let mut system_dic_path: PathBuf = match dirs::data_dir() {
        Some(path) => path,
        None => Err("No valid data path found in environment variables.")?,
    };
    system_dic_path = system_dic_path
        .join("popup_dictionary")
        .join("dicts")
        .join("system.dic");
    if !system_dic_path
        .try_exists()
        .is_ok_and(|verified| verified == true)
    {
        crate::plugins::kihon_plugin::dependencies::fetch_jumandic(&system_dic_path)?;
    }
    let system_dic: File = File::open(system_dic_path)?;
    let reader: BufReader<File> = BufReader::new(system_dic);
    let dict: Dictionary = Dictionary::read(reader)?;

    let tokenizer: Tokenizer = Tokenizer::new(dict);
    let mut worker = tokenizer.new_worker();

    worker.reset_sentence(query);
    worker.tokenize();

    let mut words: Vec<Token> = Vec::new();
    for token in worker.token_iter() {
        let validity: Validity = match token.lex_type() {
            LexType::Unknown => Validity::INVALID,
            _ => {
                if token.feature().starts_with("特殊") {
                    Validity::INVALID
                } else if token.feature().starts_with("助詞") {
                    Validity::VALID
                } else {
                    Validity::UNKNOWN
                }
            }
        };
        //println!("{:?}", token.feature());
        let conjform: String = token.feature().split(",").nth(3).unwrap_or("*").to_string();
        //println!("{:?}", conjform);
        words.push(Token {
            input_word: token.surface().to_string(),
            deinflected_word: token
                .feature()
                .split(",")
                .nth(4)
                .unwrap_or(token.surface())
                .to_string(),
            conjugations: [conjform].to_vec(),
            validity,
        });
    }

    words = improve_tokens(&mut words, dictionary);

    Ok(words)
}

fn improve_tokens(
    words: &mut Vec<Token>,
    dictionary: &crate::plugins::kihon_plugin::jmdict_dictionary::Dictionary,
) -> Vec<Token> {
    let mut new_words: Vec<Token> = Vec::new();
    let mut start_idx: usize = 0;

    while start_idx < words.len() {
        let mut found_match: bool = false;

        let is_not_particle: bool = match words[start_idx].validity {
            Validity::VALID => false,
            _ => true,
        };
        if is_not_particle {
            let word_len = if words.len() > 37 + start_idx {
                37 + start_idx
            } else {
                words.len()
            }; // 37 characters is the biggest word in the dictionary

            for end_idx in (start_idx + 1..=word_len).rev() {
                let mut base: String = words[start_idx..end_idx]
                    .iter()
                    .map(|w| w.deinflected_word.as_str())
                    .collect::<String>();
                let surface: String = words[start_idx..end_idx]
                    .iter()
                    .map(|w| w.input_word.as_str())
                    .collect::<String>();
                let end_idx_minus_one: usize = if end_idx - 1 >= start_idx {
                    end_idx - 1
                } else {
                    start_idx
                };
                let mut only_last_base: String = words[start_idx..end_idx_minus_one]
                    .iter()
                    .map(|w| w.input_word.as_str())
                    .collect::<String>();
                only_last_base.push_str(&words[end_idx_minus_one].deinflected_word);

                if let Some(_) = dictionary.lookup(&surface).expect(&format!(
                    "Error getting from database when looking up base: {}",
                    surface
                )) {
                    found_match = true;
                } else if let Some(_) = dictionary.lookup(&base).expect(&format!(
                    "Error getting from database when looking up base: {}",
                    base
                )) {
                    found_match = true;
                } else if let Some(_) = dictionary.lookup(&only_last_base).expect(&format!(
                    "Error getting from database when looking up base: {}",
                    only_last_base
                )) {
                    //println!("TRUE: {:?} {:? } {:?}", surface, base, only_last_base);
                    base = only_last_base;
                    found_match = true;
                }
                if found_match {
                    let mut seen: HashSet<String> = HashSet::new();
                    let combined_forms: Vec<String> = words[start_idx..end_idx]
                        .iter()
                        .flat_map(|word| &word.conjugations)
                        .filter(|form| seen.insert(form.to_string()))
                        .cloned()
                        .collect();
                    /*println!(
                        "{:?},{:?},{:?},{:?}",
                        surface,
                        base,
                        combined_forms,
                        words[start_idx..end_idx].to_vec()
                    );*/
                    new_words.push(Token {
                        input_word: surface,
                        deinflected_word: base,
                        conjugations: combined_forms,
                        validity: Validity::UNKNOWN,
                    });
                    start_idx = end_idx;
                    break;
                }
            }
        }

        if !found_match {
            new_words.push(words[start_idx].clone());
            start_idx += 1;
        }
    }

    new_words
}
