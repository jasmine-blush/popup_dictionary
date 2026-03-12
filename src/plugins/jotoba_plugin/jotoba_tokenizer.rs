use aho_corasick::AhoCorasick;
use aho_corasick::MatchKind;
use curl::easy::Easy;
use curl::easy::List;
use phf::phf_map;
use serde::Deserialize;
use serde::Serialize;
use std::error::Error;
use std::fmt;

use crate::plugin::Token;
use crate::plugin::Validity;

const JOTOBA_SUGGESTION_MAX: usize = 37;
const COMMON_UNKNOWNS: phf::Map<&'static str, ()> = phf_map! {
    "は" => (),
    "が" => (),
    "を" => (),
    "に" => (),
    "で" => (),
    "と" => (),
    "の" => (),
    "も" => (),
    "や" => (),
    "へ" => (),
    "か" => (),
    "よ" => (),
    "ね" => (),
    "な" => (),
    "という" => ()
};

// Structure for caching
#[derive(Clone, Debug)]
enum CachedToken {
    Valid(ValidToken),
    Invalid(String),
}

#[derive(Clone, Debug)]
struct ValidToken {
    word: String,
    response: WordsResponse,
}

impl fmt::Display for CachedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            CachedToken::Valid(cached_word) => write!(f, "{}", cached_word.word),
            CachedToken::Invalid(cached_word) => write!(f, "{}", cached_word),
        }
    }
}

