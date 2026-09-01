//! Unit rs-context's integration tests in one binary: the port of
//! `tests/rs/test_rules_context.py`, `test_rules_dead.py` and
//! `test_rules_trust.py`. A target per file relinks the whole dependency
//! graph, so one module per source file sits under this one.

mod context;
mod dead;
mod trust;
