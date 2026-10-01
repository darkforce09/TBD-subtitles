**Status:** live

# Commit conventions

How changes are committed: message format, what belongs in one commit, and the checks to run
first.

## Message

Conventional Commits: `type(scope): lowercase summary`, at most 72 characters, no trailing period.

- **Types:** `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `build`, `chore`.
- **Scopes:** the area touched — `gui`, `pipeline`, `media-io`, `inference`, `stages`,
  `subtitle-formats`, `job-model`, `child-process`, `worker-channel`, `repo-gates`, `docs`,
  `workspace`. Omit the scope when the change spans the repository.
- **Body:** two to four wrapped lines on what changed and why.
- **Trailer:** AI-authored commits end with a `Co-Authored-By:` line naming the model.

```text
feat(stages): align adjudicated text with ctc viterbi

Re-aligns only the utterances whose text changed after adjudication, in
20-60 s blocks cut at silences, and falls back to the backbone's word times
when a block fails its checks.

Co-Authored-By: <model name> <noreply@anthropic.com>
```

## What goes in one commit

- One logical change, with the documentation it affects and the roadmap boxes it ticks.
- Stage files with explicit paths; never `git add -A` or `git add .`.
- Never commit media, model files, work folders or machine-local configuration (see `.gitignore`).

## Branches

Commit straight to `main`. Short-lived branches only for experiments that may be thrown away.

## Before committing

All of these pass, run from the repository root:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo gates
```

`cargo gates` judges the tracked files; run `cargo gates --with-untracked` to include new files
before they are staged.
