//! The language-neutral half of a counterfactual pass: the split that turns
//! one merged world's implicated set into the verdict an isolated world would
//! give. It knows a proposal by four attributes and a diagnostic by three; the
//! language stack brings the overlay builder (`world`) and its checker
//! (`verify_worlds`), and keeps the receipt half to itself.

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

/// A file's whole content under the splices, keyed by its path under the root.
pub type World = IndexMap<String, String>;

/// What the split reads of a diagnostic a world added.
pub trait Diag {
    fn rel(&self) -> &str;
    fn line(&self) -> u32;
    fn severity(&self) -> &str;
}

/// What the split reads of a proposal: the file it edits, the callee body
/// lines a hosting error is read in (`(0, 0)` for a module owner) and the
/// files a new error vetoes in (`None`: every file).
pub trait Spliced {
    fn id(&self) -> &str;
    fn rel(&self) -> &str;
    fn span(&self) -> (u32, u32);
    fn watched(&self) -> Option<&HashSet<String>>;
}

/// A new error in a file this splice must not break.
pub fn errored<P: Spliced + ?Sized, D: Diag>(p: &P, added: &[D]) -> bool {
    added.iter().any(|d| {
        d.severity() == "error"
            && match p.watched() {
                None => true,
                Some(watched) => d.rel() == p.rel() || watched.contains(d.rel()),
            }
    })
}

/// The two groups to test next. Watching whole files makes every splice in a
/// file a suspect of every error in it, so the split goes on where the errors
/// are: the members hosting one in their own callee body, and the rest, whom a
/// single world then clears together. With no such split to make, a splice
/// that breaks its callers rather than its body, the group is cut in two.
pub fn split<'p, P: Spliced, D: Diag>(group: &[&'p P], errs: &[D]) -> Vec<Vec<&'p P>> {
    let hosts: Vec<&P> = group
        .iter()
        .copied()
        .filter(|p| {
            let (lo, hi) = p.span();
            errs.iter()
                .any(|d| d.rel() == p.rel() && lo <= d.line() && d.line() <= hi)
        })
        .collect();
    let named: HashSet<&str> = hosts.iter().map(|p| p.id()).collect();
    let rest: Vec<&P> = group
        .iter()
        .copied()
        .filter(|p| !named.contains(p.id()))
        .collect();
    let half = group.len() / 2;
    if !hosts.is_empty() && !rest.is_empty() {
        return vec![hosts, rest];
    }
    if half == 0 {
        vec![group.to_vec()]
    } else {
        vec![group[..half].to_vec(), group[half..].to_vec()]
    }
}

