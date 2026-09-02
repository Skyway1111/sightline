//! What a call to an external callable does. One table, each consumer
//! projecting the classes it cares about. A callable is keyed dotted -
//! through the module's bindings (`os.remove`), a root standing for its whole
//! module, a `<class>.<method>` the receiver's type spells (`str.join`), a
//! bare builtin - or, with a leading dot, by the name it spells on any
//! receiver (`.append`). The longest dotted prefix wins, so a member
//! overrides its root (`urllib.parse` under `urllib`).
//!
//! An unlisted callable is assumed pure: it computes and returns. It is never
//! assumed *inert* - only a listed pure entry answers `inert`, and a kind
//! joins that set measured: a hoist over it applied and passed its repo's
//! suite. The lookup is `core::catalog::Catalog`; the rows are here, one row
//! per name (a class may take several rows, and a name listed under several
//! classes takes all of them).

use std::collections::BTreeSet;
use std::sync::LazyLock;

use sightline_core::catalog::{
    BLOCKS, COLLECTS, CONTAINER, Catalog, ClassSet, DELETES, LOGS, MUTATES, PROCESS, PURE, READS,
    REMOTE, SPAWNS, WRITES,
};

/// `_BY_CLASS`, verbatim. Listed, not merely unlisted: a consumer acts on
/// being told pure - an import-time hoist, a #41 loop shape, a member
/// overriding a world root. `os.path` answers questions about a path string;
/// its stat members read the filesystem but reorder nothing an import could
/// see. cv2 / PIL / matplotlib / torch compute in memory (`cv2.cvtColor`,
/// `plt.subplots`, `cv2.imdecode` on a buffer, `torch.cat` in a forward);
/// only their file, window and device members touch the world.
const BY_CLASS: &[(&str, &str)] = &[
    (
        PURE,
        "len frozenset set tuple range enumerate repr sorted os.path urllib.parse",
    ),
    (
        PURE,
        "dataclasses.dataclass dataclasses.field functools.lru_cache numpy.linspace",
    ),
    (
        PURE,
        "collections.OrderedDict types.MappingProxyType logging.getLogger str.join",
    ),
    (
        PURE,
        "str.replace ndarray.setflags pathlib.Path Path.resolve",
    ),
    (PURE, "re.compile re.match re.search re.fullmatch"),
    (PURE, "re.sub re.subn re.findall re.finditer re.split"),
    (
        LOGS,
        "print breakpoint logging .debug .info .warning .warn .error .critical .exception",
    ),
    (LOGS, ".log .print_exc"),
    (
        READS,
        "open input _io socket sqlite3 glob urllib sqlalchemy redis psycopg",
    ),
    (
        READS,
        "psycopg2 os.listdir os.walk os.scandir os.stat json.load pickle.load",
    ),
    (
        READS,
        "cv2.imread cv2.VideoCapture PIL.Image.open torch.load",
    ),
    (
        READS,
        ".read .readlines .read_text .read_bytes .open .input .urlopen",
    ),
    (
        WRITES,
        "shutil smtplib ftplib paramiko tempfile sqlalchemy redis psycopg",
    ),
    (
        WRITES,
        "psycopg2 os.remove os.unlink os.rmdir os.removedirs shutil.rmtree",
    ),
    (WRITES, "os.makedirs os.mkdir os.rename json.dump"),
    (
        WRITES,
        "pickle.dump cv2.imwrite cv2.imshow matplotlib.pyplot.savefig",
    ),
    (
        WRITES,
        "matplotlib.pyplot.show torch.save .write .writelines .write_text",
    ),
    (WRITES, ".write_bytes .savefig .to_csv"),
    // process state: what a module mutates at import time is #9's question
    (
        PROCESS,
        "sys.path.insert sys.path.append sys.setrecursionlimit os.putenv os.chdir",
    ),
    (
        PROCESS,
        "os.environ.update os.environ.setdefault dotenv.load_dotenv random.seed",
    ),
    (
        PROCESS,
        "random.shuffle random.setstate numpy.random.seed torch.manual_seed",
    ),
    (
        PROCESS,
        "logging.basicConfig multiprocessing.set_start_method",
    ),
    (PROCESS, "warnings.filterwarnings warnings.simplefilter"),
    (BLOCKS, "time.sleep .sleep .acquire .wait .wait_for"),
    // a world that is not this machine, and data no reader gets back: with
    // SPAWNS and PROCESS, what #59 asks a first screen to declare
    (
        REMOTE,
        "socket sqlite3 urllib urllib3 sqlalchemy redis psycopg psycopg2",
    ),
    (REMOTE, "smtplib ftplib paramiko .urlopen"),
    (
        DELETES,
        "shutil.rmtree os.remove os.unlink os.rmdir os.removedirs",
    ),
    (
        SPAWNS,
        "subprocess threading multiprocessing asyncio boto3 curl_cffi requests httpx",
    ),
    // market-data fetchers: every call is a round trip
    (SPAWNS, "yfinance akshare"),
    (
        SPAWNS,
        "aiohttp urllib3 websockets tensorflow os.system subprocess.run",
    ),
    (
        SPAWNS,
        "torch.cuda torch.distributed torch.hub torch.multiprocessing",
    ),
    (
        SPAWNS,
        "subprocess.call subprocess.check_call subprocess.check_output",
    ),
    (
        SPAWNS,
        "subprocess.Popen requests.get requests.post requests.put requests.delete",
    ),
    (
        SPAWNS,
        "requests.head requests.patch requests.request .Thread .Process .gather",
    ),
    (
        SPAWNS,
        ".ThreadPoolExecutor .ProcessPoolExecutor .create_task .ensure_future",
    ),
    (
        SPAWNS,
        ".run_in_executor .submit .apply_async .to_thread .as_completed",
    ),
    (SPAWNS, ".start_new_thread"),
    (
        MUTATES,
        ".append .extend .insert .remove .pop .clear .sort .reverse .update .add",
    ),
    (MUTATES, ".discard .setdefault .put .write .writelines"),
    (COLLECTS, ".append .add .extend .put .setdefault"),
    // not an effect - what the callee gives back, which #3's guard reads
    (CONTAINER, "sorted tuple frozenset re.findall re.split"),
    (
        CONTAINER,
        ".findall .split .rsplit .splitlines .items .keys .values",
    ),
];

