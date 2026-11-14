---
status: active
branch: feat/relais-run-forks
repos: [amont-agent]
adrs: []
---
# A relais change run stays in its worktree

## Review panel
👉 **Decide:** none — approve if a greenpiste advice (deny after a matched week) is enough to stop relais change runs forking a worktree.
📄 Full reviews: [2026-11-14-relais-change-run-stays-worktree.reviews.md](2026-11-14-relais-change-run-stays-worktree.reviews.md)
📍 amont-agent · plan reviewed, nothing built · next: worktree after feat/greenpiste-ships merges, then Phase 1. Panel: backend, lang:rust, tui, unix (opus), codex.
**Changed by review:** path probe canonicalized and bounded; held fire dropped (relais refuses held runs itself); unreadable contracts silent, never denied.
**Verdicts:** round 1 4× approve-with-changes; round 2 3× approve, 1× approve-with-changes; codex approve-with-changes (2 high applied); final delta 4× approve; 10 carried into implementation.

## Picture
```
Claude Code: PreToolUse(mcp__relais__run {cwd, task, workspace})
        |
        v
payload.rs --> Event::PreRelaisRun
        |
        v
hook.rs on_relais_run
   kind inspect|verify|unknown? ---------- yes --> silent
   workspace == "in-place"? -------------- yes --> silent (relais checks)
   no repo, bare, or git probe failed? --- yes --> silent
   cwd a linked worktree?
      workspace fork|isolated -----------------> FIRE "second worktree"
      otherwise -------------------------------> silent (relais works in
                                                 place, or refuses if held)
   cwd the primary checkout
      (auto|fork|isolated) --------------------> FIRE "primary checkout"
        |
        v
journal + Decision::Advise (default) / Deny (once promoted)
```
Every path that would give a change run a worktree of its own ends in one
of the two FIRE lines; everything else is silent.

## Context
relais works in place in a linked git worktree and forks a worktree of its
own anywhere else (relais ADR-0008). Started from a `worktree-task`
worktree, it never duplicates one; started from the primary checkout, it
does. The person asked for a guard (2026-11-14) and chose it over a relais
`linked-only` mode: the guard belongs to greenpiste, inspection runs must
keep forking, and a relais mode would need an ADR revising ADR-0008.

Baseline, relais ledger and run events since 2026-10-31: 528 runs resolved
a workspace, 60 forked. 41 were `report` (inspection, left alone), 19 were
change runs: 13 from a primary checkout (sre-agent 7 feature, homelab 3
infra, relais 3 review-fix), 6 from a linked worktree. An `auto` run never
forks a linked worktree: when another in-place run holds it, relais refuses
the run itself (`WorkspaceBusy` / `WorkspaceHasInterruptedRun`, relais
`runner/workspace_ops.rs:27-41`), so those 6 asked for `fork` or `isolated`.
The 5 `workspace_seed_skipped … is held by in-place run` events explain a
skipped seed, not the fork. The rule targets all 19.

## Goal
When a `mcp__relais__run` change run would make its own worktree, greenpiste
says why and what to do: start a `worktree-task` worktree, or leave
`workspace` unset in one. Advise first; deny after a week's audit.