/// The suspects an isolated world would veto, found by splitting: a group
/// whose world raises no error in a member's watched files clears that member,
/// since a subset of a clean world's splices cannot error where the superset
/// did not, so only the still-implicated members split again, down to the
/// singleton world that pins the veto on its own splice. One pass per level,
/// however many groups it holds. A group whose world cleared less than half of
/// it has no structure left to exploit and its members go straight to
/// singletons, so an all-veto set costs the isolated count plus the two worlds
/// that learned so.
pub fn vetoed<P, D, W, V>(
    suspects: &[&P],
    added: &[D],
    world: W,
    mut verify_worlds: V,
) -> HashSet<String>
where
    P: Spliced,
    D: Diag,
    W: Fn(&[&P]) -> World,
    V: FnMut(&[(String, World)]) -> IndexMap<String, Vec<D>>,
{
    let mut out: HashSet<String> = HashSet::new();
    let mut groups = if suspects.is_empty() {
        Vec::new()
    } else {
        split(suspects, added)
    };
    while !groups.is_empty() {
        let batch: Vec<(String, World)> = groups
            .iter()
            .enumerate()
            .map(|(i, g)| (i.to_string(), world(g)))
            .collect();
        // an absent world: the checker crashed under the pass, and nothing is vetoed
        let mut answers: HashMap<String, Vec<D>> = verify_worlds(&batch).into_iter().collect();
        let mut pending: Vec<Vec<&P>> = Vec::new();
        for (i, group) in groups.iter().enumerate() {
            let errs = answers.remove(&i.to_string()).unwrap_or_default();
            let live: Vec<&P> = group
                .iter()
                .copied()
                .filter(|p| errored(*p, &errs))
                .collect();
            if group.len() == 1 {
                out.extend(live.iter().map(|p| p.id().to_string()));
            } else if 2 * live.len() > group.len() {
                pending.extend(live.into_iter().map(|p| vec![p]));
            } else if !live.is_empty() {
                pending.extend(split(&live, &errs));
            }
        }
        groups = pending;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rec {
        id: String,
        rel: String,
        span: (u32, u32),
        watched: Option<HashSet<String>>,
    }

    impl Rec {
        fn new(id: &str, rel: &str, span: (u32, u32), watched: Option<&[&str]>) -> Rec {
            Rec {
                id: id.to_string(),
                rel: rel.to_string(),
                span,
                watched: watched.map(|w| w.iter().map(|s| s.to_string()).collect()),
            }
        }
    }

    impl Spliced for Rec {
        fn id(&self) -> &str {
            &self.id
        }
        fn rel(&self) -> &str {
            &self.rel
        }
        fn span(&self) -> (u32, u32) {
            self.span
        }
        fn watched(&self) -> Option<&HashSet<String>> {
            self.watched.as_ref()
        }
    }

    #[derive(Clone)]
    struct D {
        rel: String,
        line: u32,
        severity: String,
    }

    impl D {
        fn new(rel: &str, line: u32) -> D {
            D {
                rel: rel.to_string(),
                line,
                severity: "error".to_string(),
            }
        }
        fn warning(rel: &str, line: u32) -> D {
            D {
                rel: rel.to_string(),
                line,
                severity: "warning".to_string(),
            }
        }
    }

    impl Diag for D {
        fn rel(&self) -> &str {
            &self.rel
        }
        fn line(&self) -> u32 {
            self.line
        }
        fn severity(&self) -> &str {
            &self.severity
        }
    }

    /// One world per file: its content is the ids spliced into it.
    fn world(group: &[&Rec]) -> World {
        let mut out = World::new();
        for p in group {
            let content: Vec<&str> = group
                .iter()
                .filter(|q| q.rel == p.rel)
                .map(|q| q.id.as_str())
                .collect();
            out.insert(p.rel.clone(), content.join(" "));
        }
        out
    }

    /// A `verify_worlds` that answers each id's pinned error wherever that id
    /// is in the world, recording the ids of every pass.
    fn checker<'a>(
        breaks: &'a HashMap<String, D>,
        passes: &'a mut Vec<Vec<String>>,
    ) -> impl FnMut(&[(String, World)]) -> IndexMap<String, Vec<D>> + 'a {
        move |batch| {
            passes.push(batch.iter().map(|(wid, _)| wid.clone()).collect());
            batch
                .iter()
                .map(|(wid, w)| {
                    let found: Vec<D> = w
                        .values()
                        .flat_map(|content| content.split(' '))
                        .filter_map(|id| breaks.get(id).cloned())
                        .collect();
                    (wid.clone(), found)
                })
                .collect()
        }
    }

    fn breaks(rows: &[(&str, D)]) -> HashMap<String, D> {
        rows.iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect()
    }

    #[test]
    fn a_watched_file_and_a_severity_bound_the_error() {
        let p = Rec::new("a", "m.rs", (1, 2), Some(&["c.rs"]));
        assert!(errored(&p, &[D::new("m.rs", 1)])); // its own file is always watched
        assert!(errored(&p, &[D::new("c.rs", 9)]));
        assert!(!errored(&p, &[D::new("other.rs", 3)]));
        assert!(!errored(&p, &[D::warning("c.rs", 9)]));
        // no dependents to enumerate: an error anywhere vetoes
        let owner = Rec::new("b", "m.rs", (0, 0), None);
        assert!(errored(&owner, &[D::new("other.rs", 3)]));
    }

    #[test]
    fn a_hosting_error_isolates_its_member_and_clears_the_rest_in_one_world() {
        // four splices in one file, so each is a suspect of the one error; it
        // sits in the first one's body, and that names it
        let group: Vec<Rec> = (1..5)
            .map(|i| Rec::new(&format!("p{i}"), "b.rs", (2 * i - 1, 2 * i), Some(&[])))
            .collect();
        let refs: Vec<&Rec> = group.iter().collect();
        let breaks = breaks(&[("p1", D::new("b.rs", 1))]);
        let mut passes: Vec<Vec<String>> = Vec::new();

        let out = vetoed(
            &refs,
            &[D::new("b.rs", 1)],
            world,
            checker(&breaks, &mut passes),
        );

        assert_eq!(out, HashSet::from(["p1".to_string()]));
        // the host alone against the three it cleared
        assert_eq!(passes, vec![vec!["0".to_string(), "1".to_string()]]);
    }

    #[test]
    fn a_caller_only_error_splits_the_group_in_two() {
        // the error is in a file no splice edits, so no body names it: the
        // group is cut in two, and only the half holding the breakers splits on
        let group: Vec<Rec> = (1..5)
            .map(|i| {
                Rec::new(
                    &format!("p{i}"),
                    &format!("m{i}.rs"),
                    (1, 2),
                    Some(&["c.rs"]),
                )
            })
            .collect();
        let refs: Vec<&Rec> = group.iter().collect();
        let breaks = breaks(&[("p1", D::new("c.rs", 9)), ("p2", D::new("c.rs", 9))]);
        let mut passes: Vec<Vec<String>> = Vec::new();

        let added = vec![D::new("c.rs", 9), D::new("c.rs", 9)];
        let out = vetoed(&refs, &added, world, checker(&breaks, &mut passes));

        assert_eq!(out, HashSet::from(["p1".to_string(), "p2".to_string()]));
        // the two groups, then the breaking group's two singletons: the clean
        // pair never bought a world of its own
        assert_eq!(passes, vec![vec!["0", "1"], vec!["0", "1"]]);
    }

    #[test]
    fn a_group_cleared_under_half_goes_to_singletons() {
        // every member breaks, so neither group clears anyone: the structure is
        // spent and the isolated worlds are bought at once
        let group: Vec<Rec> = (1..5)
            .map(|i| {
                Rec::new(
                    &format!("p{i}"),
                    &format!("m{i}.rs"),
                    (1, 2),
                    Some(&["c.rs"]),
                )
            })
            .collect();
        let refs: Vec<&Rec> = group.iter().collect();
        let rows: Vec<(&str, D)> = ["p1", "p2", "p3", "p4"]
            .iter()
            .map(|id| (*id, D::new("c.rs", 9)))
            .collect();
        let breaks = breaks(&rows);
        let mut passes: Vec<Vec<String>> = Vec::new();

        let added: Vec<D> = (0..4).map(|_| D::new("c.rs", 9)).collect();
        let out = vetoed(&refs, &added, world, checker(&breaks, &mut passes));

        assert_eq!(
            out,
            HashSet::from(["p1", "p2", "p3", "p4"].map(str::to_string))
        );
        assert_eq!(passes, vec![vec!["0", "1"], vec!["0", "1", "2", "3"]]);
    }

    #[test]
    fn an_absent_world_vetoes_nothing() {
        // the checker crashed under the pass and answered no world: an
        // unanswered group clears, so the run degrades instead of vetoing on no
        // evidence
        let group: Vec<Rec> = (1..5)
            .map(|i| Rec::new(&format!("p{i}"), &format!("m{i}.rs"), (1, 2), None))
            .collect();
        let refs: Vec<&Rec> = group.iter().collect();
        let mut passes: Vec<Vec<String>> = Vec::new();

        let dead = |batch: &[(String, World)]| {
            passes.push(
                batch
                    .iter()
                    .map(|(wid, _)| wid.clone())
                    .collect::<Vec<String>>(),
            );
            IndexMap::<String, Vec<D>>::new()
        };
        assert_eq!(
            vetoed(&refs, &[D::new("c.rs", 9)], world, dead),
            HashSet::new()
        );
        assert_eq!(passes, vec![vec!["0", "1"]]);
    }

    #[test]
    fn no_suspect_buys_no_world() {
        let mut passes: Vec<Vec<String>> = Vec::new();
        let breaks = HashMap::new();
        let out = vetoed::<Rec, D, _, _>(&[], &[], world, checker(&breaks, &mut passes));
        assert_eq!(out, HashSet::new());
        assert!(passes.is_empty());
    }
}
