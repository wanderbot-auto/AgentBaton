"""Exact source-capture composition proof; no Git, Rust runtime or IO workers.

This does not execute the new Rust contracts. Main owns actual Cargo validation.
"""
import hashlib
import json
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[2]
SOURCE=ROOT/'crates/workspaces/src/source_capture.rs'
RUST_TEST=ROOT/'crates/workspaces/src/source_capture_git_path_observation_tests.rs'
HEAD_BATCH_INVERSE=Path(__file__).parent/'fixtures/source_capture_head_batch_inverse.json'
HEAD_BATCH_INVERSE_SHA256='0133b3565f56712448fe26d3bab1b89914ba349130ea36a2a0f9b70dea75401e'
ORIGINAL_SHA256='f589fc936d466e75a524482686cd7caade2350f0453c8171d25fb35e2635aeb8'
CANDIDATE_SHA256='7b76540fa3186e2df503f9eabc17cd951fdccf9fc5cc64c9f841cc9d563924ed'
FRAGMENTS={'helper': '// One fresh read per original recheck boundary; never retained as authority.\n// Paths cannot contain newlines/control bytes in the original contract. Require\n// exactly three complete fields and the original canonical pin for each one.\nfn verify_git_path_observation(\n    raw: &[u8],\n    expected: [&Path; 3],\n    deadline: Instant,\n    control: Option<&crate::adoption::AdoptionControl<\'_>>,\n) -> Result<(), WorkspaceError> {\n    check_capture_control(deadline, control)?;\n    let text = std::str::from_utf8(raw).map_err(|_| WorkspaceError::InvalidPath)?;\n    if text.len() > 3 * (MAX_PATH_BYTES + 1) {\n        return Err(WorkspaceError::InvalidPath);\n    }\n    let text = text.strip_suffix(\'\\n\').ok_or(WorkspaceError::InvalidPath)?;\n    let fields = text.splitn(4, \'\\n\').collect::<Vec<_>>();\n    if fields.len() != 3 {\n        return Err(WorkspaceError::InvalidPath);\n    }\n    for (text, expected) in fields.into_iter().zip(expected) {\n        check_capture_control(deadline, control)?;\n        if text.is_empty() || text.len() > MAX_PATH_BYTES || text.chars().any(char::is_control) {\n            return Err(WorkspaceError::InvalidPath);\n        }\n        let path = Path::new(text);\n        if fs::canonicalize(path)? != path || path != expected {\n            return Err(WorkspaceError::InvalidPath);\n        }\n        check_capture_control(deadline, control)?;\n    }\n    Ok(())\n}\nfn recheck_git_paths(\n    deadline: Instant,\n    root: &Path,\n    expected: [&Path; 3],\n    control: Option<&crate::adoption::AdoptionControl<\'_>>,\n) -> Result<(), WorkspaceError> {\n    let raw = text_output_controlled(\n        deadline,\n        git_command(root).args([\n            "rev-parse",\n            "--path-format=absolute",\n            "--show-toplevel",\n            "--absolute-git-dir",\n            "--git-common-dir",\n        ]),\n        control,\n    )?;\n    verify_git_path_observation(&raw, expected, deadline, control)\n}\n\n', 'old': '        if git_path(deadline, &self.root, "--show-toplevel", control)? != self.root\n            || git_path(deadline, &self.root, "--absolute-git-dir", control)? != self.git_dir\n            || git_path(deadline, &self.root, "--git-common-dir", control)? != self.common_dir\n        {\n            return Err(WorkspaceError::InvalidPath);\n        }\n', 'new': '        recheck_git_paths(\n            deadline,\n            &self.root,\n            [&self.root, &self.git_dir, &self.common_dir],\n            control,\n        )?;\n', 'module': '\n#[cfg(test)]\n#[path = "source_capture_git_path_observation_tests.rs"]\nmod git_path_observation_tests;\n'}

def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def before_head_batch(path, raw):
    """Authenticate the current source and restore each exact historical layer."""
    inverse = HEAD_BATCH_INVERSE.read_bytes()
    if digest(inverse) != HEAD_BATCH_INVERSE_SHA256:
        raise AssertionError('Unknown HEAD batch inverse fixture')
    layers = json.loads(inverse)
    layer = layers[path.relative_to(ROOT).as_posix()]
    followup = layer.get('followup')
    current = followup or layer
    if type(raw) is not bytes or digest(raw) != current['after']:
        raise AssertionError('Unknown HEAD batch composition')
    if followup is not None:
        raw = restore_exact_spans(raw, followup['edits'], layer['after'])
    return restore_exact_spans(raw, layer['edits'], layer['before'])


