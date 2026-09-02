//! Repeat mining, the half no language owns: a front digests its own
//! statements into `Seq`s, this module finds every maximal repeated run worth
//! a name, and the front maps the runs back onto its nodes.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use sha2::{Digest, Sha256};

/// The subtree floor a clone has to clear to be worth a name.
pub const MIN_CLONE_NODES: usize = 20;
/// The idiom floor: under five statements a blinded block is a fetch, guard,
/// unpack prologue or a setup trio.
pub const MIN_BLOCK_STMTS: usize = 5;

/// sha256 of the text, lowercase hex, cut to 12 (R9).
pub fn digest(text: &str) -> String {
    digest_n(text, 12)
}

/// R9's other length: #35's cycle key and #38's value key cut to 8.
pub fn digest_n(text: &str, chars: usize) -> String {
    let out = Sha256::digest(text.as_bytes());
    let mut hex = String::with_capacity(chars);
    for byte in &out[..chars.div_ceil(2)] {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex.truncate(chars);
    hex
}

/// A statement sequence as the mining reads it: a blind digest and a node count
/// per statement, `order` totally ordering sequences, `top` its owner's whole
/// body, `prod` a sequence outside the tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seq {
    pub digests: Vec<String>,
    pub sizes: Vec<usize>,
    pub order: String,
    pub top: bool,
    pub prod: bool,
}

/// A reported run: `key` digests it, each occurrence is (sequence, start).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repeat {
    pub key: String,
    pub length: usize,
    pub runs: Vec<(usize, usize)>,
}

/// The predecessor of an occurrence: the digest before it, or the sequence it
/// starts, which cannot extend left.
#[derive(PartialEq, Eq, Hash)]
enum Predecessor<'a> {
    Digest(&'a str),
    Start(usize),
}

/// A group past the floors, before the containment collapse ranks it.
type Kept = (usize, String, Vec<(usize, usize)>);

/// Every maximal repeated statement run worth a name, minus the whole-body
/// duplicates a caller's function arm owns and the test-only groups.
pub fn repeats(seqs: &[Seq]) -> Vec<Repeat> {
    let mut kept: Vec<Kept> = Vec::new();
    for (length, occurrences) in maximal_repeats(seqs) {
        let mut runs: Vec<(usize, usize)> = occurrences
            .into_iter()
            .filter(|&(s, i)| seqs[s].sizes[i..i + length].iter().sum::<usize>() >= MIN_CLONE_NODES)
            .collect();
        // the function arm owns whole-body duplicates
        let whole_bodies = runs
            .iter()
            .all(|&(s, i)| seqs[s].top && i == 0 && length == seqs[s].digests.len());
        if runs.len() < 2 || whole_bodies {
            continue;
        }
        runs = drop_overlaps(seqs, length, &runs);
        // a test-only group is arrange-block noise, while a prod twin of a test
        // block stays reported at the prod site
        if runs.len() >= 2 && runs.iter().any(|&(s, _)| seqs[s].prod) {
            let (s0, i0) = runs[0];
            let key = digest(&seqs[s0].digests[i0..i0 + length].join("\n"));
            kept.push((length, key, runs));
        }
    }
    // containment collapse: a group every member of which lies inside an
    // already-reported longer run adds nothing
    let mut covered: HashMap<usize, Vec<(usize, usize)>> = HashMap::new();
    let mut out: Vec<Repeat> = Vec::new();
    kept.sort_by(|a, b| (Reverse(a.0), &a.1).cmp(&(Reverse(b.0), &b.1)));
    for (length, key, runs) in kept {
        let contained = runs.iter().all(|&(s, i)| {
            covered
                .get(&s)
                .is_some_and(|spans| spans.iter().any(|&(a, b)| a <= i && i + length <= b))
        });
        if contained {
            continue;
        }
        for &(s, i) in &runs {
            covered.entry(s).or_default().push((i, i + length));
        }
        out.push(Repeat { key, length, runs });
    }
    out
}

/// (length, [(sequence, start), ...]) per maximal repeated digest run: a suffix
/// array's LCP intervals are exactly the runs that recur and extend no further,
/// right-maximal by the interval, left-maximal where the occurrences differ in
/// their predecessor. No run crosses a sequence, so the suffixes need no
/// separators, and only a position whose floor-length window recurs can start
/// one: that is the array's whole population.
fn maximal_repeats(seqs: &[Seq]) -> Vec<(usize, Vec<(usize, usize)>)> {
    let floor = MIN_BLOCK_STMTS;
    let mut starts: HashMap<&[String], Vec<(usize, usize)>> = HashMap::new();
    for (s, seq) in seqs.iter().enumerate() {
        for i in 0..seq.digests.len().saturating_sub(floor - 1) {
            starts
                .entry(&seq.digests[i..i + floor])
                .or_default()
                .push((s, i));
        }
    }
    let mut suffixes: Vec<(&[String], usize, usize)> = starts
        .values()
        .filter(|group| group.len() > 1)
        .flatten()
        .map(|&(s, i)| (&seqs[s].digests[i..], s, i))
        .collect();
    suffixes.sort();

    let mut out: Vec<(usize, Vec<(usize, usize)>)> = Vec::new();
    let mut stack: Vec<(usize, usize)> = vec![(0, 0)]; // (run length, index of its first suffix)
    for k in 1..=suffixes.len() {
        let here = if k < suffixes.len() {
            common(suffixes[k - 1].0, suffixes[k].0)
        } else {
            0
        };
        let mut left = k - 1;
        while here < stack.last().expect("the stack keeps its floor").0 {
            let (length, popped) = stack.pop().expect("the loop guard read the top");
            left = popped;
            let occurrences: Vec<(usize, usize)> =
                suffixes[left..k].iter().map(|&(_, s, i)| (s, i)).collect();
            let predecessors: HashSet<Predecessor> = occurrences
                .iter()
                .map(|&(s, i)| {
                    if i > 0 {
                        Predecessor::Digest(&seqs[s].digests[i - 1])
                    } else {
                        Predecessor::Start(s)
                    }
                })
                .collect();
            if length >= floor && predecessors.len() > 1 {
                out.push((length, occurrences));
            }
        }
        if here > stack.last().expect("the stack keeps its floor").0 {
            stack.push((here, left));
        }
    }
    out
}

