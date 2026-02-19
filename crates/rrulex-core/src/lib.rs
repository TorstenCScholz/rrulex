use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TextStats {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    pub bytes: usize,
    pub most_common_word: Option<String>,
    pub unique_words: usize,
}

pub fn analyze(input: &str) -> TextStats {
    let lines = input.lines().count();
    let chars = input.chars().count();
    let bytes = input.len();

    let words_vec: Vec<&str> = input.split_whitespace().collect();
    let words = words_vec.len();

    let mut freq: HashMap<String, usize> = HashMap::new();
    for word in &words_vec {
        let lower = word.to_lowercase();
        *freq.entry(lower).or_insert(0) += 1;
    }

    let unique_words = freq.len();

    let most_common_word = freq
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(word, _)| word);

    TextStats {
        lines,
        words,
        chars,
        bytes,
        most_common_word,
        unique_words,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let stats = analyze("");
        assert_eq!(stats.lines, 0);
        assert_eq!(stats.words, 0);
        assert_eq!(stats.chars, 0);
        assert_eq!(stats.bytes, 0);
        assert_eq!(stats.most_common_word, None);
        assert_eq!(stats.unique_words, 0);
    }

    #[test]
    fn simple_text() {
        let stats = analyze("hello world");
        assert_eq!(stats.lines, 1);
        assert_eq!(stats.words, 2);
        assert_eq!(stats.unique_words, 2);
    }

    #[test]
    fn repeated_words() {
        let stats = analyze("the the the cat");
        assert_eq!(stats.most_common_word, Some("the".to_string()));
        assert_eq!(stats.unique_words, 2);
    }
}
