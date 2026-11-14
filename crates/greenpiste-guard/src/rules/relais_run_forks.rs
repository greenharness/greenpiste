//! `relais-run-forks` — a relais change run that would fork a worktree of its
//! own.
//!
//! relais works in place in a linked git worktree and forks a worktree
//! anywhere else. Started from the primary checkout, or asked for `fork` or
//! `isolated` inside a linked worktree, a change run duplicates a worktree the
//! person already has. Like `screenshot-in-main-session`, `examine` never fires
//! (the backtester sees only Bash); the hook's `PreToolUse` path for the relais
//! tool calls [`judge`] and [`phrase`].

use crate::rules::{Evidence, Examine, Finding, Rule, Stance, Trend};
use crate::shell::Parsed;

pub const RULE: Rule = Rule {
    id: "relais-run-forks",
    prevents: "A relais change run forking a worktree of its own where one already exists",
    irreversible: false,
    default_stance: Stance::Advise,
    max_stance: Stance::Deny,
    evidence: Evidence {
        // 19 forking change runs of 528 runs that resolved a workspace; the
        // per-1,000 rate of tool calls was not computed.
        per_1000: 0.0,
        measured: "2026-11-14",
        trend: Trend::Rare,
    },
    examine: Examine::Legacy(examine),
    confirm: None,
};

/// The tool this rule is about.
pub const TOOL: &str = "mcp__relais__run";

fn examine(_parsed: &Parsed) -> Option<Finding> {
    None
}

/// What kind of task the run carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Change,
    Other,
}

/// Where the run's `cwd` sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkout {
    Primary,
    Linked,
}

/// Why the advice fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fire {
    /// A linked worktree asked for `fork` or `isolated`.
    Second,
    /// The primary checkout.
    Primary,
}

impl Fire {
    pub fn label(self) -> &'static str {
        match self {
            Fire::Second => "second",
            Fire::Primary => "primary",
        }
    }
}

/// The workspace values relais knows.
fn known(workspace: &str) -> bool {
    matches!(workspace, "auto" | "fork" | "in-place" | "isolated")
}

/// Whether a run at `checkout` with `workspace` (absent = `auto`) would fork.
/// A value relais does not know, and `in-place`, are silent.
pub fn fork_of(checkout: Checkout, workspace: Option<&str>) -> Option<Fire> {
    let workspace = workspace.unwrap_or("auto");
    if !known(workspace) || workspace == "in-place" {
        return None;
    }
    match (checkout, workspace) {
        (Checkout::Primary, _) => Some(Fire::Primary),
        (Checkout::Linked, "fork" | "isolated") => Some(Fire::Second),
        (Checkout::Linked, _) => None,
    }
}

/// The whole table: silent unless the task is a change run that would fork.
pub fn judge(kind: Kind, workspace: Option<&str>, checkout: Checkout) -> Option<Fire> {
    match kind {
        Kind::Change => fork_of(checkout, workspace),
        Kind::Other => None,
    }
}

/// The advice for one fire.
pub fn phrase(fire: Fire, workspace: Option<&str>, cwd: &str) -> String {
    match fire {
        Fire::Second => crate::decision::phrase(
            RULE.id,
            &format!(
                "workspace \"{}\" makes a second worktree beside this one.",
                workspace.unwrap_or("auto")
            ),
            "Leave workspace unset; relais edits a linked worktree in place.",
        ),
        Fire::Primary => crate::decision::phrase(
            RULE.id,
            &format!(
                "{cwd} is in the primary checkout, so this change run would fork a worktree of \
                 its own."
            ),
            "Start a worktree with the worktree-task skill, pass its path as cwd, and leave \
             workspace unset.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_table() {
        use Checkout::{Linked, Primary};
        for workspace in [None, Some("auto"), Some("fork"), Some("isolated")] {
            assert_eq!(judge(Kind::Change, workspace, Primary), Some(Fire::Primary));
        }
        assert_eq!(judge(Kind::Change, None, Linked), None);
        assert_eq!(judge(Kind::Change, Some("auto"), Linked), None);
        assert_eq!(
            judge(Kind::Change, Some("fork"), Linked),
            Some(Fire::Second)
        );
        assert_eq!(
            judge(Kind::Change, Some("isolated"), Linked),
            Some(Fire::Second)
        );
        for checkout in [Primary, Linked] {
            assert_eq!(judge(Kind::Change, Some("in-place"), checkout), None);
            assert_eq!(judge(Kind::Change, Some("bogus"), checkout), None);
            assert_eq!(judge(Kind::Other, Some("fork"), checkout), None);
        }
    }

    #[test]
    fn the_texts_are_the_plans() {
        assert_eq!(
            phrase(Fire::Primary, None, "/r"),
            "greenpiste/relais-run-forks: /r is in the primary checkout, so this change run \
             would fork a worktree of its own. Start a worktree with the worktree-task skill, \
             pass its path as cwd, and leave workspace unset."
        );
        assert_eq!(
            phrase(Fire::Second, Some("fork"), "/r"),
            "greenpiste/relais-run-forks: workspace \"fork\" makes a second worktree beside this \
             one. Leave workspace unset; relais edits a linked worktree in place."
        );
    }
}