pub static TABLE: LazyLock<Catalog> = LazyLock::new(|| Catalog::new(".", BY_CLASS));

/// The classes a call site spells: its longest dotted prefix in the table,
/// else the bare name it spells on whatever receiver.
pub fn classes_of(dotted: Option<&str>, name: Option<&str>) -> &'static ClassSet {
    TABLE.classes_of(dotted, name)
}

/// Does this spelling build a value and reorder nothing an import hoist could
/// see? Only a listed pure entry answers yes.
pub fn inert(spelling: Option<&str>) -> bool {
    TABLE.inert(spelling)
}

/// The names one class lists, with the leading dot dropped where the class
/// keys receivers.
fn projection(class: &'static str, on_receiver: bool) -> BTreeSet<&'static str> {
    BY_CLASS
        .iter()
        .filter(|(c, _)| *c == class)
        .flat_map(|(_, names)| sightline_core::pytext::split(names))
        .filter_map(|n| match n.strip_prefix('.') {
            Some(bare) if on_receiver => Some(bare),
            None if !on_receiver => Some(n),
            _ => None,
        })
        .collect()
}

// a receiver class holds only `.name` keys, a process write only dotted ones
pub static MUTATOR_METHODS: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| projection(MUTATES, true));
pub static COLLECTORS: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| projection(COLLECTS, true));
pub static IMPORT_TIME_MUTATORS: LazyLock<BTreeSet<&'static str>> =
    LazyLock::new(|| projection(PROCESS, false));

// #41's loop shapes name their own members, each behind its micro-bench
// (`xtask perf-catalog`): never a projection of the table above.
pub const SUBPROCESS_CALLS: [&str; 5] = [
    "subprocess.run",
    "subprocess.call",
    "subprocess.check_call",
    "subprocess.check_output",
    "subprocess.Popen",
];
pub const HTTP_CALLS: [&str; 7] = [
    "requests.get",
    "requests.post",
    "requests.put",
    "requests.delete",
    "requests.head",
    "requests.patch",
    "requests.request",
];
pub const RE_CALLS: [&str; 9] = [
    "re.compile",
    "re.match",
    "re.search",
    "re.fullmatch",
    "re.sub",
    "re.subn",
    "re.findall",
    "re.finditer",
    "re.split",
];

#[cfg(test)]
mod tests {
    use super::*;

    use sightline_core::catalog::{IO, SPENDS};

    #[test]
    fn every_class_answers_for_a_spelling_a_consumer_sees() {
        let rows: &[(Option<&str>, Option<&str>, &str)] = &[
            // a member overriding no root: string work
            (Some("os.path.join"), Some("join"), PURE),
            (Some("re.compile"), Some("compile"), PURE),
            // overrides the `logging` root
            (Some("logging.getLogger"), Some("getLogger"), PURE),
            (Some("logging.warning"), Some("warning"), LOGS),
            // `log.info(...)`: a name on any receiver
            (None, Some("info"), LOGS),
            (Some("os.listdir"), Some("listdir"), READS),
            // a db client root
            (Some("sqlalchemy.select"), Some("select"), READS),
            (None, Some("read_bytes"), READS),
            (Some("shutil.copytree"), Some("copytree"), WRITES),
            (Some("redis.Redis"), Some("Redis"), WRITES),
            (None, Some("write_text"), WRITES),
            (Some("sys.path.insert"), Some("insert"), PROCESS),
            (Some("os.environ.setdefault"), Some("setdefault"), PROCESS),
            (Some("time.sleep"), Some("sleep"), BLOCKS),
            (None, Some("acquire"), BLOCKS),
            (Some("subprocess.run"), Some("run"), SPAWNS),
            // the #59 real's root
            (Some("curl_cffi.requests.post"), Some("post"), SPAWNS),
            (Some("boto3.client"), Some("client"), SPAWNS),
            (Some("psycopg.connect"), Some("connect"), WRITES),
            (None, Some("ThreadPoolExecutor"), SPAWNS),
            (None, Some("append"), MUTATES),
            (Some("re.findall"), Some("findall"), CONTAINER),
            (None, Some("splitlines"), CONTAINER),
        ];
        for (dotted, name, class) in rows {
            assert!(
                classes_of(*dotted, *name).contains(class),
                "{dotted:?} {name:?} {class}"
            );
        }
    }