def restore_exact_spans(raw, edits, expected):
    lines = raw.decode().splitlines(keepends=True)
    for edit in reversed(edits):
        start, end = edit['start'], edit['end']
        if not 0 <= start < end <= len(lines) or ''.join(lines[start:end]) != edit['expected']:
            raise AssertionError('Missing or displaced HEAD batch span')
        lines[start:end] = edit['replacement'].splitlines(keepends=True)
    restored = ''.join(lines).encode()
    if digest(restored) != expected:
        raise AssertionError('Complete pre-batch source not restored')
    return restored


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
        combined = before_head_batch(SOURCE, SOURCE.read_bytes())
        self.assertEqual(digest(combined), CANDIDATE_SHA256)
        self.assertEqual(digest(original_source(combined)), ORIGINAL_SHA256)
        historical_tests = before_head_batch(RUST_TEST, RUST_TEST.read_bytes())
        self.assertEqual(digest(historical_tests),
                         '18ee67e144e0d7420c7fd59bde173daa47ec21dd7e799d3a5191d0b51d2b61f5')
        self.assertEqual(RUST_TEST.read_bytes().count(b'#[test]'), historical_tests.count(b'#[test]') + 1)

    def test_all_original_recheck_calls_and_owning_clocks_are_untouched(self):
        combined = before_head_batch(SOURCE, SOURCE.read_bytes())
        before = original_source(combined).decode()
        reviewed = combined.decode().replace(FRAGMENTS['helper'], '')
        current = SOURCE.read_text()
        for token in ['capture.recheck(control)?;',
                      'capture.verify_tree(&tree, &expected_tree, control)?;',
                      'capture.verify_blob(blob, &selected.sha256, selected.bytes, control)?;',
                      'capture.verify_blob(&blob, &digest, bytes.len() as u64, control)?;']:
            self.assertEqual(before.count(token), reviewed.count(token))
            self.assertEqual(before.count(token), current.count(token))
        self.assertEqual(before.count('check_capture_control(deadline, control)?;'),
                         reviewed.count('check_capture_control(deadline, control)?;'))

    def test_head_and_all_path_pins_share_one_fresh_controlled_observation(self):
        source = SOURCE.read_text()
        observer = source[source.index('fn recheck_git_paths('):source.index('\nfn no_alternates(')]
        self.assertEqual(observer.count('text_output_controlled('), 1)
        self.assertEqual(observer.count('git_command(root).args('), 1)
        expected = ['"rev-parse"', '"--path-format=absolute"', '"--show-toplevel"',
                    '"--absolute-git-dir"', '"--git-common-dir"', '"--verify"', '"HEAD^{commit}"',
                    'let [top, git_dir, common, head] = decode_git_paths(&raw, deadline, control)?',
                    'verify_git_paths([top, git_dir, common], expected, deadline, control)?',
                    '!is_oid(head) || head != base', 'Err(WorkspaceError::StaleHead)']
        positions = [observer.index(fragment) for fragment in expected]
        self.assertEqual(positions, sorted(positions))
        start = source.index('    fn recheck(')
        recheck = source[start:source.index('\nfn check_tree_paths', start)]
        expected = ['check_capture_control(deadline, control)?;',
                    'self.current_selection_digest(control)? != self.selection_digest',
                    'open_root_without_symlinks(p)?.metadata()?',
                    'audit(deadline, &mut self.command(), control)?;',
                    'no_alternates(deadline, &self.common_dir, control)?;',
                    'recheck_git_paths(', '[&self.root, &self.git_dir, &self.common_dir]',
                    '&self.base,\n            control,']
        positions = [recheck.index(fragment) for fragment in expected]
        self.assertEqual(positions, sorted(positions))
        self.assertNotIn('"HEAD^{commit}"', recheck)

    def test_initial_git_and_common_directory_observation_keeps_separate_fd_checks(self):
        source = SOURCE.read_text()
        start = source.index('        let paths = text_output_controlled(')
        observe = source[start:source.index('        let mut result = Self {', start)]
        self.assertEqual(observe.count('text_output_controlled('), 1)
        expected = ['"--absolute-git-dir"', '"--git-common-dir"', 'decode_git_paths(&paths, deadline, control)?;',
                    '[&git_dir, common_dir]', 'open_root_without_symlinks(&git_dir)?',
                    'open_root_without_symlinks(common_dir)?', 'no_alternates(deadline, common_dir, control)?;']
        positions = [observe.index(fragment) for fragment in expected]
        self.assertEqual(positions, sorted(positions))

    def test_unknown_outside_change_is_rejected(self):
        raw=before_head_batch(SOURCE,SOURCE.read_bytes())
        for bad in [raw+b'\n// unknown\n',raw.replace(b'const MAX_PATH_BYTES: usize = 4096;',b'const MAX_PATH_BYTES: usize = 8192;')]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)

    def test_duplicate_or_relocated_helper_is_rejected(self):
        raw=before_head_batch(SOURCE,SOURCE.read_bytes());block=FRAGMENTS['helper'].encode()
        for bad in [block+raw,raw.replace(block,b'')+block]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)

    def test_removing_bound_head_or_old_test_cannot_be_hidden_by_inverse(self):
        raw=before_head_batch(SOURCE,SOURCE.read_bytes())
        for bad in [raw.replace(b'"HEAD^{commit}"',b'"HEAD"'),raw.replace(b'#[test]',b'#[ignore]',1)]:
            with self.assertRaisesRegex(AssertionError,'Unknown source composition'):original_source(bad)

    def test_new_head_batch_inverse_rejects_changed_or_moved_guards_and_unknown_bytes(self):
        raw = SOURCE.read_bytes()
        anchor = b'            "--verify",\n            "HEAD^{commit}",\n'
        self.assertEqual(raw.count(anchor), 1)
        mutations = [raw + b'// outside\n', raw + anchor, raw.replace(anchor, b'', 1),
                     raw.replace(anchor, b'', 1) + anchor,
                     raw.replace(b'head != base', b'head == base'),
                     raw.replace(b'!is_oid(head)', b'is_oid(head)'),
                     raw.replace(b'path != expected', b'path == expected')]
        for changed in mutations:
            self.assertNotEqual(changed, raw)
            with self.assertRaisesRegex(AssertionError, 'Unknown HEAD batch composition'):
                before_head_batch(SOURCE, changed)
        tests = RUST_TEST.read_bytes()
        for changed in (tests + b'// outside\n', tests.replace(b'#[test]', b'#[ignore]', 1)):
            with self.assertRaisesRegex(AssertionError, 'Unknown HEAD batch composition'):
                before_head_batch(RUST_TEST, changed)


if __name__=='__main__':unittest.main()
