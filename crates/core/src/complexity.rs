//! SonarSource cognitive complexity, the half no language owns: a classified
//! tree and the sum over it. Each language classifies its own nodes into `Cc`
//! (`py-facts/src/complexity.rs`, `rs-facts/src/complexity.rs`) and hands the
//! roots to `score`.

/// SonarSource's default bar: #23 emits past it, #48 never folds a caller past
/// it.
pub const CC_THRESHOLD: u32 = 15;

/// A classified node. `flat` is an increment nesting does not scale (a boolean
/// run, a recursive call, an `else`), `nests` one it does, `inner` puts the
/// node one level in from its parent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cc {
    pub flat: u32,
    pub nests: bool,
    pub inner: bool,
    pub kids: Vec<Cc>,
}

/// A nesting increment costs 1 plus the depth it sits at, a flat one its own
/// weight.
pub fn score(nodes: &[Cc], nesting: u32) -> u32 {
    let mut total = 0;
    for n in nodes {
        let depth = nesting + u32::from(n.inner);
        total += n.flat + (1 + depth) * u32::from(n.nests) + score(&n.kids, depth);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nester(kids: Vec<Cc>) -> Cc {
        Cc {
            nests: true,
            kids,
            ..Default::default()
        }
    }

    fn inner(mut node: Cc) -> Cc {
        node.inner = true;
        node
    }

    #[test]
    fn a_nesting_increment_costs_one_plus_its_depth() {
        // `if` holding an `if` holding an `if`: 1 + 2 + 3
        let deep = nester(vec![inner(nester(vec![inner(nester(vec![]))]))]);
        assert_eq!(score(std::slice::from_ref(&deep), 0), 6);
        // the same body priced as if it sat two levels in (#48's fold)
        assert_eq!(score(std::slice::from_ref(&deep), 2), 12);
    }

    #[test]
    fn a_flat_increment_ignores_depth() {
        let boolean = Cc {
            flat: 1,
            ..Default::default()
        };
        let body = nester(vec![inner(boolean.clone())]);
        assert_eq!(score(&[boolean], 0), 1);
        assert_eq!(score(&[body], 0), 2);
        assert_eq!(score(&[], 5), 0);
    }
}
