use super::*;

fn tree() -> TrackedTree {
    TrackedTree::from_listing(
        "documentation/runbooks/deploy.md\0documentation/My Notes/plan.md\0\
         apps/tbd_subtitles/src/main.rs\0README.md",
    )
}

fn checkout(path: &str, plain_view: bool, fragment: Option<&str>) -> Destination {
    Destination::CheckoutPath {
        path: path.to_string(),
        plain_view,
        fragment: fragment.map(str::to_string),
    }
}

#[test]
fn a_fragment_alone_names_the_same_document() {
    assert_eq!(
        classify("#getting-started"),
        Destination::SameDocument {
            fragment: "getting-started".to_string()
        }
    );
}

#[test]
fn a_checkout_path_keeps_its_query_view_and_fragment() {
    assert_eq!(
        classify("../api/main.rs#L10-L20"),
        checkout("../api/main.rs", false, Some("L10-L20"))
    );
    assert_eq!(
        classify("guide.md?plain=1#L4"),
        checkout("guide.md", true, Some("L4"))
    );
    assert_eq!(classify("guide.md#"), checkout("guide.md", false, None));
    assert_eq!(
        classify("main.rs:12"),
        checkout("main.rs:12", false, None),
        "a dotted name before a colon is a path, not a scheme"
    );
}

#[test]
fn other_schemes_and_hosts_are_external() {
    for destination in [
        "https://example.com/x",
        "http://docs.rs/regex",
        "mailto:someone@example.com",
        "vscode://file/x",
        "//cdn.example.com/lib.js",
        "https://github.com/someone/project/blob/main/x.md",
    ] {
        assert_eq!(
            classify(destination),
            Destination::External,
            "{destination}"
        );
    }
}

#[test]
fn a_fragment_asks_rendered_markdown_for_an_anchor_and_text_for_lines() {
    assert_eq!(
        fragment_need("docs/guide.md", false, "Setup%20steps"),
        FragmentNeed::Anchor("Setup steps".to_string())
    );
    assert_eq!(
        fragment_need("docs/guide.md", true, "L4-L9"),
        FragmentNeed::Lines(4, 9)
    );
    assert_eq!(
        fragment_need("src/main.rs", false, "L2"),
        FragmentNeed::Lines(2, 2)
    );
    assert_eq!(
        fragment_need("src/main.rs", false, "main"),
        FragmentNeed::Unmatchable("main".to_string())
    );
    assert_eq!(
        decode_fragment("%FF"),
        "%FF",
        "an undecodable fragment stays as written"
    );
    assert_eq!(
        unmatchable_anchor_problem("src/main.rs", "main"),
        "`src/main.rs` is not rendered Markdown, so `main` matches nothing; only a #L<n> or \
         #L<n>-L<m> line anchor applies"
    );
}

#[test]
fn a_path_resolves_from_the_root_or_from_the_document_folder() {
    let tree = tree();
    let document = "documentation/runbooks/README.md";
    assert_eq!(
        resolve(document, "deploy.md", &tree),
        Resolution::File("documentation/runbooks/deploy.md".to_string())
    );
    assert_eq!(
        resolve(document, "./deploy.md", &tree),
        Resolution::File("documentation/runbooks/deploy.md".to_string())
    );
    assert_eq!(
        resolve(document, "/apps/tbd_subtitles/src/main.rs", &tree),
        Resolution::File("apps/tbd_subtitles/src/main.rs".to_string())
    );
    assert_eq!(
        resolve(document, "../../README.md", &tree),
        Resolution::File("README.md".to_string())
    );
    assert_eq!(
        resolve(
            "README.md",
            "apps//tbd_subtitles/./src/../src/main.rs",
            &tree
        ),
        Resolution::File("apps/tbd_subtitles/src/main.rs".to_string())
    );
}

#[test]
fn a_folder_holding_a_tracked_file_is_a_target() {
    let tree = tree();
    assert_eq!(
        resolve("README.md", "/apps/tbd_subtitles/", &tree),
        Resolution::Folder("apps/tbd_subtitles".to_string())
    );
    assert_eq!(
        resolve("README.md", "/", &tree),
        Resolution::Folder(String::new())
    );
}

#[test]
fn percent_escapes_decode_before_the_lookup() {
    let tree = tree();
    assert_eq!(
        resolve("README.md", "documentation/My%20Notes/plan.md", &tree),
        Resolution::File("documentation/My Notes/plan.md".to_string())
    );
    assert_eq!(percent_decode("a%2Fb%zz%4"), Some("a/b%zz%4".to_string()));
    assert_eq!(percent_decode("%C3%A9t%C3%A9"), Some("été".to_string()));
    assert_eq!(percent_decode("%FF"), None);
}

#[test]
fn an_untracked_or_absent_path_is_missing_and_climbing_out_escapes() {
    let tree = tree();
    assert_eq!(
        resolve("documentation/runbooks/deploy.md", "rollback.md", &tree),
        Resolution::Missing("documentation/runbooks/rollback.md".to_string())
    );
    assert_eq!(
        resolve(
            "documentation/runbooks/deploy.md",
            "../../../outside.md",
            &tree
        ),
        Resolution::EscapesRepository
    );
    assert_eq!(
        resolve("README.md", "/../README.md", &tree),
        Resolution::EscapesRepository
    );
    assert_eq!(
        resolve("README.md", "%FF.md", &tree),
        Resolution::Missing("%FF.md".to_string())
    );
}

#[test]
fn line_anchors_read_one_line_or_a_range() {
    assert_eq!(line_anchor("L7"), Some((7, 7)));
    assert_eq!(line_anchor("L10-L20"), Some((10, 20)));
    for other in ["L", "l7", "L7-20", "L7-", "section", "L1x", "L-3"] {
        assert_eq!(line_anchor(other), None, "{other}");
    }
}

#[test]
fn a_viewer_numbers_every_line_including_a_last_one_without_a_break() {
    assert_eq!(line_count(""), 0);
    assert_eq!(line_count("one"), 1);
    assert_eq!(line_count("one\n"), 1);
    assert_eq!(line_count("one\ntwo"), 2);
    assert_eq!(line_count("one\n\n"), 2);
}

#[test]
fn a_line_anchor_must_fit_the_file() {
    assert_eq!(line_anchor_problem(3, 3, 3), None);
    assert_eq!(line_anchor_problem(1, 3, 3), None);
    assert_eq!(
        line_anchor_problem(4, 4, 3).as_deref(),
        Some("the file has 3 line(s)")
    );
    assert_eq!(
        line_anchor_problem(0, 2, 3).as_deref(),
        Some("line numbers start at 1")
    );
    assert_eq!(
        line_anchor_problem(3, 2, 3).as_deref(),
        Some("the range ends at 2 before it starts at 3")
    );
}
