//! Global ignore rules must work when a hook drains a checkout outside its cwd.

use mcp_client::{ContextStreamClient, TargetedFileDecision};

#[test]
fn targeted_ingest_applies_global_ignores_outside_process_cwd() {
    // This integration-test process contains only this test, so configuring its
    // Git environment cannot race other tests or change the operator's config.
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("checkout");
    std::fs::create_dir_all(root.join("ignored-directory")).unwrap();
    std::fs::write(root.join("keep.rs"), "fn keep() {}\n").unwrap();
    std::fs::write(root.join("ignored.rs"), "fn ignored() {}\n").unwrap();
    std::fs::write(root.join("ignored-directory/nested.rs"), "fn nested() {}\n").unwrap();
    let excludes = fixture.path().join("global-ignore");
    std::fs::write(&excludes, "ignored.rs\nignored-directory/\n").unwrap();
    let git_config = fixture.path().join("git-config");
    let excludes = excludes.to_string_lossy().replace('\\', "/");
    std::fs::write(&git_config, format!("[core]\nexcludesFile = {excludes}\n")).unwrap();
    std::env::set_var("GIT_CONFIG_GLOBAL", &git_config);
    assert!(!root.starts_with(std::env::current_dir().unwrap()));

    let decision = |relative: &str| {
        ContextStreamClient::targeted_text_file_decision(
            root.to_str().unwrap(),
            root.join(relative).to_str().unwrap(),
        )
    };
    assert!(matches!(
        decision("keep.rs"),
        TargetedFileDecision::Upload(_)
    ));
    for path in ["ignored.rs", "ignored-directory/nested.rs"] {
        assert!(matches!(decision(path), TargetedFileDecision::Delete(found) if found == path));
    }
}