fn common(a: &[String], b: &[String]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// Within one sequence, overlapping occurrences of one run (a periodic row)
/// collapse to non-overlapping ones, earliest first.
fn drop_overlaps(seqs: &[Seq], length: usize, runs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = runs.to_vec();
    sorted.sort_by_key(|&(s, i)| (&seqs[s].order, i));
    let mut kept: Vec<(usize, usize)> = Vec::new();
    let mut last_end: HashMap<usize, usize> = HashMap::new();
    for (s, i) in sorted {
        if last_end.get(&s).is_none_or(|&end| i >= end) {
            last_end.insert(s, i + length);
            kept.push((s, i));
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<String> {
        text.chars().map(|c| c.to_string()).collect()
    }

    fn row(order: &str, digests: &str, size: usize, top: bool, prod: bool) -> Seq {
        let digests = chars(digests);
        let sizes = vec![size; digests.len()];
        Seq {
            digests,
            sizes,
            order: order.to_string(),
            top,
            prod,
        }
    }

    #[test]
    fn digest_is_sha256_hex_cut_to_twelve() {
        assert_eq!(digest(""), "e3b0c44298fc");
        assert_eq!(digest("a\nb"), "7e18f737311b");
    }

    // Every expected value below is the repeat set for the row sequences
    // the test beside it builds.

    #[test]
    fn a_whole_body_duplicate_belongs_to_the_function_arm() {
        // two functions whose entire bodies are the same five statements
        let seqs = [
            row("a.py", "12345", 5, true, true),
            row("b.py", "12345", 5, true, true),
        ];
        assert_eq!(repeats(&seqs), Vec::<Repeat>::new());
    }

    #[test]
    fn a_test_only_group_is_dropped_and_its_prod_twin_is_not() {
        let tests_only = [
            row("t1.py", "012345", 5, false, false),
            row("t2.py", "912345", 5, false, false),
        ];
        assert_eq!(repeats(&tests_only), Vec::<Repeat>::new());

        let with_prod = [
            row("t1.py", "012345", 5, false, false),
            row("p2.py", "912345", 5, false, true),
        ];
        // the key and the run order are the drop-overlaps sort's: `p2.py`
        // sorts before `t1.py`, so the prod copy names the group
        assert_eq!(
            repeats(&with_prod),
            vec![Repeat {
                key: "b5584a146411".to_string(),
                length: 5,
                runs: vec![(1, 1), (0, 1)],
            }]
        );
    }

    #[test]
    fn overlapping_occurrences_in_one_sequence_collapse_to_the_earliest() {
        // "12121" sits at 0 and 2 in the first row and at 1 in the second:
        // three occurrences, two runs
        let seqs = [
            row("a.py", "1212121", 5, false, true),
            row("b.py", "9121219", 5, false, true),
        ];
        assert_eq!(
            repeats(&seqs),
            vec![Repeat {
                key: "02160b4177f6".to_string(),
                length: 5,
                runs: vec![(0, 0), (1, 1)],
            }]
        );
    }

    #[test]
    fn a_group_inside_longer_reported_runs_collapses_into_them() {
        // "CDEFG" recurs in all four rows and is maximal, but every one of its
        // four occurrences lies inside a reported seven-statement run
        let seqs = [
            row("a.py", "ABCDEFG", 5, false, true),
            row("b.py", "ABCDEFG", 5, false, true),
            row("c.py", "CDEFGHI", 5, false, true),
            row("d.py", "CDEFGHI", 5, false, true),
        ];
        assert_eq!(
            repeats(&seqs),
            vec![
                Repeat {
                    key: "679da1b754fc".to_string(),
                    length: 7,
                    runs: vec![(0, 0), (1, 0)],
                },
                Repeat {
                    key: "a80e173060ec".to_string(),
                    length: 7,
                    runs: vec![(2, 0), (3, 0)],
                },
            ]
        );
    }

    #[test]
    fn a_run_under_the_node_floor_is_not_worth_a_name() {
        // three nodes a statement, five statements: 15 under the floor of 20
        let seqs = [
            row("a.py", "12345", 3, false, true),
            row("b.py", "12345", 3, false, true),
        ];
        assert_eq!(repeats(&seqs), Vec::<Repeat>::new());
    }
}
