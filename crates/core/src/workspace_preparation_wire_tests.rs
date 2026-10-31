use super::*;
use serde_json::json;

#[test]
fn renderer_projection_keeps_zero_large_and_max_filesystem_integers_exact() {
    for (device, inode) in [(0, 0), (9_007_199_254_740_993, u64::MAX)] {
        let value = CodeWorkspacePreparation {
            task_id: "task".into(),
            task_revision: 1,
            spec_revision: 1,
            project_id: "project".into(),
            project_revision: Some(1),
            host_id: "local".into(),
            source: Some(WorkspaceIdentity {
                canonical_root: PathBuf::from("/source"),
                device,
                inode,
                kind: WorkspaceKind::Directory,
            }),
            source_head: None,
            preparation_key: None,
            state: CodeWorkspacePreparationState::NeedsDatasetSelection,
            reason: CodeWorkspacePreparationReason::SelectedDatasetRequired,
            candidates: vec![],
            execution_authorized: false,
            creates_workspace: false,
        };
        // This is a serializer unit control, not a real filesystem identity receipt.
        let original = serde_json::to_value(&value).unwrap();
        assert_eq!(original["source"]["device"].as_u64(), Some(device));
        let wire = serde_json::to_value(CodeWorkspacePreparationWire::from(value)).unwrap();
        assert_eq!(wire["source"]["device"], device.to_string());
        assert_eq!(wire["source"]["inode"], inode.to_string());
        let mut expected = original;
        expected["source"]["device"] = json!(device.to_string());
        expected["source"]["inode"] = json!(inode.to_string());
        assert_eq!(wire, expected);
        assert_eq!(wire["execution_authorized"], false);
        assert_eq!(wire["creates_workspace"], false);
    }
}
