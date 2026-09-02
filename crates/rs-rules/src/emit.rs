//! Verified findings' deletions as one unified diff
//! against each file's raw bytes, line endings preserved. Writes text, never
//! the tree.
//!
//! The findings arrive verified - #32 pays its worlds at audit time - so all
//! this owes is the settling of patches that share a site (`core::patch`'s
//! `compose`, the language-blind half) and the printing. A deletion brings no
//! import to place, so nothing here re-parses: the world already compiled this
//! text, and the patch differs from it only by the emptied lines it drops.

use std::collections::BTreeMap;

use sightline_core::edits::{apply_edits, takes_line};
use sightline_core::findings::{Finding, SpanEdit};
use sightline_core::patch::{compose, headers, unified_diff};
use sightline_core::pytext;
use sightline_rs_facts::model::RsFacts;
use sightline_rs_provers::RsProvers;

/// `Language.fix` for `RS`: the unified diff, empty where nothing is fixable.
/// The leading `# sightline-fix:` lines name what the patch discharges (`git
/// apply` ignores text before the first diff header). The provers are the
/// record's third slot and nothing here asks them: a deletion was verified
/// when the finding was made.
pub fn fix(findings: &[Finding], facts: &RsFacts<'_>, _provers: &RsProvers<'_>) -> String {
    let composed = compose(findings.to_vec());
    let mut by_rel: BTreeMap<&str, Vec<&Finding>> = BTreeMap::new();
    for f in &composed {
        if let Some(fix) = &f.fix {
            by_rel.entry(&fix.rel).or_default().push(f);
        }
    }
    let mut patched_findings: Vec<Finding> = Vec::new();
    let mut body = String::new();
    for (rel, group) in by_rel {
        let Some(qname) = facts.module_by_rel.get(rel) else {
            continue;
        };
        // the parse read the file's bytes whole, so its own source holds the
        // line endings `git apply` matches on disk; split on `\n` only, ends
        // kept (`re.findall(r".*\n|.+$", raw)`)
        let old: Vec<String> = facts.modules[qname]
            .source
            .split_inclusive('\n')
            .map(str::to_string)
            .collect();
        let edits: Vec<SpanEdit> = group
            .iter()
            .filter_map(|f| f.fix.as_ref())
            .flat_map(|fix| fix.edits.iter().cloned())
            .collect();
        patched_findings.extend(group.into_iter().cloned());
        body.push_str(&unified_diff(&old, &patched(old.clone(), &edits), rel));
    }
    if body.is_empty() {
        return String::new();
    }
    headers(&patched_findings).concat() + &body
}

/// The file under these edits with every emptied line dropped: a world keeps
/// the line, its diagnostic diff being line-keyed, and a patch does not.
fn patched(mut lines: Vec<String>, edits: &[SpanEdit]) -> Vec<String> {
    apply_edits(&mut lines, edits);
    let mut dead: Vec<u32> = edits
        .iter()
        .filter(|e| takes_line(e))
        .map(|e| e.line)
        .collect();
    dead.sort_unstable();
    dead.dedup();
    for line in dead.into_iter().rev() {
        if pytext::strip(&lines[line as usize - 1]).is_empty() {
            lines.remove(line as usize - 1);
        }
    }
    lines
}
