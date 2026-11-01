"""Exact source-capture composition proof; no Git, Rust runtime or IO workers.

This does not execute the new Rust contracts. Main owns actual Cargo validation.
"""
import hashlib
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'crates/workspaces/src/source_capture.rs'
ORIGINAL_SHA256='f589fc936d466e75a524482686cd7caade2350f0453c8171d25fb35e2635aeb8'
CANDIDATE_SHA256='7b76540fa3186e2df503f9eabc17cd951fdccf9fc5cc64c9f841cc9d563924ed'
FRAGMENTS={'helper': '// One fresh read per original recheck boundary; never retained as authority.\n// Paths cannot contain newlines/control bytes in the original contract. Require\n// exactly three complete fields and the original canonical pin for each one.\nfn verify_git_path_observation(\n    raw: &[u8],\n    expected: [&Path; 3],\n    deadline: Instant,\n    control: Option<&crate::adoption::AdoptionControl<\'_>>,\n) -> Result<(), WorkspaceError> {\n    check_capture_control(deadline, control)?;\n    let text = std::str::from_utf8(raw).map_err(|_| WorkspaceError::InvalidPath)?;\n    if text.len() > 3 * (MAX_PATH_BYTES + 1) {\n        return Err(WorkspaceError::InvalidPath);\n    }\n    let text = text.strip_suffix(\'\\n\').ok_or(WorkspaceError::InvalidPath)?;\n    let fields = text.splitn(4, \'\\n\').collect::<Vec<_>>();\n    if fields.len() != 3 {\n        return Err(WorkspaceError::InvalidPath);\n    }\n    for (text, expected) in fields.into_iter().zip(expected) {\n        check_capture_control(deadline, control)?;\n        if text.is_empty() || text.len() > MAX_PATH_BYTES || text.chars().any(char::is_control) {\n            return Err(WorkspaceError::InvalidPath);\n        }\n        let path = Path::new(text);\n        if fs::canonicalize(path)? != path || path != expected {\n            return Err(WorkspaceError::InvalidPath);\n        }\n        check_capture_control(deadline, control)?;\n    }\n    Ok(())\n}\nfn recheck_git_paths(\n    deadline: Instant,\n    root: &Path,\n    expected: [&Path; 3],\n    control: Option<&crate::adoption::AdoptionControl<\'_>>,\n) -> Result<(), WorkspaceError> {\n    let raw = text_output_controlled(\n        deadline,\n        git_command(root).args([\n            "rev-parse",\n            "--path-format=absolute",\n            "--show-toplevel",\n            "--absolute-git-dir",\n            "--git-common-dir",\n        ]),\n        control,\n    )?;\n    verify_git_path_observation(&raw, expected, deadline, control)\n}\n\n', 'old': '        if git_path(deadline, &self.root, "--show-toplevel", control)? != self.root\n            || git_path(deadline, &self.root, "--absolute-git-dir", control)? != self.git_dir\n            || git_path(deadline, &self.root, "--git-common-dir", control)? != self.common_dir\n        {\n            return Err(WorkspaceError::InvalidPath);\n        }\n', 'new': '        recheck_git_paths(\n            deadline,\n            &self.root,\n            [&self.root, &self.git_dir, &self.common_dir],\n            control,\n        )?;\n', 'module': '\n#[cfg(test)]\n#[path = "source_capture_git_path_observation_tests.rs"]\nmod git_path_observation_tests;\n'}

def original_source(raw):
    assert type(raw) is bytes and hashlib.sha256(raw).hexdigest()==CANDIDATE_SHA256, 'Unknown source composition'
    source=raw.decode()
    for new,old in [(FRAGMENTS['helper'],''),(FRAGMENTS['new'],FRAGMENTS['old']),(FRAGMENTS['module'],'')]:
        assert source.count(new)==1, 'Missing or duplicate source boundary'
        source=source.replace(new,old,1)
    restored=source.encode()
    assert hashlib.sha256(restored).hexdigest()==ORIGINAL_SHA256, 'Original whole source not restored'
    return restored


class ExactSourceCaptureComposition(unittest.TestCase):
    def test_full_original_source_including_all_original_tests_restores_byte_for_byte(self):
        self.assertEqual(hashlib.sha256(original_source(SOURCE.read_bytes())).hexdigest(),ORIGINAL_SHA256)

    def test_all_original_recheck_calls_and_owning_clocks_are_untouched(self):
        before=original_source(SOURCE.read_bytes()).decode();after=SOURCE.read_text()
        self.assertEqual(before.count('capture.recheck(control)?;'),after.count('capture.recheck(control)?;'))
        for token in ['capture.verify_tree(&tree, &expected_tree, control)?;',
                      'capture.verify_blob(blob, &selected.sha256, selected.bytes, control)?;',
                      'capture.verify_blob(&blob, &digest, bytes.len() as u64, control)?;',
                      'check_capture_control(deadline, control)?;']:
            if token.startswith('check_capture'):
                after=after.replace(FRAGMENTS['helper'],'')
            self.assertEqual(before.count(token),after.count(token))

    def test_original_bound_head_still_follows_local_path_observation(self):
        source=SOURCE.read_text();start=source.index('    fn recheck(');end=source.index('\nfn check_tree_paths',start)
        recheck=source[start:end]
        self.assertLess(recheck.index('recheck_git_paths('),recheck.index('.args(["rev-parse", "--verify", "HEAD^{commit}"])'))
        self.assertIn('no_alternates(deadline, &self.common_dir, control)?;',recheck)
        self.assertIn('audit(deadline, &mut self.command(), control)?;',recheck)
        self.assertIn('Err(WorkspaceError::StaleHead)',recheck)

    def test_unknown_outside_change_is_rejected(self):
        raw=SOURCE.read_bytes()
        for bad in [raw+b'\n// unknown\n',raw.replace(b'const MAX_PATH_BYTES: usize = 4096;',b'const MAX_PATH_BYTES: usize = 8192;')]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)

    def test_duplicate_or_relocated_helper_is_rejected(self):
        raw=SOURCE.read_bytes();block=FRAGMENTS['helper'].encode()
        for bad in [block+raw,raw.replace(block,b'')+block]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)

    def test_removing_bound_head_or_old_test_cannot_be_hidden_by_inverse(self):
        raw=SOURCE.read_bytes()
        for bad in [raw.replace(b'"HEAD^{commit}"',b'"HEAD"'),raw.replace(b'#[test]',b'#[ignore]',1)]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)


if __name__=='__main__':unittest.main()
