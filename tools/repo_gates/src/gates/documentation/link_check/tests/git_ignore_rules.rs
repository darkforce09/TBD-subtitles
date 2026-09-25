use std::path::PathBuf;
use std::process::Command;

use super::*;

fn owned(paths: &[&str]) -> Vec<String> {
    paths.iter().map(ToString::to_string).collect()
}

/// A fresh git work tree under the system temporary folder with `ignore_file` as its
/// `.gitignore`, removed when dropped.
struct ScratchRepository {
    root: PathBuf,
}

impl ScratchRepository {
    fn new(tag: &str, ignore_file: &str) -> ScratchRepository {
        let root = std::env::temp_dir().join(format!(
            "documentation-gates-ignore-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the scratch repository");
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .expect("git runs");
        assert!(status.success(), "git init");
        std::fs::write(root.join(".gitignore"), ignore_file).expect("write .gitignore");
        ScratchRepository { root }
    }
}

impl Drop for ScratchRepository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn the_answer_is_the_paths_git_names_back() {
    let asked = owned(&["build/out.bin", "notes/plan.md", "build/"]);
    let ignored = parse_ignored("build/out.bin\0build/\0", &asked).expect("a clean answer");
    assert_eq!(
        ignored.into_iter().collect::<Vec<_>>(),
        ["build/", "build/out.bin"]
    );
    assert!(
        parse_ignored("", &asked)
            .expect("an empty answer")
            .is_empty()
    );
}

#[test]
fn a_path_git_was_not_asked_about_makes_the_answer_untrustworthy() {
    let asked = owned(&["build/out.bin"]);
    assert_eq!(
        parse_ignored("build/out.bin\0elsewhere/x\0", &asked),
        Err("git named `elsewhere/x`, which it was not asked about".to_string())
    );
}

#[test]
fn a_real_checkout_answers_every_path_in_one_batch() {
    let repository = ScratchRepository::new("answers", "/target/\nscratch/\n*.log\nsecrets.env\n");
    let rules = GitIgnoreRules::new(&repository.root);
    let asked = owned(&[
        "target/debug/xtask",
        "assets/scratch/everon/tile.bin",
        "assets/scratch/",
        "logs/run.log",
        "deploy/secrets.env",
        "apps/gone.rs",
        "apps/gone/",
    ]);
    let ignored = rules.ignored(&asked).expect("git answers");
    assert_eq!(
        ignored.into_iter().collect::<Vec<_>>(),
        [
            "assets/scratch/",
            "assets/scratch/everon/tile.bin",
            "deploy/secrets.env",
            "logs/run.log",
            "target/debug/xtask",
        ],
        "every path below an ignored folder, the folder itself, and each matching file"
    );
}

#[test]
fn a_batch_git_ignores_nothing_of_answers_an_empty_set() {
    let repository = ScratchRepository::new("nothing", "/target/\n");
    let rules = GitIgnoreRules::new(&repository.root);
    let ignored = rules
        .ignored(&owned(&["apps/gone.rs", "docs/old/"]))
        .expect("git exits 1 when nothing is ignored, which is an answer");
    assert!(ignored.is_empty());
    assert!(
        rules
            .ignored(&[])
            .expect("no question needs no git")
            .is_empty()
    );
}

#[test]
fn a_checkout_git_cannot_read_did_not_run() {
    let missing = std::env::temp_dir().join(format!(
        "documentation-gates-ignore-missing-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&missing);
    let rules = GitIgnoreRules::new(&missing);
    assert!(rules.ignored(&owned(&["apps/x.rs"])).is_err());
}

#[test]
fn a_failed_batch_names_the_command_and_its_status() {
    match check_problem(128, "fatal: not a git repository") {
        NotRun::ToolError {
            tool,
            status,
            stderr,
        } => {
            assert_eq!(tool, "git check-ignore --stdin -z");
            assert_eq!(status, 128);
            assert_eq!(stderr, "fatal: not a git repository");
        }
        other => panic!("a tool error, not {other:?}"),
    }
}