## Non-goals
- No relais change, and no `linked-only` relais mode (revisited only if the
  week's count finds forks this hook never saw).
- No Bash `relais run` matching: `relais run --help` says a run starts only
  from Claude Code with the relais plugin.
- `RELAIS_WORKSPACE` and `CI` in the plugin's environment are not read: the
  hook cannot see the plugin's environment.
- Inspection (`kind: inspect`) and verify (`kind: verify`) runs, replays and
  holdout copies: their own worktrees are by design.
- No deny in this plan: the switch is a stance change after the count.

## Code review
- amont-agent: backend, lang:rust, tui, unix

## Behaviour
PreToolUse, `tool_name == "mcp__relais__run"`, `tool_input = {cwd, task, workspace?}`.
`cwd` is the tool's `cwd` argument, never the session's.

- **kind.** `task.kind`; an object with no `kind` counts as `change`. When
  `task` is a string (a contract path), a relative path is resolved against
  `cwd`; it is read only if it is a regular file (a FIFO or device would
  block), through `take(64 KiB + 1)`, and parsed. Too large, not a regular
  file, unreadable or unparsable is kind `unknown`: silent at every stance,
  journalled `relais run kind_unknown`, so a valid inspection contract the
  hook cannot read is never denied.
- **Linked vs primary.** One `git -C <cwd> rev-parse --path-format=absolute
  --is-bare-repository --git-dir --git-common-dir` through
  `git::stdout_bounded` (deadline, `GIT_TERMINAL_PROMPT=0`,
  `GIT_OPTIONAL_LOCKS=0`); both paths canonicalized before comparing, so a
  cwd reached through a symlink (`/tmp`, `/var`) or a subdirectory compares
  right. Different → linked. Bare → silent. Timeout or git failure fails
  open: silent, journalled `relais run probe_failed`.
- **Held worktrees are relais's.** An `auto` run in a linked worktree another
  in-place run holds is refused by relais itself, with its own remedy
  (`relais status <run>`, `relais resume --retire <run>`); the hook reads no
  relais marker.

| input | result |
|---|---|
| kind `inspect`, `verify` or `unknown` | silent |
| `workspace: "in-place"` | silent |
| cwd missing, not in a git repository, or bare | silent |
| linked, workspace absent/`auto` | silent |
| linked, workspace `fork` or `isolated` | fire (second): `workspace "<w>" makes a second worktree beside this one. Leave workspace unset; relais edits a linked worktree in place.` |
| primary, workspace absent/`auto`/`fork`/`isolated` | fire (primary): `<cwd> is the primary checkout, so this change run would fork a worktree of its own. Start a worktree-task worktree and pass its path as cwd.` |
| any fire, stance `observe` | journalled, silent |
| any fire, stance `advise` (default) | advise, every time (rare: 19 in two weeks) |
| any fire, stance `deny` | deny with the same text |

Each text is prefixed by `crate::decision::phrase` with `<name>/relais-run-forks: `,
where `<name>` is `amont-agent` today and `greenpiste` once plan 3b-1's names
module lands. Subagent calls are judged the same (`agent_id` is not an
exemption). Journal excerpt: `relais run <primary|second> <workspace>`,
no paths.
Install: one PreToolUse block, matcher `mcp__relais__run`, idempotent.
Rollback: set the stance to `observe`, or remove the block.

## Phases
- [x] Phase 1 — rule, payload, dispatch, install target:
  `crates/amont-agent-guard/src/rules/relais_run_forks.rs` (RULE: default
  `advise`, max `deny`, evidence 19 forks / 528 runs measured 2026-11-14;
  `TOOL`; `phrase_*`; a pure `judge(kind, workspace, checkout) -> Option<Fire>`),
  `crates/amont-agent-guard/src/rules/mod.rs`, `src/rule_table.rs`
  (+1 rule and the count test), `crates/amont-agent-guard/src/payload.rs`
  (`Event::PreRelaisRun`, beside the screenshot arm),
  `crates/amont-agent-guard/src/git.rs` (linked/primary probe built on
  `stdout_bounded`), `src/hook.rs` (`on_relais_run`, modelled on `on_grep`'s
  stance match with `stance::resolve(&RULE)`),
  `crates/amont-agent-guard/src/settings.rs` (TARGETS entry after the
  screenshot one), `crates/amont-agent-guard/src/corpus.rs` +
  `crates/amont-agent-guard/tests/corpus/relais-run-forks.cases`.
- [x] Phase 2 — tests: unit tests on `judge` for every table row;
  `tests/hook.rs` with a temp repo plus `git worktree add`: primary auto →
  advises; linked auto → silent; linked fork → advises; linked auto with a
  relais in-place marker present → silent; inspect in primary → silent;
  contract path with `kind: inspect` → silent (relative path too);
  unreadable, malformed, non-regular and oversized contract paths → silent
  with `kind_unknown`, an oversized one under stance `deny` too; cwd through a symlink
  and from a subdirectory of the primary → advises; bare repo, cwd that does
  not exist, cwd outside any repository → silent; stance `observe` → silent;
  stance `deny` → deny for primary and second; two primary calls in one
  session → two advisories and two journal rows (fails if `first_telling` is
  copied from `on_screenshot`); primary with an `agent_id` → advises.
  `tests/settings_merge.rs` block counts.
- [x] Phase 3 — docs and release notes: `docs/rules.md` (row, `advise`,
  ceiling `deny`), `CHANGELOG.md` (Unreleased → Added). Release via
  `tag-release`; the install is the person's.

## Decision log
- 2026-11-14 — The worktree starts from `origin/main` after
  `feat/greenpiste-ships` (plan 3b-1, crate and binary rename, 299 files)
  merges, so this branch does not conflict with it; paths above are named
  as today, and become `crates/greenpiste-guard/...` if 3b-1 lands first.
- 2026-11-14 — Advise every time, not once per session: each fire is the
  overlap itself, and it happens about 10 times a week.
- 2026-11-14 — A guard, not a relais mode, because the person agreed the
  rule is greenpiste's and inspection runs keep forking.
- 2026-11-14 — ADR-0064 (`harness.extensions`): relais is an optional
  extension. The hook reads only its tool name and input schema (relais
  SPEC.md:359 is the pinned source), never its files; an unknown input
  shape goes silent, never an error.
- 2026-11-14 — tui joins the code review: the advice texts are the rule's
  whole output, and round 1 found two of them unfollowable.
- 2026-11-14 — The held fire is dropped (Codex, final review): relais
  refuses an `auto` run in a held worktree itself (`workspace_ops.rs:27-41`)
  and never forks it, so the advice claimed a fork that does not happen and
  coupled the hook to relais's marker layout.
- 2026-11-14 — An unreadable contract is `unknown` and silent (Codex): an
  oversized but valid inspection contract must never be denied.
- 2026-11-14 — Implemented by relais run run-6603e92b3b99c-1fa9 (in place in
  this worktree, accepted), with the 10 findings carried from review; paths
  are `crates/greenpiste-guard/...`, since 3b-1 (#254) merged first.
- 2026-11-14 — Implementation review changed the primary text to `<dir> is
  in the primary checkout, so this change run would fork a worktree of its
  own. Start a worktree with the worktree-task skill, pass its path as cwd,
  and leave workspace unset.`, with `<dir>` the resolved run directory
  (tui: a `sub/` or `.` cwd was named "the primary checkout"). A bare repo
  is journalled `bare`; the probe runs under `LC_ALL=C`; the journal names
  the run's repository, not the session's.

## Verification
- Phase 1–2, the debug binary fed real PreToolUse JSON with a scratch
  `CLAUDE_CONFIG_DIR` and fresh `session_id`: cwd = a scratch repo's
  primary → advise text after the prefix equals the table byte for byte,
  prefix as `decision::phrase` gives it; cwd = its linked worktree → empty
  stdout; same with `workspace: "fork"` → the second-worktree text;
  `kind: "inspect"` at the primary → empty; a cwd that does
  not exist, and one outside any repository → empty; the primary reached
  through `/tmp` and from a subdirectory → the primary text. Exit 0 and
  0 B stderr every case; journal line per fire. Would fail if the arm is
  missing (every case empty).
  Observed (`target/debug/greenpiste hook`, scratch repo under `~/.cache`):
  primary auto, primary `fork`, primary through a symlink and from `sub/` →
  the primary text with `greenpiste/` prefix, `jq -e .` ok; linked `fork`
  and `isolated` → the second text naming the value; linked auto, linked
  auto with `relais-inplace/run-x.json`, `in-place`, `bogus`, `kind:
  inspect` (object and relative path), a non-repo dir and a bare repo →
  empty. 14 cases plus one extra primary call piped to `jq -e .`, every one
  exit 0, 0 B stderr. Journal: 5 primary `advised` rows (the 4 primary
  cases and the `jq` call) + 2 second, 1 `not_a_repo` row, none for the
  silent in-repo cases. (The plan's "through `/tmp`" case was run as a symlink to the
  primary, which is what it tests.)
- Install: `install --write` twice on a scratch copy of the real
  `settings.json` → one new block, second run no diff, real file untouched.
  Observed: rc 0 twice, one PreToolUse block with matcher
  `mcp__relais__run`, second run "already current — nothing written" and
  `diff -q` equal, real `~/.claude/settings.json` sha unchanged.
- Suite: `cargo clippy --workspace --all-targets -- -D warnings`;
  `cargo test --workspace --locked`.
- After the person's install (follow-up, not a pending row): one real
  relais change run from a primary checkout shows the advice. After seven
  days, each fire is matched to its relais run (session, repo, time) and
  its resolved workspace (fork, isolated, in place, refused), and every
  change run that resolved `fork` or `isolated` is matched to a fire.
  Counted: false fires, missed forks, and fires followed by an in-place
  retry. Missed forks reopen the relais-mode question; `deny` needs a
  non-empty sample with no false fire, not just zero misses.

<!-- panel: repos=amont-agent reviewers=backend,lang:rust,tui,unix,codex body-sha=1bfa7c73048b -->