    #[test]
    fn no_pure_or_unlisted_call_spends_or_shows_as_io() {
        for dotted in [
            "urllib.parse.urlencode", // pure member under a world root
            "os.path.dirname",
            "json.dumps", // unlisted: assumed pure
            "mypkg.helpers.format_row",
        ] {
            let classes = classes_of(Some(dotted), None);
            assert!(
                classes.is_disjoint(&IO) && classes.is_disjoint(&SPENDS),
                "{dotted}"
            );
        }
    }

    /// Unlisted is assumed pure but never assumed inert: a kind joins
    /// measured (`pathlib.Path` did, once the shipped-subset rejection took
    /// the one hoist that had failed on it); `json.dumps` never has.
    #[test]
    fn only_a_listed_pure_entry_is_inert() {
        assert!(inert(Some("re.compile")) && inert(Some("len")));
        assert!(inert(Some("urllib.parse.quote")) && inert(Some("pathlib.Path")));
        assert!(!inert(Some("json.dumps")) && !inert(None));
    }

    #[test]
    fn a_log_line_is_io_but_never_a_spend() {
        let logging = classes_of(Some("logging"), Some("logging"));
        assert!(!logging.is_disjoint(&IO) && logging.is_disjoint(&SPENDS));
        assert!(classes_of(None, Some("print_exc")).is_disjoint(&SPENDS));
    }

    #[test]
    fn the_projections_each_consumer_takes() {
        // a collector mutates its receiver
        assert!(COLLECTORS.is_subset(&MUTATOR_METHODS) && *COLLECTORS != *MUTATOR_METHODS);
        assert!(MUTATOR_METHODS.contains("write") && MUTATOR_METHODS.contains("put"));
        assert!(IMPORT_TIME_MUTATORS.contains("random.seed"));
        assert!(!IMPORT_TIME_MUTATORS.iter().any(|n| n.starts_with('.')));
    }

    /// `cv2.cvtColor` / `plt.subplots` spend nothing; the file members do.
    #[test]
    fn image_and_plot_roots_compute_in_memory() {
        assert!(
            classes_of(Some("cv2.cvtColor"), None).is_disjoint(&BTreeSet::from([READS, WRITES]))
        );
        assert!(classes_of(Some("cv2.imread"), None).contains(READS));
        assert!(classes_of(Some("matplotlib.pyplot.savefig"), None).contains(WRITES));
    }

    /// #59's projection: work outside this process, a machine this is not,
    /// state everyone shares, data no reader gets back. Local file work is
    /// still `IO` for the rules that ask about effects at all.
    #[test]
    fn what_a_first_screen_is_asked_to_declare() {
        for spelling in [
            "subprocess.run",
            "urllib.request.urlopen",
            "redis",
            "os.environ.update",
            "shutil.rmtree",
            "os.remove",
        ] {
            assert!(
                !classes_of(Some(spelling), None).is_disjoint(&SPENDS),
                "{spelling}"
            );
        }
        for spelling in ["open", "os.walk", "json.dump", "time.sleep", "print"] {
            assert!(
                classes_of(Some(spelling), None).is_disjoint(&SPENDS),
                "{spelling}"
            );
        }
        // an effect, just not a spend; and a deleter is still a writer
        assert!(!classes_of(Some("open"), None).is_disjoint(&IO));
        assert!(!classes_of(Some("shutil.rmtree"), None).is_disjoint(&IO));
    }

    /// `torch.cat` / `torch.chunk` / `F.pad` build tensors; only the device,
    /// checkpoint and distributed members reach past the process.
    #[test]
    fn tensor_math_computes_in_memory() {
        for spelling in [
            "torch.cat",
            "torch.chunk",
            "torch.nn.functional.pad",
            "torch.nn.LayerNorm",
            "torch.no_grad",
        ] {
            assert!(
                classes_of(Some(spelling), None).is_disjoint(&SPENDS),
                "{spelling}"
            );
        }
        assert!(!classes_of(Some("torch.cuda.device_count"), None).is_disjoint(&SPENDS));
        assert!(classes_of(Some("torch.load"), None).contains(READS));
        assert!(classes_of(Some("torch.save"), None).contains(WRITES));
        assert!(classes_of(Some("torch.manual_seed"), None).contains(PROCESS));
    }
}