// Jotoba API Words Response
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WordsResponse {
    pub words: Vec<Word>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Word {
    pub reading: Reading,
    pub senses: Vec<Sense>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Reading {
    pub kana: String,
    #[serde(default)]
    pub kanji: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Sense {
    pub glosses: Vec<String>,
}

// Jotoba API Suggestion Response
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SuggestionResponse {
    suggestions: Vec<Suggestion>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Suggestion {
    primary: String,
    secondary: Option<String>,
}

// Easy Client
struct Client {
    words_easy: Easy,
    suggestion_easy: Easy,
}

impl Client {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut words_easy = Easy::new();
        words_easy.url("https://jotoba.de/api/search/words")?;
        words_easy.post(true)?;
        let mut list = List::new();
        list.append("Content-Type: application/json")?;
        words_easy.http_headers(list)?;

        let mut suggestion_easy = Easy::new();
        suggestion_easy.url("https://jotoba.de/api/suggestion")?;
        suggestion_easy.post(true)?;
        let mut list = List::new();
        list.append("Content-Type: application/json")?;
        suggestion_easy.http_headers(list)?;

        Ok(Client {
            words_easy,
            suggestion_easy,
        })
    }
}

pub struct JotobaTokenizer {
    token_cache: Vec<CachedToken>,
    easy_client: Client,
}

impl JotobaTokenizer {
    pub fn new() -> Self {
        return Self {
            token_cache: Vec::new(),
            easy_client: Client::new().unwrap(), // TODO: handle error
        };
    }

    pub fn tokenize(&mut self, sentence: &str) -> Result<Vec<Token>, Box<dyn Error>> {
        tracing::info!("Trying to tokenize with Jotoba.");
        tracing::debug!("Input text for Jotoba tokenization is: {}.", sentence);

        // Firstly splitting the input sentence into valid and invalid sections of text
        let mut sentence_parts = Vec::new(); // (part, is_alphabetic)
        let mut current_part = String::new();

        for c in sentence.chars() {
            if c.is_alphabetic() {
                current_part.push(c);
            } else {
                if !current_part.is_empty() {
                    sentence_parts.push((current_part.clone(), true));
                    current_part.clear();
                }
                sentence_parts.push((c.to_string(), false));
            }
        }
        if !current_part.is_empty() {
            sentence_parts.push((current_part, true));
        }

        // Then processing valid sections, adding invalid sections as "invalid" tokens.
        let mut tokenized_sentence: Vec<Token> = Vec::new();
        for (part_slice, is_alphabetic) in sentence_parts {
            tracing::trace!("Processing section with Jotoba: {}.", part_slice);

            // Non-alphabetic section
            if !is_alphabetic {
                tracing::trace!("Section is invalid.");

                tokenized_sentence.push(Token {
                    input_word: part_slice.to_owned(),
                    deinflected_word: part_slice.to_owned(),
                    conjugations: Vec::new(),
                    validity: Validity::INVALID,
                });
                continue;
            }

            tracing::trace!("Section is valid.");

            // Section is longer than maximum allowed Jotoba suggestion API request length, have to use costly manual tokenization
            if part_slice.chars().count() > JOTOBA_SUGGESTION_MAX {
                tracing::trace!("Tokenizing whole section.");

                let tokens = self.tokenize_sentence(&part_slice)?;
                tokenized_sentence.extend(tokens);
                continue;
            }

            tracing::trace!("Getting Jotoba suggestions for section.");

            // Get jotoba suggestions and build AhoCorasick left-match, do manual tokenization on non-matches
            let suggestion_response = self.query_suggestion(&part_slice)?;
            let suggestions = suggestion_response.suggestions;

            let patterns: Vec<&str> = suggestions
                .iter()
                .flat_map(|s| {
                    let mut list = vec![s.primary.as_str()];
                    if let Some(ref sec) = s.secondary {
                        list.push(sec.as_str());
                    }
                    list
                })
                .collect();

            let ac = AhoCorasick::builder()
                .match_kind(MatchKind::LeftmostLongest)
                .build(patterns)?;

            let mut last_end_byte = 0;
            for mat in ac.find_iter(&part_slice) {
                let start_byte = mat.start();
                let end_byte = mat.end();

                // non-matches inbetween matches, tokenize
                if start_byte > last_end_byte {
                    let unknown_slice = &part_slice[last_end_byte..start_byte];
                    if COMMON_UNKNOWNS.contains_key(unknown_slice) {
                        tokenized_sentence.push(Token {
                            input_word: unknown_slice.to_owned(),
                            deinflected_word: unknown_slice.to_owned(),
                            conjugations: Vec::new(),
                            validity: Validity::VALID,
                        });
                    } else {
                        tokenized_sentence
                            .extend(self.tokenize_sentence(&unknown_slice.to_string())?);
                    }
                }

                let matched_word_slice = &part_slice[start_byte..end_byte];
                tokenized_sentence.push(Token {
                    input_word: matched_word_slice.to_owned(),
                    deinflected_word: matched_word_slice.to_owned(),
                    conjugations: Vec::new(),
                    validity: Validity::VALID,
                });

                last_end_byte = end_byte;
            }

            // Do manual tokenization on non-matches at end of sentence
            if last_end_byte < part_slice.len() {
                tracing::trace!("End of section has non-matches.");

                let unknown_slice = &part_slice[last_end_byte..part_slice.len()];
                if COMMON_UNKNOWNS.contains_key(unknown_slice) {
                    tokenized_sentence.push(Token {
                        input_word: unknown_slice.to_owned(),
                        deinflected_word: unknown_slice.to_owned(),
                        conjugations: Vec::new(),
                        validity: Validity::VALID,
                    });
                } else {
                    tokenized_sentence.extend(self.tokenize_sentence(&unknown_slice.to_string())?);
                }
            }
        }

        Ok(tokenized_sentence)
    }

    fn tokenize_sentence(&mut self, sentence: &String) -> Result<Vec<Token>, Box<dyn Error>> {
        tracing::trace!("Doing manual tokenization on section: {}.", sentence);

        let mut sentence: String = sentence.to_owned();

        let mut token_cache: Vec<CachedToken> = Vec::new();
        let mut previous_word: String = String::new();
        while !sentence.is_empty() {
            let response: WordsResponse = self.query_words(&sentence)?;
            if response.words.len() > 0 {
                let mut removed: bool = false;

                for word in &response.words {
                    if let Some(kanji) = &word.reading.kanji {
                        if let Some(remainder) = sentence.strip_prefix(kanji) {
                            let remainder_owned: String = remainder.to_string();
                            sentence.clear();
                            sentence.push_str(&remainder_owned);
                            removed = true;
                            token_cache.push(CachedToken::Valid(ValidToken {
                                word: kanji.clone(),
                                response: response.clone(),
                            }));
                            break;
                        }
                    }
                }

                if !removed {
                    if let Some(remainder) = sentence.strip_prefix(&response.words[0].reading.kana)
                    {
                        let remainder_owned: String = remainder.to_string();
                        sentence.clear();
                        sentence.push_str(&remainder_owned);
                        removed = true;
                        token_cache.push(CachedToken::Valid(ValidToken {
                            word: response.words[0].reading.kana.clone(),
                            response: response.clone(),
                        }));
                    } else {
                        if let Some(first_char) = sentence.chars().next() {
                            let char_len: usize = first_char.len_utf8();
                            let first_char: String = sentence.drain(0..char_len).collect();
                            previous_word.push_str(&first_char);
                            let words_len: usize = token_cache.len();
                            if words_len > 0 {
                                if let CachedToken::Valid(parsed_word) =
                                    token_cache.get_mut(words_len - 1).unwrap()
                                {
                                    if !parsed_word.word.is_empty() {
                                        token_cache.push(CachedToken::Valid(ValidToken {
                                            word: String::new(),
                                            response: response.clone(),
                                        }));
                                    }
                                } else {
                                    token_cache.push(CachedToken::Valid(ValidToken {
                                        word: String::new(),
                                        response: response.clone(),
                                    }));
                                }
                            } else {
                                token_cache.push(CachedToken::Valid(ValidToken {
                                    word: String::new(),
                                    response: response.clone(),
                                }));
                            }
                        } else {
                            return Err(Box::from("Could not properly parse section."));
                        }
                    }
                }
                if removed && !previous_word.is_empty() {
                    let words_len: usize = token_cache.len();
                    if let CachedToken::Valid(parsed_word) =
                        token_cache.get_mut(words_len - 2).unwrap()
                    {
                        parsed_word.word = previous_word.clone();
                    } else {
                        // outdated comment (TODO: recheck logic):
                        // This can occur when jotoba gives a response to a word but the input word itself is different.
                        // For example, when a typo happens: Input word = ユーザ but the correct spelling and jotoba response is ユーザー.
                        return Err(Box::from("Logical error in previous_word."));
                    }
                    previous_word.clear();
                }
            } else {
                if let Some(first_char) = sentence.chars().next() {
                    let char_len: usize = first_char.len_utf8();
                    let first_char: String = sentence.drain(0..char_len).collect();
                    let words_len: usize = token_cache.len();
                    if words_len > 0 {
                        match token_cache.get_mut(words_len - 1).unwrap() {
                            CachedToken::Valid(last_token) => {
                                // this if prevents the problem from the comment above about e.g. typos
                                if last_token.word.is_empty() {
                                    previous_word.push_str(&first_char);
                                } else {
                                    token_cache.push(CachedToken::Invalid(first_char));
                                }
                            }
                            CachedToken::Invalid(parsed_word) => {
                                parsed_word.push_str(&first_char);
                            }
                        }
                    } else {
                        token_cache.push(CachedToken::Invalid(first_char));
                    }
                } else {
                    return Err(Box::from("No matching translation(s) found."));
                }
            }
        }

        if token_cache.is_empty() {
            return Err(Box::from("No matching translation(s) found."));
        }

        let mut tokens: Vec<Token> = Vec::new();
        for cached_token in token_cache.iter() {
            match cached_token {
                CachedToken::Valid(valid_token) => {
                    let token = Token {
                        input_word: valid_token.word.to_string(),
                        deinflected_word: valid_token.word.to_string(),
                        conjugations: Vec::new(),
                        validity: Validity::VALID,
                    };
                    tokens.push(token);
                }
                CachedToken::Invalid(invalid_token) => {
                    let token = Token {
                        input_word: invalid_token.to_string(),
                        deinflected_word: invalid_token.to_string(),
                        conjugations: Vec::new(),
                        validity: Validity::INVALID,
                    };
                    tokens.push(token);
                }
            }
        }

        self.token_cache = token_cache;

        Ok(tokens)
    }

    fn query_words(&mut self, sentence: &String) -> Result<WordsResponse, Box<dyn Error>> {
        tracing::trace!("Querying words for section: {}.", sentence);

        let easy: &mut Easy = &mut self.easy_client.words_easy;

        let mut buf: Vec<u8> = Vec::new();
        let request_string: String = format!(
            "{}{}{}",
            r#"{"query":""#, sentence, r#"","language":"English"}"#
        );
        let request: &[u8] = request_string.as_bytes();
        easy.post_fields_copy(request)?;

        {
            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        let json: WordsResponse = serde_json::from_str(String::from_utf8(buf.to_vec())?.as_str())?;

        Ok(json)
    }

    fn query_suggestion(
        &mut self,
        sentence: &String,
    ) -> Result<SuggestionResponse, Box<dyn Error>> {
        tracing::trace!("Querying suggestion for section: {}.", sentence);

        let easy: &mut Easy = &mut self.easy_client.suggestion_easy;

        let mut buf: Vec<u8> = Vec::new();

        let request_string: String = format!(
            "{}{}{}",
            r#"{"input":""#, sentence, r#"","lang":"en-US","search_type":"0"}"#
        );
        let request: &[u8] = request_string.as_bytes();
        easy.post_fields_copy(request)?;

        {
            let mut transfer = easy.transfer();
            transfer.write_function(|data| {
                buf.extend_from_slice(data);
                Ok(data.len())
            })?;
            transfer.perform()?;
        }

        let json: SuggestionResponse =
            serde_json::from_str(String::from_utf8(buf.to_vec())?.as_str())?;

        Ok(json)
    }

    pub fn get_response(&mut self, token: &Token) -> Result<WordsResponse, Box<dyn Error>> {
        tracing::trace!(
            "Retrieving token input: {}, deinflection: {}, conjugations: {}, is_valid: {}.",
            token.input_word,
            token.deinflected_word,
            token.conjugations.len(),
            token.is_valid()
        );

        let cached_token: Option<&CachedToken> =
            self.token_cache
                .iter()
                .find(|cached_token| match cached_token {
                    CachedToken::Valid(valid_token) => valid_token.word == token.input_word,
                    CachedToken::Invalid(_) => false,
                });

        match cached_token {
            Some(CachedToken::Valid(valid_token)) => Ok(valid_token.response.clone()),
            _ => {
                let response = self.query_words(&token.input_word)?;
                if !response.words.is_empty() {
                    self.token_cache.push(CachedToken::Valid(ValidToken {
                        word: token.input_word.to_string(),
                        response: response.clone(),
                    }));

                    tracing::trace!("Found!");
                    Ok(response)
                } else {
                    self.token_cache
                        .push(CachedToken::Invalid(token.input_word.to_string()));
                    Err(Box::from(format!(
                        "No matching translation(s) found for token input: {}, deinflection: {}, conjugations: {}, is_valid: {}.",
                        token.input_word,
                        token.deinflected_word,
                        token.conjugations.len(),
                        token.is_valid()
                    )))
                }
            }
        }
    }
}
