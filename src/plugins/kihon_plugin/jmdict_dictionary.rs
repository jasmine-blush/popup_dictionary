use ahash::AHashMap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::plugin::change_progress;

#[derive(Clone)]
pub struct Dictionary {
    db: Db,
}

#[derive(Hash, PartialEq, Eq)]
pub enum DictionaryKey {
    Term(String),
    Reading(String),
}
impl DictionaryKey {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            DictionaryKey::Term(s) => {
                let mut buf = Vec::with_capacity(s.len() + 1);
                buf.push(0);
                buf.extend_from_slice(s.as_bytes());
                buf
            }
            DictionaryKey::Reading(s) => {
                let mut buf = Vec::with_capacity(s.len() + 1);
                buf.push(1);
                buf.extend_from_slice(s.as_bytes());
                buf
            }
        }
    }
}

#[derive(bincode::Encode, bincode::Decode, Debug)]
pub struct DictionaryEntry {
    pub terms: Vec<DictionaryTerm>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct DictionaryTerm {
    pub id: String,
    pub frequency: DictionaryFrequency,
    pub common: bool,
    pub term: String,
    pub reading: String,
    pub alt_forms: Vec<AltForm>,
    pub furigana: Option<Vec<DictionaryFurigana>>,
    pub meanings: Vec<DictionaryMeaning>,
}

#[derive(bincode::Encode, bincode::Decode, Clone, Debug)]
pub struct DictionaryFrequency {
    bccwj: Option<usize>,
    jiten: Option<usize>,
}
impl DictionaryFrequency {
    pub fn get_for_cmp(&self) -> Option<usize> {
        if self.bccwj.is_some() {
            return self.bccwj;
        }
        return self.jiten;
    }
    pub fn get_all(&self) -> (Option<usize>, Option<usize>) {
        return (self.bccwj, self.jiten);
    }
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

// bccwj-combined json
type BCCWJEntry<'b> = (&'b str, &'b str, BCCWJData<'b>); // [term, "freq", {"reading": "reading", "frequency": 0}]

#[derive(Serialize, Deserialize, Debug)]
struct BCCWJData<'b> {
    reading: &'b str,
    frequency: usize,
}
// ---

struct Dependencies {
    bccwj: String,
    jiten: String,
    furigana: String,
    simplified: String,
}

#[derive(Hash, PartialEq, Eq)]
struct FrequencyKey<'c> {
    term: &'c str,
    reading: &'c str,
}

const DB_VERSION_FLAG: &str = "db_version_004";

impl Dictionary {
    pub fn load_dictionary(
        path: &PathBuf,
        progress: &Arc<Mutex<String>>,
    ) -> Result<Self, Box<dyn Error>> {
        let version_flag_path = path.join("version.dat");
        let mut valid_db = false;

        if version_flag_path.exists() {
            if let Ok(version) = fs::read_to_string(&version_flag_path) {
                if version.trim() == DB_VERSION_FLAG {
                    valid_db = true;
                }
            }
        }
        if valid_db {
            let db: Db = sled::open(path)?;
            if db.was_recovered() {
                if db.contains_key(DB_VERSION_FLAG)? {
                    return Ok(Self { db });
                }
            }
        }

        if path.exists() {
            let _ = fs::remove_dir_all(path);
        }

        let db: Db = sled::open(path)?;

        Self::populate_database(&db, progress)?;

        fs::write(&version_flag_path, DB_VERSION_FLAG)?;

        Ok(Self { db })
    }

    fn populate_database<'a, 'b>(
        db: &'a Db,
        progress: &'b Arc<Mutex<String>>,
    ) -> Result<&'a Db, Box<dyn Error>> {
        tracing::info!("Trying to populate database for Kihon plugin.");

        let dependencies = Self::fetch_dependencies(progress)?;
        Self::parse_jmdict_simplified(&db, dependencies, progress)?;

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

        // term_meta_bank_1.json
        let bccwj_frequency_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading bccwj-combined.");

