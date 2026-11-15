"""Exercise the actual collaboration CLI against disposable local Git repositories."""
import concurrent.futures
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]


class CollabTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='ab-collab-test-')
        self.root = Path(self.temporary.name).resolve()
        self.repo = self.root / 'repo'
        self.repo.mkdir()
        self.script = self.repo / 'scripts/collab.sh'
        self.script.parent.mkdir()
        for name in ('collab.sh', 'collab-run.py'):
            shutil.copyfile(ROOT / 'scripts' / name, self.script.parent / name)
        self.git('init', '-b', 'main')
        self.git('config', 'user.name', 'Local fixture')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'commit.gpgsign', 'false')
        self.git('config', 'core.hooksPath', '/dev/null')
        for name in ('a/file', 'b/file', 'shared'):
            path = self.repo / name
            path.parent.mkdir(exist_ok=True)
            path.write_text('base\n')
        (self.repo / '.gitignore').write_text('artifacts/\n')
        self.git('add', '.')
        self.git('commit', '-m', 'fixture')
        self.git('init', '--bare', str(self.root / 'origin.git'))
        self.git('remote', 'add', 'origin', str(self.root / 'origin.git'))
        self.git('push', 'origin', 'main')
        self.state = self.repo / '.git/agentbaton-collab'

    def tearDown(self):
        if self.state.exists():
            for record in self.state.glob('runtime-*'):
                if not record.is_symlink():
                    path = Path(record.read_text().strip())
                    # Only delete directories made by this test's real CLI.
                    if path.parent == Path('/tmp').resolve() and path.name.startswith('ab-collab-'):
                        shutil.rmtree(path, ignore_errors=True)
        self.temporary.cleanup()

    def git(self, *args, cwd=None):
        result = subprocess.run(['git', *args], cwd=cwd or self.repo,
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def cli(self, *args, cwd=None, expected=0, env=None):
        result = subprocess.run(['/bin/bash', str(self.script), *args], cwd=cwd or self.repo,
                                text=True, capture_output=True, env=env, timeout=15)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        return result

    def task(self, name, *paths):
        self.cli('new', name, 'origin/main', *paths)
        return self.repo / 'artifacts/collab-worktrees' / name

    def probe(self, task, env=None):
        result = self.cli('run', '--', sys.executable, '-c',
                          'import json,os; print(json.dumps([os.getcwd(),os.environ["BATON_RUNTIME_DIR"],os.environ["CARGO_TARGET_DIR"]]))',
                          cwd=task, env=env)
        return json.loads(result.stdout.splitlines()[-1])

    def test_overlap_and_disjoint_worktrees(self):
        a = self.task('a', 'a')
        for path in ('a', 'a/file', 'A/file'):
            self.cli('new', 'overlap', 'origin/main', path, expected=2)
        b = self.task('b', 'b')
        (a / 'a/file').write_text('task A\n')
        self.assertEqual((b / 'a/file').read_text(), 'base\n')
        self.cli('scope', cwd=a)

    def test_parent_overlap_and_invalid_paths(self):
        self.task('a', 'a/file')
        self.cli('new', 'parent', 'origin/main', 'a', expected=2)
        for path in ('../other', 'a/../b', '.git/config', 'artifacts/test'):
            self.cli('new', 'invalid', 'origin/main', path, expected=2)

    def test_concurrent_claim_has_one_winner(self):
        def create(name):
            return subprocess.run(['/bin/bash', str(self.script), 'new', name,
                                   'origin/main', 'a'], cwd=self.repo, capture_output=True).returncode
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            self.assertEqual(sorted(pool.map(create, ('one', 'two'))), [0, 2])
        self.assertEqual(len(list((self.state / 'active').iterdir())), 1)

    def test_scope_detects_all_edit_states(self):
        task = self.task('a', 'a')
        (task / 'b/file').write_text('outside\n')
        self.cli('scope', cwd=task, expected=1)
        (task / 'b/file').write_text('base\n')
        (task / 'outside').write_text('new\n')
        self.cli('scope', cwd=task, expected=1)
        self.git('add', 'outside', cwd=task)
        self.cli('scope', cwd=task, expected=1)
        self.git('commit', '-m', 'out of scope fixture', cwd=task)
        self.cli('scope', cwd=task, expected=1)

    def test_rename_checks_source_and_destination(self):
        task = self.task('a', 'a')
        self.git('mv', 'b/file', 'a/moved', cwd=task)
        self.cli('scope', cwd=task, expected=1)

    def test_preview_conflict_is_nonmutating(self):
        task = self.task('a', 'shared')
        (task / 'shared').write_text('source\n')
        self.git('add', 'shared', cwd=task)
        self.git('commit', '-m', 'source', cwd=task)
        self.git('checkout', '-b', 'target')
        (self.repo / 'shared').write_text('target\n')
        self.git('add', 'shared')
        self.git('commit', '-m', 'target')
        before = (self.git('rev-parse', 'HEAD', cwd=task), self.git('status', '--porcelain', cwd=task))
        self.cli('preview', 'target', cwd=task, expected=1)
        self.assertEqual(before, (self.git('rev-parse', 'HEAD', cwd=task), self.git('status', '--porcelain', cwd=task)))
        self.cli('preview', 'origin/main', cwd=task)

    def test_rebase_refuses_dirty_and_published(self):
        task = self.task('a', 'a')
        (task / 'a/file').write_text('dirty\n')
        self.cli('rebase', '--private', cwd=task, expected=2)
        (task / 'a/file').write_text('base\n')
        self.git('push', 'origin', 'collab/a', cwd=task)
        self.cli('rebase', '--private', cwd=task, expected=2)

    def test_runtime_and_build_output_are_task_local_and_stable(self):
        a = self.task('a', 'a')
        b = self.task('b', 'b')
        environment = dict(os.environ, BATON_RUNTIME_DIR='/must/not/use', CARGO_TARGET_DIR='/must/not/use')
        first, second = self.probe(a, environment), self.probe(b, environment)
        self.assertEqual(first, self.probe(a))
        self.assertNotEqual(first[1], second[1])
        self.assertEqual(first[0], str(a))
        self.assertEqual(first[2], str(a / 'target'))
        self.assertEqual(second[2], str(b / 'target'))
        for path in (first[1], second[1]):
            self.assertEqual(Path(path).stat().st_mode & 0o777, 0o700)
            self.assertLess(len(os.fsencode(Path(path) / 'batond.sock')), 104)

    def test_unsafe_runtime_is_rejected_without_chmod(self):
        task = self.task('a', 'a')
        path = Path(self.probe(task)[1])
        path.chmod(0o755)
        self.cli('run', '--', sys.executable, '-c', 'raise AssertionError("must not run")', cwd=task, expected=2)
        self.assertEqual(path.stat().st_mode & 0o777, 0o755)

    def test_run_preserves_argv_and_exit_code(self):
        task = self.task('a', 'a')
        value = 'spaces; $(false) "quoted"'
        result = self.cli('run', '--', sys.executable, '-c',
                          'import sys; print(sys.argv[1]); sys.exit(7)', value, cwd=task, expected=7)
        self.assertEqual(result.stdout.splitlines()[-1], value)

    def test_native_and_task_leases_and_signal_cleanup(self):
        a = self.task('a', 'a')
        b = self.task('b', 'b')
        ready = self.root / 'ready'
        code = ('import signal,sys,time; from pathlib import Path; '
                'signal.signal(signal.SIGTERM,lambda *_:sys.exit(23)); '
                'Path(sys.argv[1]).write_text("ready"); time.sleep(30)')
        process = subprocess.Popen(['/bin/bash', str(self.script), 'run', '--native', '--',
                                    sys.executable, '-c', code, str(ready)],
                                   cwd=a, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        try:
            deadline = time.monotonic() + 10
            while not ready.exists() and process.poll() is None and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(ready.exists())
            self.cli('run', '--', 'true', cwd=a, expected=2)
            self.cli('finish', '--handoff', cwd=a, expected=2)
            self.cli('run', '--native', '--', 'true', cwd=b, expected=2)
            self.probe(b)  # Non-native tasks continue while a native command runs.
            process.terminate()
            stdout, stderr = process.communicate(timeout=10)
            self.assertEqual(process.returncode, 23, stdout + stderr)
            self.cli('run', '--native', '--', 'true', cwd=b)
            self.probe(a)
        finally:
            if process.poll() is None:
                process.terminate()
                process.communicate(timeout=10)

    def test_finish_releases_claim_and_prevents_further_run(self):
        task = self.task('a', 'a')
        self.cli('finish', '--handoff', cwd=task)
        self.cli('run', '--', 'true', cwd=task, expected=2)
        self.task('replacement', 'a')


if __name__ == '__main__':
    unittest.main()
