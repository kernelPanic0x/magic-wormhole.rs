use rand::{rngs::OsRng, seq::SliceRandom};
use serde_json::{self, Value};
use std::fmt;

#[derive(PartialEq)]
pub struct Wordlist {
    num_words: usize,
    words: Vec<Vec<String>>,
}

impl fmt::Debug for Wordlist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Wordlist ( {}, lots of words...)", self.num_words)
    }
}

impl Wordlist {
    #[cfg(test)]
    pub fn new(num_words: usize, words: Vec<Vec<String>>) -> Wordlist {
        Wordlist { num_words, words }
    }

    pub fn get_completions(&self, prefix: &str) -> Vec<String> {
        let count_dashes = prefix.matches('-').count();
        let words = &self.words[count_dashes % self.words.len()];

        let (prefix_without_last, last_partial) = prefix.rsplit_once('-').unwrap_or(("", prefix));

        let matches = if cfg!(feature = "fuzzy-complete") {
            self.fuzzy_complete(last_partial, words)
        } else {
            words
                .iter()
                .filter(|word| word.starts_with(last_partial))
                .cloned()
                .collect()
        };

        matches
            .into_iter()
            .map(|word| {
                let mut completion = String::new();
                completion.push_str(prefix_without_last);
                if !prefix_without_last.is_empty() {
                    completion.push('-');
                }
                completion.push_str(&word);
                completion
            })
            .collect()
    }

    /// Get either even or odd wordlist
    pub fn get_wordlist(&self, prefix: &str, cursor_pos: Option<usize>) -> &Vec<String> {
        let limited_prefix = match cursor_pos {
            Some(pos) if pos < prefix.len() => &prefix[..pos],
            _ => prefix,
        };
        let count_dashes = limited_prefix.matches('-').count();
        &self.words[count_dashes % self.words.len()]
    }

    #[cfg(feature = "fuzzy-complete")]
    fn fuzzy_complete(&self, partial: &str, words: &[String]) -> Vec<String> {
        use fuzzt::algorithms::JaroWinkler;

        let words = words.iter().map(|w| w.as_str()).collect::<Vec<&str>>();

        fuzzt::get_top_n(partial, &words, None, None, None, Some(&JaroWinkler))
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    pub fn choose_words(&self) -> String {
        let mut rng = OsRng;
        let components: Vec<String> = self
            .words
            .iter()
            .cycle()
            .take(self.num_words)
            .map(|words| words.choose(&mut rng).unwrap().to_string())
            .collect();
        components.join("-")
    }
}

/// Extract partial str from prefix with cursor position
pub fn extract_partial_from_prefix<'a>(prefix: &'a str, pos: usize) -> &'a str {
    let current_word_start = prefix[..pos].rfind('-').map(|i| i + 1).unwrap_or(0);
    let current_word_end = prefix[pos..]
        .find('-')
        .map(|i| i + pos)
        .unwrap_or_else(|| prefix.len());

    &prefix[current_word_start..current_word_end]
}

fn load_pgpwords() -> Vec<Vec<String>> {
    let raw_words_value: Value = serde_json::from_str(include_str!("pgpwords.json")).unwrap();
    let raw_words = raw_words_value.as_object().unwrap();
    let mut even_words: Vec<String> = Vec::with_capacity(256);
    even_words.resize(256, String::from(""));
    let mut odd_words: Vec<String> = Vec::with_capacity(256);
    odd_words.resize(256, String::from(""));
    for (index_str, values) in raw_words.iter() {
        let index = u8::from_str_radix(index_str, 16).unwrap() as usize;
        even_words[index] = values
            .get(1)
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase()
            .to_string();
        odd_words[index] = values
            .get(0)
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase()
            .to_string();
    }

    vec![even_words, odd_words]
}

pub fn default_wordlist(num_words: usize) -> Wordlist {
    Wordlist {
        num_words,
        words: load_pgpwords(),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_load_words() {
        let w = load_pgpwords();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0][0], "adroitness");
        assert_eq!(w[1][0], "aardvark");
        assert_eq!(w[0][255], "yucatan");
        assert_eq!(w[1][255], "zulu");
    }

    #[test]
    fn test_default_wordlist() {
        let d = default_wordlist(2);
        assert_eq!(d.words.len(), 2);
        assert_eq!(d.words[0][0], "adroitness");
        assert_eq!(d.words[1][0], "aardvark");
        assert_eq!(d.words[0][255], "yucatan");
        assert_eq!(d.words[1][255], "zulu");
    }

    fn vecstrings(all: &str) -> Vec<String> {
        all.split_whitespace()
            .map(|s| {
                if s == "." {
                    String::from("")
                } else {
                    s.to_string()
                }
            })
            .collect()
    }

    #[test]
    fn test_choose_words() {
        let few_words: Vec<Vec<String>> = vec![vecstrings("purple"), vecstrings("sausages")];

        let w = Wordlist::new(2, few_words.clone());
        assert_eq!(w.choose_words(), "purple-sausages");
        let w = Wordlist::new(3, few_words.clone());
        assert_eq!(w.choose_words(), "purple-sausages-purple");
        let w = Wordlist::new(4, few_words);
        assert_eq!(w.choose_words(), "purple-sausages-purple-sausages");
    }

    #[test]
    fn test_choose_more_words() {
        let more_words: Vec<Vec<String>> =
            vec![vecstrings("purple yellow"), vecstrings("sausages")];

        let expected2 = vecstrings("purple-sausages yellow-sausages");
        let expected3: Vec<String> = vec![
            "purple-sausages-purple",
            "yellow-sausages-purple",
            "purple-sausages-yellow",
            "yellow-sausages-yellow",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let w = Wordlist::new(2, more_words.clone());
        for _ in 0..20 {
            assert!(expected2.contains(&w.choose_words()));
        }

        let w = Wordlist::new(3, more_words);
        for _ in 0..20 {
            assert!(expected3.contains(&w.choose_words()));
        }
    }

    #[test]
    fn test_wormhole_code_completions() {
        let list = default_wordlist(2);

        assert_eq!(list.get_completions("22"), Vec::<String>::new());

        assert_eq!(
            list.get_completions("22-chisel"),
            ["22-chisel", "22-chairlift", "22-christmas"]
        );
    }
}
