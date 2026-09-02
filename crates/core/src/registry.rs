//! The rule registry: every language's `RULES` aggregated into one table.
//!
//! A reading is per language, so an id may hold one record per language:
//! they share slug and family, and `by_id` answers with the one every
//! consumer of those means. Posture is
//! the reading's own, since a reading no round has judged ships REPORT
//! while its sibling ratchets.

use std::collections::HashMap;

use anyhow::bail;

use crate::rule::{Posture, RuleRecord};

/// Ids no rule holds. A retired id is never reused; `explain` answers one
/// from `data/retired.toml`.
pub const RETIRED: &[&str] = &[
    "4", "8", "13", "15", "16", "17", "19", "22", "25", "28", "30", "31", "43", "45", "46", "51",
    "52", "61",
];

#[derive(Debug)]
pub struct Registry {
    /// sorted by (id as an integer, lang)
    pub rules: Vec<RuleRecord>,
    /// slug -> id, the suppression marker's alias map
    pub id_by_slug: HashMap<String, String>,
    by_id: HashMap<&'static str, usize>,
    by_reading: HashMap<(&'static str, &'static str), usize>,
}

impl Registry {
    /// # Errors
    ///
    /// An id that is not a number, a retired id, or two readings of one id
    /// in one language.
    pub fn new(records: Vec<RuleRecord>) -> anyhow::Result<Self> {
        let mut rules = records;
        rules.sort_by_key(|r| (r.id.parse::<u32>().unwrap_or(0), r.lang));

        let mut by_id: HashMap<&'static str, usize> = HashMap::new();
        let mut by_reading: HashMap<(&'static str, &'static str), usize> = HashMap::new();
        let mut id_by_slug = HashMap::new();
        for (i, r) in rules.iter().enumerate() {
            if r.id.parse::<u32>().is_err() {
                bail!("rule id {:?} is not a number", r.id);
            }
            if RETIRED.contains(&r.id) {
                bail!("rule #{} is retired: an id is never reused", r.id);
            }
            if by_reading.insert((r.id, r.lang), i).is_some() {
                bail!("duplicate rule reading: #{} for {}", r.id, r.lang);
            }
            by_id.entry(r.id).or_insert(i);
            id_by_slug.insert(r.slug.to_string(), r.id.to_string());
        }
        Ok(Self {
            rules,
            id_by_slug,
            by_id,
            by_reading,
        })
    }

    /// The first record holding this id, whichever language wrote it.
    #[must_use]
    #[allow(clippy::indexing_slicing, reason = "an index `new` took from `rules`")]
    pub fn by_id(&self, id: &str) -> Option<&RuleRecord> {
        self.by_id.get(id).map(|&i| &self.rules[i])
    }

    #[must_use]
    #[allow(clippy::indexing_slicing, reason = "an index `new` took from `rules`")]
    pub fn reading(&self, id: &str, lang: &str) -> Option<&RuleRecord> {
        self.by_reading.get(&(id, lang)).map(|&i| &self.rules[i])
    }

    /// What a finding blocks on: the record of the language that produced
    /// it, since an unjudged reading reports while its sibling ratchets.
    #[must_use]
    pub fn posture_of(&self, id: &str, lang: &str) -> Option<Posture> {
        self.reading(id, lang)
            .or_else(|| self.by_id(id))
            .map(|r| r.posture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::rule::Scope;

    fn record(id: &'static str, lang: &'static str, posture: Posture) -> RuleRecord {
        RuleRecord {
            id,
            slug: if lang == "py" {
                "structural-clones"
            } else {
                "rs-clones"
            },
            family: "surface",
            engine_class: "IDX",
            posture,
            meaning: "m",
            goal: "g",
            lang,
            scope: Scope::Repo,
            complement: "",
        }
    }

    fn registry() -> Registry {
        Registry::new(vec![
            record("11", "rs", Posture::Report),
            record("2", "py", Posture::Ratchet),
            record("11", "py", Posture::Ratchet),
        ])
        .unwrap()
    }

    #[test]
    fn records_sort_by_id_as_a_number_then_language() {
        let reg = registry();
        assert_eq!(
            reg.rules.iter().map(|r| (r.id, r.lang)).collect::<Vec<_>>(),
            [("2", "py"), ("11", "py"), ("11", "rs")]
        );
    }

    #[test]
    fn by_id_answers_with_the_first_reading_and_posture_with_the_language() {
        let reg = registry();
        assert_eq!(reg.by_id("11").unwrap().lang, "py");
        assert_eq!(reg.posture_of("11", "py"), Some(Posture::Ratchet));
        assert_eq!(reg.posture_of("11", "rs"), Some(Posture::Report));
        // a language with no reading of its own falls back to `by_id`
        assert_eq!(reg.posture_of("11", "q"), Some(Posture::Ratchet));
        assert_eq!(reg.posture_of("99", "py"), None);
    }

    #[test]
    fn the_slug_alias_map_reaches_every_id() {
        let reg = registry();
        assert_eq!(reg.id_by_slug["structural-clones"], "11");
        assert_eq!(reg.id_by_slug["rs-clones"], "11");
    }

    #[test]
    fn a_duplicate_reading_is_refused() {
        let err = Registry::new(vec![
            record("11", "py", Posture::Ratchet),
            record("11", "py", Posture::Report),
        ])
        .unwrap_err();
        assert_eq!(err.to_string(), "duplicate rule reading: #11 for py");
    }

    #[test]
    fn a_retired_id_is_refused() {
        let err = Registry::new(vec![record("13", "py", Posture::Ratchet)]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "rule #13 is retired: an id is never reused"
        );
    }

    #[test]
    fn the_retired_list_is_the_one_the_rule_table_buries() {
        // a retired id is never reused; `explain` prints its burial rows
        assert_eq!(RETIRED.len(), 18);
        let mut ids: Vec<u32> = RETIRED.iter().map(|s| s.parse().unwrap()).collect();
        let sorted = {
            let mut c = ids.clone();
            c.sort_unstable();
            c
        };
        assert_eq!(ids, sorted, "the list reads in id order");
        ids.dedup();
        assert_eq!(ids.len(), RETIRED.len());
    }
}
