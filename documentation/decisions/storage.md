**Status:** live

# Decisions: storage

The decisions about where a job keeps what its steps write and how it is read back: the job
database, the archived values, which process may open it, and the library of signs shared by
episodes. The [decision log](/documentation/decisions/) says how entries are written; the
fingerprinted steps these build on are in
[stack and pipeline](/documentation/decisions/stack_and_pipeline.md).

### 2026-09-30 — Step outputs live in one redb database per job, archived with rkyv, owned by one process

**Context:** Every step wrote a JSON file (`<name>.part`, then a rename) into the job's work
directory, and `job.json` held each step's fingerprint; a step was reused while `job.json` held its
fingerprint and its outputs existed. The next visual steps need data per frame of every sign (a
position, a follow score and an erase mask, the read-back check's readings) for videos up to two
hours at 60 fps, about 432,000 frames: a file per sign per frame or a JSON document parsed whole to
read one frame does not scale. The owner approved the
[binary storage plan](/documentation/architecture/binary_storage_plan.md). The pinned redb release
was measured with a second process
([findings](/documentation/research/redb_multi_process.md)): with default features a read-write
open locks the file exclusively and a read-only open shared, so while one process writes, every
other open, from another process or from a second handle in the same process, fails at once with
`DatabaseAlreadyOpen`; a file a killed writer left opens read-write (repairing it, keeping every
commit) but refuses read-only opens until then. The `experimental-multiprocess` feature's
`SingleWriter` mode lets readers open beside a writer, but belongs to the redb 5 API preview. The
app is in beta: no job needs to survive a format change.

**Decision:**

- Each job keeps every step's output, the job record, the step records and the owner's corrections
  in one `redb` database, `work/<job>/job.redb`, with the tables `meta`, `step_records`,
  `outputs`, `corrections`, and the per-frame tables `frames` and `readings` keyed by
  (occurrence id, frame number). Large media (audio streams, stills, crops, plates, patches) stay
  files, written and synced before the record that names them commits.
- Values are `rkyv` archives of the `job_model` types, read in place. `rkyv` is built with
  `unaligned` so an archive is read straight from the slice redb returns, and with `little_endian`
  and `pointer_width_32` named so that no other crate's features can change the stored format
  (a `usize` archives as 32 bits; the contract types hold only counts and indices in them).
  `PathBuf` fields keep their Rust type and archive as UTF-8 strings (`rkyv::with::AsString`); a
  path that is not UTF-8 is an error. The types keep their serde derives for `tbd-subtitles dump`,
  which prints a record as JSON.
- redb runs with default features (`ExclusiveWriter`), pinned at 4.3.0; rkyv is pinned at 0.8.18.
- One process owns a job's database: the process running the job opens it read-write once for the
  whole job and names itself in `job.lock`. GPU workers (law 7) never open it: they return their
  output on stdout or in `work/<job>/exchange/`, and the runner validates and commits it. The window
  reads and writes the jobs it runs through its own handle; a job another process owns is shown as
  busy until that process exits. A job no process runs is opened read-write, which repairs a file a
  crash left.
- Law 6 reads: each step commits its output and its record in one transaction, and is skipped
  while its record's revision and input fingerprint are current. Every table's layout has a version
  in `meta` that is part of every fingerprint; a rerun clears a step, every step that reads it and
  their per-frame rows in one transaction.
- Approved signs go into `library.redb` beside the work folders, keyed by the normalised Japanese
  and a perceptual hash of the keyframe crop, shared by every episode. Because a second open fails
  at once and a terminal run and the window may run together, a process opens the library for one
  transaction at a time and retries while another process holds it.
- There is no JSON fallback, importer or migration: a job from before the database, or one whose
  stored layout differs, reruns; Git keeps the old code.

**Consequences:** `job_model` depends on `rkyv` and every contract type derives its `Archive`,
`Serialize` and `Deserialize` with a round-trip test that reads from a misaligned buffer (`cargo
test -p job_model`). A field added to or changed in a type changes its layout, which nothing
detects by itself: whoever changes a contract type bumps the layout version of the tables that
store it, and the steps that stored it rerun. `tools/redb_process_probe` records the redb
behaviour and runs its own executable as child processes; rerun it when the redb pin moves.
Phase 1 changes no step: the store, the move of each step and the per-frame tables follow in the
plan's later phases, and until then steps still write JSON files. Reading a job while another
process runs it is not possible on stable redb; `SingleWriter` is revisited when redb 5 makes it
stable.

**Supersedes:** 2026-09-26 — Stages run as fingerprinted steps (the fingerprints stay; where they
are kept and when a step counts as done change: a step's record in `job.redb`, not `job.json`
and its output files).
