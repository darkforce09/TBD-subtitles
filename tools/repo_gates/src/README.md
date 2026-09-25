# Gate runner source

The source of the `cargo gates` binary: its command line, the machinery every gate shares, the
gates themselves, and the one module that names where things live in this repository.

## Contents

```text
tools/repo_gates/src/
├── cli.rs     the clap command line: the `Gate` names in run order and the arguments every gate takes
├── gate_run/  shared machinery: the tracked-file listing, the `--path` scope, path regions, the run
├── gates/     one module per gate, grouped into documentation, source and workspace gates
├── layout.rs  the repository's places: code trees, documentation root, frozen folders, crate layers
└── main.rs    finds the workspace root, runs the chosen gates and exits with the worst status
```

## How it works

`main.rs` parses the command line (`cli.rs`), finds the workspace root, and hands each chosen
`Gate` to `gates::run` together with the `GateRequest` built from `--path` and
`--with-untracked`. Most gates are a `FileRule` (judge one file at a time) run by
`gate_run::file_rule`; README coverage, Markdown placement and the link check judge folders or
whole document sets and drive `gate_run` themselves. Every gate reads the repository's places from
`layout.rs` alone, so a folder that moves changes one constant.

```text
main.rs ── Cli::parse ── repository_root ──┐
                                           ▼
                 gates::run(gate, root, request) ──> gate_run::prepare (git ls-files, --path scope)
                                           │                     │
                                           ▼                     ▼
                      one verdict per file, folder or document ──> GateRun::print → 0 / 1 / 2
```

`cli.rs` is also read by the link check, which walks this command tree to judge every
`cargo gates` command a document cites, so the command line and the documents cannot drift apart.

## Commands

### `cargo gates`

Synopsis: `cargo gates [<gate>] [--report] [--path <dir>]... [--with-untracked]`

Runs the named gate, or every gate in the order below when none is named, over the tracked files.

| Gate | What it holds |
|---|---|
| `language-bans` | every tracked file is Rust, Markdown, TOML or JSON (plus `Cargo.lock`, git and editor configuration, subtitle fixtures in `tests/fixtures/`); no shebang names a shell, Python or Node |
| `file-length` | production Rust files stay under 500 lines, test files under 1000 |
| `module-headers` | every `lib.rs`, `main.rs` and production file of 80 lines or more opens with the Role, Position, Signals and state, Invariants header |
| `test-placement` | unit tests live in sibling files under `tests/`, never inline |
| `prose-rules` | no ticket ids; no history words outside the decision log; no milestone ids in Rust files and code READMEs |
| `editorconfig` | UTF-8, LF line ends, a final newline, no trailing whitespace outside Markdown |
| `crate-layering` | a product crate depends only on lower layers; a tool only on the crates listed for it |
| `readme-coverage` | every folder in the README span has a README.md whose Contents block lists the folder exactly |
| `readme-sections` | every README holds the core `##` sections in order, with the three Boundaries bullets |
| `markdown-placement` | code trees hold no Markdown but README.md; live documents stay at or under 500 lines |
| `status-lines` | every document under the documentation root opens with a valid status line |
| `link-check` | every link, backticked repository path and cited `cargo gates` command exists |

Arguments:

- `--path <dir>`: judge only what lies at or under this repository-relative folder; repeatable.
  `.` or the checkout root means the whole repository, the default. A value that climbs out with
  `..`, lies outside the checkout, names a file or names no tracked folder is refused with exit 2.
- `--with-untracked`: also judge the untracked files git does not ignore, exactly like tracked
  ones; the summary line says the run included them.
- `--report`: the link check prints every break in full instead of the first 20.

Exit codes: 0 when every check of every gate run held; 1 when a check found a violation; 2 when a
check could not run (no workspace root, a failed listing, an unreadable file, a refused or empty
scope). Running every gate exits with the worst status.

Example:

```bash
cargo gates readme-coverage --path tools --with-untracked
```

## Boundaries

- Depends on: `tools/verification_core` (verdicts, the report, `proc::Run` for git); `clap`,
  `regex`, `syn`, `proc-macro2` and `toml`; the `git` program.
- Used by: the `gates` alias in `.cargo/config.toml`; nothing imports these modules.
- Rules:
  - the gate names are the kebab-case `Gate` variants in run order, and the link check accepts
    exactly those (`the_gates_tree_resolves_its_own_commands`);
  - every gate takes a repeatable `--path` and `--with-untracked`
    (`every_gate_takes_a_repeatable_path_and_the_untracked_flag`), and only the link check reads
    `--report` (`the_gate_takes_the_report_flag_beside_the_gate_arguments`);
  - the top-level places (code trees, documentation root, frozen folders, project instructions)
    are spelled only in `layout.rs`, and every gate reads them from there.

## Related documentation

- [Gate runner](/tools/repo_gates/README.md) — the crate, its configuration and how to run it.
- [README standard](/documentation/standards/readme_standard.md#gates) — the README gates and
  what each checks.