            crate::plugins::kihon_plugin::dependencies::get_bccwj_combined().unwrap()
        });

        // jmdict-furigana.json
        let jmdict_furigana_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading jmdict-furigana.");

            crate::plugins::kihon_plugin::dependencies::get_jmdict_furigana().unwrap()
        });

        // frequency_list_global.csv
        let jiten_frequency_handle = std::thread::spawn(|| {
            tracing::debug!("Downloading jiten-moe.");

            crate::plugins::kihon_plugin::dependencies::get_jiten_moe().unwrap()
        });

        change_progress(
            progress,
            "Downloading datasets [1/4]. \nThis may take a few minutes.",
        );
        let jiten_frequency = jiten_frequency_handle
            .join()
            .map_err(|e| format!("Could not download jiten-moe: {:?}", e))?;
        tracing::debug!("jiten-moe successfully downloaded.");
        change_progress(
            progress,
            "Downloading datasets [2/4]. \nThis may take a few minutes.",
        );
        let jmdict_furigana = jmdict_furigana_handle
            .join()
            .map_err(|e| format!("Could not download jmdict-furigana: {:?}", e))?;
        tracing::debug!("jmdict-furigana successfully downloaded.");
        change_progress(
            progress,
            "Downloading datasets [3/4]. \nThis may take a few minutes.",
        );
        let bccwj_frequency = bccwj_frequency_handle
            .join()
            .map_err(|e| format!("Could not download bccwj-combined: {:?}", e))?;
        tracing::debug!("bccwj-combined successfully downloaded.");
        change_progress(
            progress,
            "Downloading datasets [4/4]. \nThis may take a few minutes.",
        );
        let jmdict_simplified = jmdict_simplified_handle
            .join()
            .map_err(|e| format!("Could not download jmdict-simplified: {:?}", e))?;
        tracing::debug!("jmdict-simplified successfully downloaded.");

        Ok(Dependencies {
            bccwj: bccwj_frequency,
            jiten: jiten_frequency,
            furigana: jmdict_furigana,
            simplified: jmdict_simplified,
        })
    }

    fn parse_jmdict_simplified(
        db: &Db,
        dependencies: Dependencies,
        progress: &Arc<Mutex<String>>,
    ) -> Result<(), Box<dyn Error>> {
        change_progress(
            &progress,
            "Parsing frequency data. \nThis may take a few minutes.",
        );
        let bccwj_frequency_map: AHashMap<FrequencyKey, usize> =
            Self::parse_bccwj_frequencies(&dependencies.bccwj)?;
        let jiten_frequency_map: AHashMap<FrequencyKey, usize> =
            Self::parse_jiten_frequencies(&dependencies.jiten)?;
        change_progress(
            &progress,
            "Parsing furigana data. \nThis may take a few minutes.",
        );
        let furigana_map: AHashMap<(&str, &str), Vec<Furigana>> =
            Self::parse_jmdict_furigana(&dependencies.furigana)?;

        change_progress(
            &progress,
            "Parsing dictionary data. \nThis may take a few minutes.",
        );
        let jmdict: JMDict = serde_json::from_str(&dependencies.simplified)?;

        change_progress(
            &progress,
            "Generating dictionary database. \nThis may take a few minutes.",
        );

        //TODO: build db_entries with rayon here
        let mut db_entries: AHashMap<DictionaryKey, DictionaryEntry> =
            AHashMap::with_capacity(460000); // 455399
        let wildcard: String = String::from("*");
        for word in &jmdict.words {
            let mut terms: Vec<(DictionaryKey, DictionaryTerm)> = Vec::new();

            if !word.kanji.is_empty() {
                for kanji in &word.kanji {
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

                    for kana in word.kana.iter().filter(|kana| {
                        kana.applies_to_kanji.contains(&wildcard)
                            || kana.applies_to_kanji.contains(&kanji.text)
                    }) {
                        let id = word.id.clone();

                        let frequency_key = FrequencyKey {
                            term: &kanji.text.as_str(),
                            reading: &kana.text.as_str(),
                        };
                        let bccjw_frequency = bccwj_frequency_map.get(&frequency_key);
                        let jiten_frequency = jiten_frequency_map.get(&frequency_key);
                        let frequency = DictionaryFrequency {
                            bccwj: bccjw_frequency.cloned(),
                            jiten: jiten_frequency.cloned(),
                        };

                        let common = kanji.common.clone();
                        let term = kanji.text.clone();
                        let reading = kana.text.clone();

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

                        terms.push((
                            DictionaryKey::Term(kanji.text.clone()),
                            DictionaryTerm {
                                id: id.clone(),
                                frequency: frequency.clone(),
                                common,
                                term: term.clone(),
                                reading: reading.clone(),
                                alt_forms: Vec::new(),
                                furigana: furigana.clone(),
                                meanings: meanings.clone(),
                            },
                        ));

                        //let frequency = frequency_kana.or(frequency_kanji).cloned();
                        let common = kana.common.clone();

                        terms.push((
                            DictionaryKey::Reading(kana.text.clone()),
                            DictionaryTerm {
                                id,
                                frequency: frequency.clone(),
                                common,
                                term,
                                reading,
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
                    let id = word.id.clone();
                    let frequency_key = FrequencyKey {
                        term: &kana.text.as_str(),
                        reading: &kana.text.as_str(),
                    };
                    let bccjw_frequency = bccwj_frequency_map.get(&frequency_key);
                    let jiten_frequency = jiten_frequency_map.get(&frequency_key);
                    let frequency = DictionaryFrequency {
                        bccwj: bccjw_frequency.cloned(),
                        jiten: jiten_frequency.cloned(),
                    };
                    let common = kana.common.clone();

                    let reading = kana.text.clone();

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

                    terms.push((
                        DictionaryKey::Reading(kana.text.clone()),
                        DictionaryTerm {
                            id,
                            frequency: frequency.clone(),
                            common,
                            term: String::new(),
                            reading,
                            alt_forms: Vec::new(),
                            furigana: None,
                            meanings,
                        },
                    ));
                }

                for i in 0..terms.len() {
                    let mut alt_forms: Vec<AltForm> = Vec::new();
                    for j in 0..terms.len() {
                        if i == j {
                            continue;
                        }

                        let entry = &terms[i].1;
                        let comp = &terms[j].1;

                        //TODO: also check kanji and kana tags
                        if entry.meanings == comp.meanings {
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

                    terms[i].1.alt_forms = alt_forms;
                }

                for (key, term) in terms.drain(..) {
                    db_entries
                        .entry(key)
                        .or_insert_with(|| DictionaryEntry { terms: Vec::new() })
                        .terms
                        .push(term);
                }
            } else {
                for kana in &word.kana {
                    let id = word.id.clone();
                    let frequency_key = FrequencyKey {
                        term: &kana.text.as_str(),
                        reading: &kana.text.as_str(),
                    };
                    let bccjw_frequency = bccwj_frequency_map.get(&frequency_key);
                    let jiten_frequency = jiten_frequency_map.get(&frequency_key);
                    let frequency = DictionaryFrequency {
                        bccwj: bccjw_frequency.cloned(),
                        jiten: jiten_frequency.cloned(),
                    };
                    let common = kana.common.clone();

                    let reading = kana.text.clone();

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

                    terms.push((
                        DictionaryKey::Reading(kana.text.clone()),
                        DictionaryTerm {
                            id,
                            frequency: frequency.clone(),
                            common,
                            term: String::new(),
                            reading,
                            alt_forms: Vec::new(),
                            furigana: None,
                            meanings,
                        },
                    ));
                }

                let len = terms.len();
                for i in 0..len {
                    let mut alt_forms: Vec<AltForm> = Vec::new();
                    for j in 0..len {
                        if i == j {
                            continue;
                        }

                        let entry = &terms[i].1;
                        let comp = &terms[j].1;

                        //TODO: also check kana tags
                        if entry.meanings == comp.meanings {
                            alt_forms.push(AltForm {
                                term: comp.term.clone(),
                                reading: comp.reading.clone(),
                                furigana: comp.furigana.clone(),
                            });
                        }
                    }

                    terms[i].1.alt_forms = alt_forms;
                }

                for (key, term) in terms.drain(..) {
                    db_entries
                        .entry(key)
                        .or_insert_with(|| DictionaryEntry { terms: Vec::new() })
                        .terms
                        .push(term);
                }
            }
        }

        let entries_vec: Vec<(DictionaryKey, DictionaryEntry)> = db_entries.into_iter().collect();
        entries_vec.into_par_iter().chunks(2000).for_each(|chunk| {
            let mut batch = sled::Batch::default();

            for (key, mut entry) in chunk {
                let serialized_entry: Vec<u8> =
                    bincode::encode_to_vec(&entry, bincode::config::standard()).unwrap();
                batch.insert(key.serialize(), serialized_entry);
            }

            db.apply_batch(batch).unwrap();
        });

        Ok(())
    }

    fn parse_bccwj_frequencies(
        bccwj_string: &str,
    ) -> Result<AHashMap<FrequencyKey<'_>, usize>, Box<dyn Error>> {
        let json: Vec<BCCWJEntry> = serde_json::from_str(bccwj_string)?;

        let mut frequency_map: AHashMap<FrequencyKey, usize> = AHashMap::with_capacity(json.len());
        for item in json {
            frequency_map
                .entry(FrequencyKey {
                    term: item.0,
                    reading: item.2.reading,
                })
                .and_modify(|e| {
                    *e = (*e).min(item.2.frequency);
                })
                .or_insert(item.2.frequency);
        }

        // special frequency for 乃「の」to prioritize even when written as kana
        frequency_map.insert(
            FrequencyKey {
                term: "乃",
                reading: "の",
            },
            1,
        );

        Ok(frequency_map)
    }

    fn parse_jiten_frequencies(
        jiten_string: &str,
    ) -> Result<AHashMap<FrequencyKey<'_>, usize>, Box<dyn Error>> {
        let lines: Vec<&str> = jiten_string.split_whitespace().collect();

        let mut frequency_map: AHashMap<FrequencyKey, usize> = AHashMap::with_capacity(lines.len());
        for i in 1..lines.len() {
            let line = lines[i];

            let entry: Vec<&str> = line.split(',').collect();

            if entry.len() == 3 {
                let term = entry[0];
                let reading = entry[1];
                let frequency: usize = entry[2].parse()?;
                frequency_map
                    .entry(FrequencyKey {
                        term: term,
                        reading: reading,
                    })
                    .and_modify(|e| {
                        *e = (*e).min(frequency);
                    })
                    .or_insert(frequency);
            }
        }

        // special frequency for 乃「の」to prioritize even when written as kana
        frequency_map.insert(
            FrequencyKey {
                term: "乃",
                reading: "の",
            },
            1,
        );

        Ok(frequency_map)
    }

    fn parse_jmdict_furigana(
        furigana_string: &str,
    ) -> Result<AHashMap<(&str, &str), Vec<Furigana<'_>>>, Box<dyn Error>> {
        let json_string_safe = furigana_string
            .strip_prefix('\u{FEFF}')
            .unwrap_or(&furigana_string);
        let json: Vec<JMDictFurigana> = serde_json::from_str(json_string_safe)?;

        let mut furigana_map: AHashMap<(&str, &str), Vec<Furigana>> =
            AHashMap::with_capacity(json.len());
        for item in json {
            furigana_map.insert((item.text, item.reading), item.furigana);
        }

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

    /*
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
    }*/

    pub fn lookup(&self, word: &str) -> Result<Option<DictionaryEntry>, Box<dyn Error>> {
        if let Some(serialized_entry) = self
            .db
            .get(DictionaryKey::Term(word.to_owned()).serialize())?
        {
            let (entry, _): (DictionaryEntry, usize) =
                bincode::decode_from_slice(&serialized_entry, bincode::config::standard())
                    .expect(&format!("{:?}", &serialized_entry));
            return Ok(Some(entry));
        }
        if let Some(serialized_entry) = self
            .db
            .get(DictionaryKey::Reading(word.to_owned()).serialize())?
        {
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
