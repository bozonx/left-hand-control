//! Shared macro references, cycle detection and bounded expansion analysis.

use std::collections::{HashMap, HashSet};

use super::actions::{Action, MAX_MACRO_DEPTH, MAX_MACRO_STEPS};
use crate::mapper::builtin_macros::BUILTIN_MACROS;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum MacroRef {
    User(String),
    Builtin(String),
}

impl MacroRef {
    fn parse(raw: &str) -> Option<Self> {
        match Action::parse(Some(raw)) {
            Action::Macro(id) => Some(Self::User(id)),
            Action::BuiltinMacro(id) => Some(Self::Builtin(id)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpansionIssue {
    Depth,
    Steps,
}

pub(crate) struct MacroGraph<'a> {
    definitions: HashMap<MacroRef, Vec<&'a str>>,
}

impl<'a> MacroGraph<'a> {
    pub(crate) fn new(users: impl IntoIterator<Item = (&'a str, Vec<&'a str>)>) -> Self {
        let mut definitions: HashMap<_, _> = BUILTIN_MACROS
            .iter()
            .map(|item| (MacroRef::Builtin(item.id.into()), item.steps.to_vec()))
            .collect();
        definitions.extend(
            users
                .into_iter()
                .map(|(id, steps)| (MacroRef::User(id.trim().into()), steps)),
        );
        Self { definitions }
    }

    pub(crate) fn has_cycle(&self, source: &MacroRef) -> bool {
        let mut visited = HashSet::new();
        let mut pending = self.references(source);
        while let Some(current) = pending.pop() {
            if &current == source {
                return true;
            }
            if visited.insert(current.clone()) {
                pending.extend(self.references(&current));
            }
        }
        false
    }

    fn references(&self, source: &MacroRef) -> Vec<MacroRef> {
        self.definitions
            .get(source)
            .into_iter()
            .flatten()
            .filter_map(|raw| MacroRef::parse(raw))
            .collect()
    }

    pub(crate) fn expansion_issues(&self) -> HashMap<MacroRef, ExpansionIssue> {
        fn size(
            graph: &MacroGraph<'_>,
            id: &MacroRef,
            depth: usize,
            memo: &mut HashMap<(MacroRef, usize), Result<usize, ExpansionIssue>>,
        ) -> Result<usize, ExpansionIssue> {
            if depth > MAX_MACRO_DEPTH {
                return Err(ExpansionIssue::Depth);
            }
            let key = (id.clone(), depth);
            if let Some(result) = memo.get(&key) {
                return *result;
            }
            let result = (|| {
                let mut total = 0;
                for step in graph.definitions.get(id).into_iter().flatten() {
                    total += if let Some(reference) = MacroRef::parse(step) {
                        // Count the reference itself, matching the mapper's
                        // bounded expansion contract, even for missing targets.
                        1 + if graph.definitions.contains_key(&reference) {
                            size(graph, &reference, depth + 1, memo)?
                        } else {
                            0
                        }
                    } else {
                        usize::from(!step.trim().is_empty())
                    };
                    if total > MAX_MACRO_STEPS {
                        return Err(ExpansionIssue::Steps);
                    }
                }
                Ok(total)
            })();
            memo.insert(key, result);
            result
        }
        let mut memo = HashMap::new();
        self.definitions
            .keys()
            .filter_map(|source| {
                size(self, source, 0, &mut memo)
                    .err()
                    .map(|issue| (source.clone(), issue))
            })
            .collect()
    }

    #[cfg(test)]
    fn expansion_issue(&self, source: &MacroRef) -> Option<ExpansionIssue> {
        self.expansion_issues().get(source).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycles_distinguish_namespaces_and_ignore_literals_and_missing_targets() {
        let graph = MacroGraph::new([
            ("a", vec![" macro: b "]),
            ("b", vec!["macro:a"]),
            (
                "copyLine",
                vec!["builtin:copyLine", "text:macro:copyLine", "macro:missing"],
            ),
            ("self", vec!["macro:self"]),
        ]);
        assert!(graph.has_cycle(&MacroRef::User("a".into())));
        assert!(graph.has_cycle(&MacroRef::User("b".into())));
        assert!(graph.has_cycle(&MacroRef::User("self".into())));
        assert!(!graph.has_cycle(&MacroRef::User("copyLine".into())));
        assert!(!graph.has_cycle(&MacroRef::User("missing".into())));
    }

    #[test]
    fn long_chains_and_cycles_do_not_recurse_unboundedly() {
        let ids: Vec<_> = (0..2000).map(|i| i.to_string()).collect();
        let actions: Vec<_> = (1..=2000).map(|i| format!("macro:{i}")).collect();
        let mut graph = MacroGraph::new(
            ids.iter()
                .zip(&actions)
                .map(|(id, step)| (id.as_str(), vec![step.as_str()])),
        );
        let source = MacroRef::User("0".into());
        assert!(!graph.has_cycle(&source));
        assert_eq!(graph.expansion_issue(&source), Some(ExpansionIssue::Depth));
        graph
            .definitions
            .insert(MacroRef::User("2000".into()), vec!["macro:0"]);
        assert!(graph.has_cycle(&source));
    }

    #[test]
    fn expansion_counts_repeated_references_and_checks_the_boundary() {
        let steps = vec!["KeyA"; MAX_MACRO_STEPS];
        let graph = MacroGraph::new([
            ("limit", steps),
            ("over", vec!["macro:limit"]),
            ("leaf", vec!["KeyA"]),
            ("repeated", vec!["macro:leaf"; MAX_MACRO_STEPS / 2 + 1]),
        ]);
        assert_eq!(graph.expansion_issue(&MacroRef::User("limit".into())), None);
        assert_eq!(
            graph.expansion_issue(&MacroRef::User("over".into())),
            Some(ExpansionIssue::Steps)
        );
        assert_eq!(
            graph.expansion_issue(&MacroRef::User("repeated".into())),
            Some(ExpansionIssue::Steps)
        );
    }
}
