//! The effect-class vocabulary both languages price on, and the longest-prefix
//! lookup over a table each brings itself. `py-provers/src/catalog.rs` and
//! `rs-provers/src/catalog.rs` hold the rows; the class names live here alone
//! so a reading means the same thing on both sides.

use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;

pub const PURE: &str = "pure";
pub const LOGS: &str = "logs";
pub const READS: &str = "reads-world";
pub const WRITES: &str = "writes-world";
pub const PROCESS: &str = "writes-process";
pub const BLOCKS: &str = "blocks";
pub const SPAWNS: &str = "spawns";
pub const MUTATES: &str = "mutates-receiver";
pub const COLLECTS: &str = "collects";
pub const CONTAINER: &str = "returns-container";
pub const REMOTE: &str = "off-machine";
pub const DELETES: &str = "deletes";

/// World contact.
pub static IO: LazyLock<ClassSet> = LazyLock::new(|| {
    [LOGS, READS, WRITES, PROCESS, BLOCKS, SPAWNS]
        .into_iter()
        .collect()
});

/// Spending: `IO` minus the developer-facing effects (a log line, a
/// `breakpoint()`), which cost a caller nothing, plus the two off-machine
/// classes.
pub static SPENDS: LazyLock<ClassSet> =
    LazyLock::new(|| [SPAWNS, PROCESS, REMOTE, DELETES].into_iter().collect());

/// The classes one spelling names. Sorted, so a reading that reaches a message
/// is deterministic whatever order the rows were folded in.
pub type ClassSet = BTreeSet<&'static str>;

static NONE: LazyLock<ClassSet> = LazyLock::new(ClassSet::new);

/// A language's table plus the separator its paths use (`.` for Python, `::`
/// for Rust).
pub struct Catalog {
    table: HashMap<&'static str, ClassSet>,
    sep: &'static str,
}

impl Catalog {
    /// The class-to-names fold: each row is `(class, "name name ...")`, and a
    /// name listed under several classes takes all of them.
    pub fn new(sep: &'static str, rows: &[(&'static str, &'static str)]) -> Catalog {
        let mut table: HashMap<&'static str, ClassSet> = HashMap::new();
        for (class, names) in rows {
            for name in crate::pytext::split(names) {
                table.entry(name).or_default().insert(class);
            }
        }
        Catalog { table, sep }
    }

    /// One key, exactly as written.
    pub fn get(&self, key: &str) -> &ClassSet {
        self.table.get(key).unwrap_or(&NONE)
    }

    /// The longest separated prefix of `key` the table lists, so a member
    /// overrides its root (`urllib.parse` under `urllib`).
    pub fn longest_prefix(&self, key: &str) -> Option<&ClassSet> {
        let mut node = key;
        while !node.is_empty() {
            if let Some(classes) = self.table.get(node) {
                return Some(classes);
            }
            node = match node.rfind(self.sep) {
                Some(at) => &node[..at],
                None => "",
            };
        }
        None
    }

    /// The classes a call site spells: its longest dotted prefix in the table,
    /// else the bare name it spells on whatever receiver.
    pub fn classes_of(&self, dotted: Option<&str>, name: Option<&str>) -> &ClassSet {
        if let Some(found) = self.longest_prefix(dotted.unwrap_or("")) {
            return found;
        }
        let name = name.unwrap_or("");
        let on_receiver = self.get(&format!(".{name}"));
        if on_receiver.is_empty() {
            self.get(name)
        } else {
            on_receiver
        }
    }

    /// Does this spelling build a value and reorder nothing an import hoist
    /// could see? Only a listed pure entry answers yes.
    pub fn inert(&self, spelling: Option<&str>) -> bool {
        spelling.is_some() && self.classes_of(spelling, None).contains(PURE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An inline stand-in for the Python table: the rows themselves are phase
    /// 3's (`py-provers/src/catalog.rs`), the mechanics are this module's.
    fn catalog() -> Catalog {
        Catalog::new(
            ".",
            &[
                (
                    PURE,
                    "len os.path urllib.parse re.compile logging.getLogger",
                ),
                (LOGS, "logging .info .warning"),
                (READS, "open urllib os.listdir .read_bytes"),
                (WRITES, "shutil shutil.rmtree os.remove .write_text"),
                (DELETES, "shutil.rmtree os.remove"),
                (SPAWNS, "subprocess subprocess.run"),
                (MUTATES, ".append .update"),
                (COLLECTS, ".append"),
            ],
        )
    }

    #[test]
    fn a_member_overrides_its_root_and_a_bare_name_lands_on_any_receiver() {
        let c = catalog();
        assert!(
            c.classes_of(Some("os.path.join"), Some("join"))
                .contains(PURE)
        );
        assert!(
            c.classes_of(Some("urllib.parse.quote"), None)
                .contains(PURE)
        );
        assert!(
            c.classes_of(Some("urllib.request.urlopen"), None)
                .contains(READS)
        );
        assert!(
            c.classes_of(Some("logging.getLogger"), Some("getLogger"))
                .contains(PURE)
        );
        assert!(
            c.classes_of(Some("logging.warning"), Some("warning"))
                .contains(LOGS)
        );
        assert!(c.classes_of(None, Some("info")).contains(LOGS));
        assert!(c.classes_of(None, Some("read_bytes")).contains(READS));
        assert!(
            c.classes_of(Some("mypkg.helpers.format_row"), None)
                .is_empty()
        );
    }

    #[test]
    fn a_name_under_two_classes_carries_both() {
        let c = catalog();
        assert_eq!(
            *c.classes_of(None, Some("append")),
            ClassSet::from([COLLECTS, MUTATES])
        );
        assert_eq!(
            *c.classes_of(Some("os.remove"), None),
            ClassSet::from([DELETES, WRITES])
        );
    }

    #[test]
    fn only_a_listed_pure_entry_is_inert() {
        let c = catalog();
        assert!(c.inert(Some("re.compile")) && c.inert(Some("len")));
        assert!(!c.inert(Some("json.dumps")) && !c.inert(None));
    }

    #[test]
    fn a_log_line_is_io_but_never_a_spend() {
        let c = catalog();
        let logging = c.classes_of(Some("logging"), Some("logging"));
        assert!(!logging.is_disjoint(&IO));
        assert!(logging.is_disjoint(&SPENDS));
        assert!(!c.classes_of(Some("open"), None).is_disjoint(&IO));
        assert!(c.classes_of(Some("open"), None).is_disjoint(&SPENDS));
        assert!(
            !c.classes_of(Some("shutil.rmtree"), None)
                .is_disjoint(&SPENDS)
        );
        assert!(!c.classes_of(Some("shutil.rmtree"), None).is_disjoint(&IO));
    }

    #[test]
    fn the_rust_separator_reads_the_same_table_shape() {
        let c = Catalog::new(
            "::",
            &[(READS, "std::fs .read_to_string"), (LOGS, "println")],
        );
        assert!(
            c.classes_of(Some("std::fs::read_to_string"), None)
                .contains(READS)
        );
        assert!(c.classes_of(Some("println"), None).contains(LOGS));
        assert!(c.classes_of(Some("std::mem::swap"), None).is_empty());
        assert!(c.classes_of(None, Some("read_to_string")).contains(READS));
    }
}
