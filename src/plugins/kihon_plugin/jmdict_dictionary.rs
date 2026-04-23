use ahash::AHashMap;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::plugin::change_progress;

#[derive(Clone)]
pub struct Dictionary {
    db: Db,
}

#[derive(bincode::Encode, bincode::Decode, Debug)]
pub struct DictionaryEntry {
    pub terms: Vec<DictionaryTerm>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct DictionaryTerm {
    pub id: String,
    pub frequency: Option<usize>,
    pub common: bool,
    pub term: String,
    pub reading: String,
    pub alt_forms: Vec<AltForm>,
    pub furigana: Option<Vec<DictionaryFurigana>>,
    pub meanings: Vec<DictionaryMeaning>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug, PartialEq, Eq)]
pub struct DictionaryFurigana {
    pub ruby: String,
    pub rt: Option<String>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug, PartialEq, Eq)]
pub struct AltForm {
    pub term: String,
    pub reading: String,
    pub furigana: Option<Vec<DictionaryFurigana>>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug, PartialEq, Eq)]
pub struct DictionaryMeaning {
    pub tags: Vec<String>,
    pub info: Vec<String>,
    pub gloss: Vec<String>,
}

// JMDict json
#[derive(Serialize, Deserialize)]
struct JMDict {
    tags: HashMap<String, String>,
    words: Vec<Word>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Word {
    id: String,
    kanji: Vec<Kanji>,
    kana: Vec<Kana>,
    sense: Vec<Sense>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Kanji {
    common: bool,
    text: String,
    tags: Vec<String>, //TODO: Handle these. These are tags that only apply to that kanji
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Kana {
    common: bool,
    text: String,
    tags: Vec<String>, //TODO: Handle these. These are tags that only apply to that kana
    applies_to_kanji: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Sense {
    part_of_speech: Vec<String>,
    misc: Vec<String>,
    info: Vec<String>,
    applies_to_kanji: Vec<String>,
    applies_to_kana: Vec<String>,
    gloss: Vec<Gloss>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Gloss {
    text: String,
}
// ---

// jmdict-furigana json
#[derive(Serialize, Deserialize, Debug)]
struct JMDictFurigana<'a> {
    text: &'a str,
    reading: &'a str,
    furigana: Vec<Furigana<'a>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Furigana<'a> {
    pub ruby: &'a str,
    pub rt: Option<&'a str>,
}
// ---

struct Dependencies {
    leeds: String,
    furigana: String,
    simplified: String,
}

const DB_VERSION_FLAG: &str = "db_version_001";

impl Dictionary {
    pub fn load_dictionary(
        path: &PathBuf,
        progress: &Arc<Mutex<String>>,
    ) -> Result<Self, Box<dyn Error>> {
        let db: Db = sled::open(path)?;

        if db.was_recovered() {
            if db.contains_key(DB_VERSION_FLAG)? {
                return Ok(Self { db });
            }
            db.clear()?;
        }

        Self::populate_database(&db, progress)?;
        Ok(Self { db })
    }

    fn populate_database<'a, 'b>(
        db: &'a Db,
        progress: &'b Arc<Mutex<String>>,
    ) -> Result<&'a Db, Box<dyn Error>> {
        tracing::info!("Trying to populate database for Kihon plugin.");
        let start: Instant = Instant::now();

        let dependencies = Self::fetch_dependencies(progress)?;
        Self::parse_jmdict_simplified(&db, dependencies, progress, start)?;
        db.insert(DB_VERSION_FLAG, "")?;
        db.flush()?;

        Ok(db)
    }

    const GENERIC_TAGS: phf::Map<&'static str, &'static str> = phf::phf_map! {
        "?" => "unclassified",
        "noun" => "noun (common) (futsuumeishi)",
        "expression" => "expression (phrases, clauses, etc.)",
        "na-adj" => "adjectival noun or quasi-adjective (keiyodoshi)",
        "no-adj" =>"noun which may take the genitive case particle 'no'",
        "i-adj" => "adjective (keiyoushi)",
        "godan" => "godan verb",
        "transitive" => "transitive verb",
        "pronoun" => "pronoun",
        "adverb" => "adverb (fukushi)",
        "to-adverb" => "adverb taking the 'to' particle",
        "suru" => "noun or participle which takes the aux. verb suru",
        "pre-noun" => "pre-noun adjectival (rentaishi)",
        "interjection" => "interjection (kandoushi)",
        "ichidan" => "ichidan verb",
        "intransitive" => "intransitive verb",
        "aux-verb" => "auxiliary verb",
        "pre-adj" => "noun or verb acting prenominally",
        "conjunction" => "conjunction",
        "particle" => "particle",
        "suffix" => "suffix",
        "taru-adj" => "'taru' adjective",
        "auxiliary" => "auxiliary",
        "copula" => "copula",
        "prefix" => "prefix",
        "kuru-verb" => "kuru verb - special class",
        "aux-adj" => "auxiliary adjective",
        "counter" => "counter",
        "numeric" => "numeric",
        "shiku-adj" => "'shiku' adjective (archaic)",
        "nidan-l" => "nidan verb (lower class) (archaic)",
        "su-verb" => "su verb - precursor to modern suru",
        "irregular" => "irregular verb",
        "ku-adj" => "'ku' adjective (archaic)",
        "nidan-u" => "nidan verb (upper class) (archaic)",
        "nidan" => "nidan verb (archaic)",
        "yodan" => "yodan verb (archaic)",
        "nari-adj" => "archaic/formal form of na-adjective",
    };

    pub fn get_tag(tag: &str) -> &str {
        match Self::GENERIC_TAGS.get(tag) {
            Some(description) => description,
            None => "unknown",
        }
    }

    fn fetch_dependencies(progress: &Arc<Mutex<String>>) -> Result<Dependencies, Box<dyn Error>> {
        change_progress(
            progress,
            "Downloading datasets [0/3]. \nThis may take a few minutes.",
        );
        // jmdict-simplified.json
        let jmdict_simplified_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading jmdict-simplified.");

            crate::plugins::kihon_plugin::dependencies::get_jmdict_simplified().unwrap()
        });

        // jmdict-furigana.json
        let jmdict_furigana_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading jmdict-furigana.");

            crate::plugins::kihon_plugin::dependencies::get_jmdict_furigana().unwrap()
        });

        // leeds-corpus-frequency.txt
        let leeds_frequency_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading leeds-corpus-frequency.");

            crate::plugins::kihon_plugin::dependencies::get_leeds_frequencies().unwrap()
        });

        change_progress(
            progress,
            "Downloading datasets [1/3]. \nThis may take a few minutes.",
        );
        let leeds_frequency = leeds_frequency_handle
            .join()
            .map_err(|e| format!("Could not download leeds-corpus-frequency: {:?}", e))?;
        tracing::debug!("leeds-corpus-frequency successfully downloaded.");
        change_progress(
            progress,
            "Downloading datasets [2/3]. \nThis may take a few minutes.",
        );
        let jmdict_furigana = jmdict_furigana_handle
            .join()
            .map_err(|e| format!("Could not download jmdict-furigan: {:?}", e))?;
        tracing::debug!("jmdict-furigana successfully downloaded.");
        change_progress(
            progress,
            "Downloading datasets [3/3]. \nThis may take a few minutes.",
        );
        let jmdict_simplified = jmdict_simplified_handle
            .join()
            .map_err(|e| format!("Could not download jmdict-simplified: {:?}", e))?;
        tracing::debug!("jmdict-simplified successfully downloaded.");

        Ok(Dependencies {
            leeds: leeds_frequency,
            furigana: jmdict_furigana,
            simplified: jmdict_simplified,
        })
    }

    fn parse_jmdict_simplified(
        db: &Db,
        dependencies: Dependencies,
        progress: &Arc<Mutex<String>>,
        start: Instant,
    ) -> Result<(), Box<dyn Error>> {
        let start_leeds: Instant = Instant::now();
        change_progress(
            &progress,
            "Parsing frequency data. \nThis may take a few minutes.",
        );
        let frequency_map: AHashMap<&str, usize> =
            Self::parse_leeds_frequencies(&dependencies.leeds)?;
        let leeds_duration = start_leeds.elapsed();
        let start_furigana: Instant = Instant::now();
        change_progress(
            &progress,
            "Parsing furigana data. \nThis may take a few minutes.",
        );
        let furigana_map: AHashMap<(&str, &str), Vec<Furigana>> =
            Self::parse_jmdict_furigana(&dependencies.furigana)?;
        let furigana_duration = start_furigana.elapsed();

        let start_simple: Instant = Instant::now();
        change_progress(
            &progress,
            "Parsing dictionary data. \nThis may take a few minutes.",
        );
        let jmdict: JMDict = serde_json::from_str(&dependencies.simplified)?;
        let simple_duration: Duration = start_simple.elapsed();
        println!(
            "Parsed in... leeds: {:.3} ms, furigana: {:.3} ms, simple: {:.3} ms",
            leeds_duration.as_secs_f64() * 1000.0,
            furigana_duration.as_secs_f64() * 1000.0,
            simple_duration.as_secs_f64() * 1000.0
        );
        let duration: Duration = start.elapsed();
        println!(
            "Fetched and parsed all in: {:.3} ms",
            duration.as_secs_f64() * 1000.0
        );

        let start_db: Instant = Instant::now();
        change_progress(
            &progress,
            "Generating dictionary database. \nThis may take a few minutes.",
        );

        let mut insert_durations: f64 = 0.0;
        let wildcard: String = String::from("*");
        for word in &jmdict.words {
            let mut entries: Vec<(String, DictionaryTerm)> = Vec::new();

            let current_id: String = word.id.to_string();
            if !word.kanji.is_empty() {
                for kanji in &word.kanji {
                    for kana in word.kana.iter().filter(|kana| {
                        kana.applies_to_kanji.contains(&wildcard)
                            || kana.applies_to_kanji.contains(&kanji.text)
                    }) {
                        let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                            &word
                                .sense
                                .iter()
                                .filter(|sense| {
                                    sense.applies_to_kanji.contains(&wildcard)
                                        || sense.applies_to_kanji.contains(&kanji.text)
                                })
                                .collect::<Vec<&Sense>>(),
                            &jmdict.tags,
                        );

                        let mut frequency = frequency_map.get(&kanji.text.as_str());
                        if frequency.is_none() {
                            frequency = frequency_map.get(&kana.text.as_str());
                        }

                        let furigana = furigana_map
                            .get(&(kanji.text.as_str(), kana.text.as_str()))
                            .map(|f| {
                                f.iter()
                                    .map(|furigana| DictionaryFurigana {
                                        ruby: furigana.ruby.to_string(),
                                        rt: furigana.rt.map(|s| s.to_string()),
                                    })
                                    .collect::<Vec<DictionaryFurigana>>()
                            });

                        entries.push((
                            format!("term:{}", kanji.text),
                            DictionaryTerm {
                                id: current_id.clone(),
                                frequency: frequency.clone().map(|f| f.clone()),
                                common: kanji.common.clone(),
                                term: kanji.text.clone(),
                                reading: kana.text.clone(),
                                alt_forms: Vec::new(),
                                furigana,
                                meanings: meanings.clone(),
                            },
                        ));

                        let mut frequency = frequency_map.get(&kana.text.as_str());
                        if frequency.is_none() {
                            frequency = frequency_map.get(&kanji.text.as_str());
                        }

                        let furigana = furigana_map
                            .get(&(kanji.text.as_str(), kana.text.as_str()))
                            .map(|f| {
                                f.iter()
                                    .map(|furigana| DictionaryFurigana {
                                        ruby: furigana.ruby.to_string(),
                                        rt: furigana.rt.map(|s| s.to_string()),
                                    })
                                    .collect::<Vec<DictionaryFurigana>>()
                            });

                        entries.push((
                            format!("reading:{}", kana.text),
                            DictionaryTerm {
                                id: current_id.clone(),
                                frequency: frequency.clone().map(|f| f.clone()),
                                common: kana.common.clone(),
                                term: kanji.text.clone(),
                                reading: kana.text.clone(),
                                alt_forms: Vec::new(),
                                furigana,

                                meanings: meanings.clone(),
                            },
                        ));
                    }
                }

                for kana in word
                    .kana
                    .iter()
                    .filter(|kana| kana.applies_to_kanji.is_empty())
                {
                    let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                        &word
                            .sense
                            .iter()
                            .filter(|sense| {
                                sense.applies_to_kana.contains(&wildcard)
                                    || sense.applies_to_kana.contains(&kana.text)
                            })
                            .collect::<Vec<&Sense>>(),
                        &jmdict.tags,
                    );

                    entries.push((
                        format!("reading:{}", kana.text),
                        DictionaryTerm {
                            id: current_id.clone(),
                            frequency: frequency_map
                                .get(&kana.text.as_str())
                                .clone()
                                .map(|f| f.clone()),
                            common: kana.common.clone(),
                            term: String::new(),
                            reading: kana.text.clone(),
                            alt_forms: Vec::new(),
                            furigana: None,
                            meanings: meanings.clone(),
                        },
                    ));
                }

                for (i, (key, entry)) in entries.iter().enumerate() {
                    let mut alt_forms: Vec<AltForm> = Vec::new();
                    for (j, (_, comp)) in entries.iter().enumerate() {
                        //TODO: also check kanji and kana tags
                        if i != j && entry.meanings == comp.meanings {
                            if entry.term != comp.term || entry.reading != comp.reading {
                                let alt_form: AltForm = AltForm {
                                    term: comp.term.clone(),
                                    reading: comp.reading.clone(),
                                    furigana: comp.furigana.clone(),
                                };
                                if !alt_forms.contains(&alt_form) {
                                    alt_forms.push(alt_form);
                                }
                            }
                        }
                    }

                    let start_insert: Instant = Instant::now();
                    Self::insert_entry(
                        db,
                        key,
                        &current_id,
                        &entry.frequency.as_ref(),
                        &entry.common,
                        &entry.term,
                        &entry.reading,
                        &alt_forms,
                        &entry.furigana,
                        &entry.meanings,
                    )?;
                    insert_durations += start_insert.elapsed().as_secs_f64();
                }
            } else {
                for kana in &word.kana {
                    let meanings: Vec<DictionaryMeaning> = Self::build_meanings(
                        &word
                            .sense
                            .iter()
                            .filter(|sense| {
                                sense.applies_to_kana.contains(&wildcard)
                                    || sense.applies_to_kana.contains(&kana.text)
                            })
                            .collect::<Vec<&Sense>>(),
                        &jmdict.tags,
                    );

                    entries.push((
                        format!("reading:{}", kana.text),
                        DictionaryTerm {
                            id: current_id.clone(),
                            frequency: frequency_map
                                .get(&kana.text.as_str())
                                .clone()
                                .map(|f| f.clone()),
                            common: kana.common.clone(),
                            term: String::new(),
                            reading: kana.text.clone(),
                            alt_forms: Vec::new(),
                            furigana: None,
                            meanings: meanings.clone(),
                        },
                    ));
                }

                for (i, (key, entry)) in entries.iter().enumerate() {
                    let mut alt_forms: Vec<AltForm> = Vec::new();
                    for (j, (_, comp)) in entries.iter().enumerate() {
                        //TODO: also check kana tags
                        if i != j && entry.meanings == comp.meanings {
                            alt_forms.push(AltForm {
                                term: comp.term.clone(),
                                reading: comp.reading.clone(),
                                furigana: comp.furigana.clone(),
                            });
                        }
                    }

                    let start_insert: Instant = Instant::now();
                    Self::insert_entry(
                        db,
                        key,
                        &current_id,
                        &entry.frequency.as_ref(),
                        &entry.common,
                        &entry.term,
                        &entry.reading,
                        &alt_forms,
                        &entry.furigana,
                        &entry.meanings,
                    )?;
                    insert_durations += start_insert.elapsed().as_secs_f64();
                }
            }
        }

        let db_duration: Duration = start_db.elapsed();
        println!(
            "Generated db in: {:.3} ms, inserts: {:.3} ms",
            db_duration.as_secs_f64() * 1000.0,
            insert_durations * 1000.0
        );

        db.flush()?;

        Ok(())
    }

    fn parse_leeds_frequencies(
        leeds_string: &str,
    ) -> Result<AHashMap<&str, usize>, Box<dyn Error>> {
        let frequency_map: AHashMap<&str, usize> = leeds_string
            .lines()
            .enumerate()
            .map(|(i, l)| (l, i))
            .collect();

        Ok(frequency_map)
    }

    fn parse_jmdict_furigana(
        furigana_string: &str,
    ) -> Result<AHashMap<(&str, &str), Vec<Furigana<'_>>>, Box<dyn Error>> {
        let start: Instant = Instant::now();

        let json_string_safe = furigana_string
            .strip_prefix('\u{FEFF}')
            .unwrap_or(&furigana_string);
        let parse: Instant = Instant::now();
        let json: Vec<JMDictFurigana> = serde_json::from_str(json_string_safe)?;
        let duration: Duration = start.elapsed();
        let since_parse: Duration = parse.elapsed();
        println!(
            "Deserialized in: {:.3} ms, Parse: {:.3} ms",
            duration.as_secs_f64() * 1000.0,
            since_parse.as_secs_f64() * 1000.0
        );

        let start: Instant = Instant::now();

        let mut furigana_map: AHashMap<(&str, &str), Vec<Furigana>> =
            AHashMap::with_capacity(json.len());
        for item in json {
            furigana_map.insert((item.text, item.reading), item.furigana);
        }
        let duration: Duration = start.elapsed();
        println!("Mapped in: {:.3} ms", duration.as_secs_f64() * 1000.0);
        println!("Entires: {}", furigana_map.len());

        Ok(furigana_map)
    }

    fn build_meanings(
        senses: &Vec<&Sense>,
        tags: &HashMap<String, String>,
    ) -> Vec<DictionaryMeaning> {
        let mut meanings: Vec<DictionaryMeaning> = Vec::new();

        for sense in senses {
            let mut meaning_tags: Vec<String> = Vec::new();
            for part in &sense.part_of_speech {
                if let Some(generic_tag) = Self::JMDICT_GENERIC_MAPPING.get(part) {
                    meaning_tags.push(generic_tag.to_string());
                } else {
                    tracing::debug!("No generic tag found for jmdict tag: {}", part);
                }
            }

            let mut info: Vec<String> = sense.info.to_vec();
            info.extend_from_slice(
                &sense
                    .misc
                    .iter()
                    .filter_map(|misc| tags.get(misc))
                    .cloned()
                    .collect::<Vec<String>>(),
            );

            let dict_meaning: DictionaryMeaning = DictionaryMeaning {
                tags: meaning_tags,
                info,
                gloss: sense
                    .gloss
                    .iter()
                    .map(|gloss| gloss.text.to_string())
                    .collect(),
            };

            meanings.push(dict_meaning);
        }

        meanings
    }

    fn insert_entry(
        db: &Db,
        key: &str,
        id: &str,
        frequency: &Option<&usize>,
        common: &bool,
        term: &str,
        reading: &str,
        alt_forms: &Vec<AltForm>,
        furigana: &Option<Vec<DictionaryFurigana>>,
        meanings: &Vec<DictionaryMeaning>,
    ) -> Result<(), Box<dyn Error>> {
        let frequency: Option<usize> = match frequency {
            Some(freq_value) => Some(**freq_value),
            None => None,
        };

        let dictionary_term: DictionaryTerm = DictionaryTerm {
            id: id.to_string(),
            frequency,
            common: *common,
            term: term.to_string(),
            reading: reading.to_string(),
            alt_forms: alt_forms.clone(),
            furigana: furigana.clone(),
            meanings: meanings.to_vec(),
        };

        if let Some(serialized_entry) = db.get(key)? {
            let (mut dictionary_entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())?;

            /*
            Sorting of terms in each entry:
            1. common, freq         -- first
            2. common, no freq
            3. uncommon, freq
            4. uncommon, no freq    -- last
            */
            //TODO: implement combining terms with the same meanings into one with "alternative readings"
            if *common {
                if let Some(frequency) = frequency {
                    let mut inserted: bool = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if !term.common || term.frequency.is_none() {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                        if let Some(term_frequency) = term.frequency {
                            if term_frequency > frequency {
                                dictionary_entry
                                    .terms
                                    .insert(index, dictionary_term.clone());
                                inserted = true;
                                break;
                            }
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                } else {
                    let mut inserted = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if !term.common {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                }
            } else {
                if let Some(frequency) = frequency {
                    let mut inserted: bool = false;
                    for (index, term) in dictionary_entry.terms.iter().enumerate() {
                        if term.common {
                            continue;
                        }
                        if !term.common && term.frequency.is_none() {
                            dictionary_entry
                                .terms
                                .insert(index, dictionary_term.clone());
                            inserted = true;
                            break;
                        }
                        if let Some(term_frequency) = term.frequency {
                            if term_frequency > frequency {
                                dictionary_entry
                                    .terms
                                    .insert(index, dictionary_term.clone());
                                inserted = true;
                                break;
                            }
                        }
                    }
                    if !inserted {
                        dictionary_entry.terms.push(dictionary_term.clone());
                    }
                } else {
                    dictionary_entry.terms.push(dictionary_term.clone());
                }
            }

            let serialized_entry: Vec<u8> =
                bincode::encode_to_vec(&dictionary_entry, bincode::config::standard())?;
            _ = db.insert(key, serialized_entry.as_slice())?;
        } else {
            let dictionary_entry = DictionaryEntry {
                terms: vec![dictionary_term],
            };
            let serialized_entry: Vec<u8> =
                bincode::encode_to_vec(&dictionary_entry, bincode::config::standard())?;

            _ = db.insert(key, serialized_entry.as_slice())?;
        }

        Ok(())
    }

    pub fn lookup(&self, word: &str) -> Result<Option<DictionaryEntry>, Box<dyn Error>> {
        if let Some(serialized_entry) = self.db.get(format!("term:{}", word))? {
            let (entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())
                    .expect(&format!("{:?}", &serialized_entry));
            return Ok(Some(entry));
        }
        if let Some(serialized_entry) = self.db.get(format!("reading:{}", word))? {
            let (entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())
                    .expect("reading");
            return Ok(Some(entry));
        }
        Ok(None)
    }

    const JMDICT_GENERIC_MAPPING: phf::Map<&'static str, &'static str> = phf::phf_map! {
        "unc" => "?",
        "n" => "noun",
        "exp" => "expression",
        "adj-na" => "na-adj",
        "adj-no" => "no-adj",
        "adj-i" => "i-adj",
        "v5u" => "godan",
        "vt" => "transitive",
        "pn" => "pronoun",
        "adv" => "adverb",
        "adv-to" => "to-adverb",
        "vs" => "suru",
        "adj-pn" => "pre-noun",
        "int" => "interjection",
        "v1" => "ichidan",
        "vi" => "intransitive",
        "v5s" => "godan",
        "v5k" => "godan",
        "v5r" => "godan",
        "v5aru" => "godan",
        "aux-v" => "aux-verb",
        "adj-f" => "pre-adj",
        "conj" => "conjunction",
        "prt" => "particle",
        "v5m" => "godan",
        "n-suf" => "suffix",
        "v5g" => "godan",
        "v5r-i" => "godan",
        "suf" => "suffix",
        "vs-i" => "suru",
        "adj-t" => "taru-adj",
        "adj-ix" => "i-adj",
        "aux" => "auxiliary",
        "cop" => "copula",
        "pref" => "prefix",
        "vk" => "kuru-verb",
        "aux-adj" => "aux-adj",
        "n-pref" => "prefix",
        "ctr" => "counter",
        "num" => "numeric",
        "vs-s" => "suru",
        "adj-shiku" => "shiku-adj",
        "v5t" => "godan",
        "v5b" => "godan",
        "v5k-s" => "godan",
        "vz" => "ichidan",
        "v2m-s" => "nidan-l",
        "vs-c" => "su-verb",
        "v1-s" => "ichidan",
        "v5n" => "godan",
        "vn" => "irregular",
        "adj-ku" => "ku-adj",
        "v2h-k" => "nidan-u",
        "v2a-s" => "nidan",
        "v4m" => "yodan",
        "v2r-k" => "nidan-u",
        "v4r" => "yodan",
        "v2r-s" => "nidan-l",
        "v5u-s" => "godan",
        "vr" => "irregular",
        "v4s" => "yodan",
        "adj-nari" => "nari-adj",
        "v4k" => "yodan",
        "v2k-s" => "nidan-l",
        "v2t-k" => "nidan-u",
        "v4h" => "yodan",
        "v4t" => "yodan",
        "v4g" => "yodan",
        "v2h-s" => "nidan-l",
        "v2g-s" => "nidan-l",
        "v4b" => "yodan",
        "v2y-s" => "nidan-l",
        "v2d-s" => "nidan-l",
        "v2y-k" => "nidan-u",
        "v2k-k" => "nidan-u",
        "v2g-k" => "nidan-u",
        "v2b-k" => "nidan-u",
        "v2s-s" => "nidan-l",
        "v2z-s" => "nidan-l",
        "v2t-s" => "nidan-l",
        "v2n-s" => "nidan-l",
        "v2w-s" => "nidan-l",
    };
}
