"""Synthetic process observations and mocked collector only; no SQL/process calls."""
import ast
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import types
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / 'apps/desktop/tests/factory-child-goal-real-ipc.py'
FIELDS = ('pid', 'ppid', 'start_ticks', 'uid', 'pgid')
IDENTITY = {'pid': 2001, 'ppid': 2000, 'start_ticks': '98765', 'uid': 1000, 'pgid': 2001}
EXPECTED = {k: IDENTITY[k] for k in ('pid', 'pgid', 'start_ticks')}
ARGV = ['/synthetic/provider', '--fixture-settings', '/synthetic/settings']
RAW = b'python\0' + b'\0'.join(s.encode() for s in ARGV) + b'\0'
SECRET = 'PRIVATE_ARGV_CANARY'


def sha(raw): return hashlib.sha256(raw).hexdigest()


def require(value, *_):
    if not value: raise AssertionError('Strict graph observation refused')


def load():
    tree = ast.parse(DRIVER.read_text())
    names = {'GRAPH_IDENTITY_FIELDS', 'GRAPH_LIVE_STATES', 'GRAPH_TERMINAL_STATES'}
    nodes = [n for n in tree.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id in names for t in n.targets)
        or isinstance(n, ast.FunctionDef) and (n.name.startswith('graph_') or n.name in ('observe_graph_identity', 'bind_graph_observation', 'collect_graph_records'))]
    writes = []
    ns = {'publish_graph_binding_ack': mock.Mock(), 'Path': Path, 'os': types.SimpleNamespace(geteuid=lambda: 1000), 'copy': copy, 're': re, 'json': json,
        'require': require, 'base': types.SimpleNamespace(write_private_json=lambda path, value: writes.append((path.name, copy.deepcopy(value))), sha=sha, MAX_HTTP_BYTES=1048576)}
    exec(compile(ast.Module(body=nodes, type_ignores=[]), '<original-graph-observation-AST>', 'exec'), ns)
    return ns, writes


def sample(state='S', **changes):
    return {'observation': 'observed', **IDENTITY, 'state': state, **changes}


def decision(ns, known=None, before=None, raw=RAW, after=None):
    return ns['graph_binding_decision'](EXPECTED, known, before if before is not None else sample(),
        ns['graph_argv_observation'](raw, ARGV), after if after is not None else sample(), 1000)


class PureGraphObservation(unittest.TestCase):
    def test_only_stable_live_initial_metadata_and_exact_nonempty_argv_create_binding(self):
        ns, _ = load()
        result = decision(ns, before=sample('R'), after=sample('S'))
        self.assertEqual(result, {'kind': 'initial_live', 'binding': IDENTITY, 'stop_proved': False})
        self.assertIsNot(result['binding'], IDENTITY)
        for before, after in [({'observation': 'absent'}, {'observation': 'absent'}),
            (sample(), sample(start_ticks='98766')), (sample('Z'), sample('Z')),
            (sample(), {'observation': 'unobserved'})]:
            with self.subTest(before=before, after=after):
                self.assertEqual(decision(ns, before=before, after=after)['kind'], 'not_verified')

    def test_invalid_types_uid_ppid_group_or_changed_metadata_never_initially_bind(self):
        ns, _ = load()
        for key, value in [('pid', True), ('ppid', 0), ('uid', True), ('uid', 1001),
            ('pgid', False), ('pgid', 2002), ('start_ticks', 98765), ('start_ticks', ''), ('state', 'unknown')]:
            with self.subTest(key=key, value=value):
                self.assertEqual(decision(ns, before=sample(**{key: value}), after=sample(**{key: value}))['kind'], 'not_verified')
        self.assertEqual(decision(ns, after=sample(ppid=1999))['kind'], 'not_verified')

    def test_empty_unknown_unterminated_oversized_wrong_argv_is_never_live_proof(self):
        ns, _ = load()
        for raw in [b'', None, b'not-terminated', RAW + b'x' * 8193, b'\xff\0', SECRET.encode() + b'\0']:
            for known in [None, IDENTITY]:
                with self.subTest(raw_type=type(raw).__name__, known=known is not None):
                    self.assertEqual(decision(ns, known=known, raw=raw)['kind'], 'not_verified')
        for raw, kind in [(b'', 'empty'), (None, 'unobserved'), (b'x', 'missing_nul'),
            (b'x' * 8193, 'oversize'), (b'\xff\0', 'invalid_utf8'), (SECRET.encode() + b'\0', 'mismatch')]:
            observed = ns['graph_argv_observation'](raw, ARGV)
            self.assertEqual(observed['kind'], kind); self.assertNotIn(SECRET, json.dumps(observed))

    def test_repeat_terminal_absent_and_replaced_retain_only_previously_strong_binding(self):
        ns, _ = load()
        cases = [(sample(), b'', sample('Z'), 'repeat_terminal'),
            (sample(), RAW, sample(), 'repeat_live'),
            (sample(), b'', {'observation': 'absent'}, 'repeat_absent'),
            ({'observation': 'absent'}, None, {'observation': 'absent'}, 'repeat_absent'),
            (sample(), None, sample(start_ticks='98766'), 'repeat_replaced')]
        for before, raw, after, kind in cases:
            with self.subTest(kind=kind):
                result = decision(ns, known=IDENTITY, before=before, raw=raw, after=after)
                self.assertEqual(result, {'kind': kind, 'binding': None, 'stop_proved': False})
                initial_kind = 'initial_live' if kind == 'repeat_live' else 'not_verified'
                self.assertEqual(decision(ns, before=before, raw=raw, after=after)['kind'], initial_kind)
        result = ns['graph_binding_decision'](EXPECTED, IDENTITY, sample('Z'),
            {'kind': 'not_read', 'bytes': None}, sample('Z'), 1000)
        self.assertEqual(result['kind'], 'repeat_terminal'); self.assertIs(result['stop_proved'], False)

    def test_repeat_unknown_empty_live_or_changed_live_arguments_never_become_stop(self):
        ns, _ = load()
        for before, raw, after in [(sample(), b'', sample()),
            (sample(), None, sample('Z')), ({'observation': 'unobserved'}, None, {'observation': 'absent'}),
            (sample(), RAW, {'observation': 'unobserved'}),
            (sample(), RAW, sample(ppid=1999)), (sample(), RAW, sample(uid=1001)),
            (sample(), SECRET.encode() + b'\0', sample('Z'))]:
            with self.subTest(before=before, after=after):
                result = decision(ns, known=IDENTITY, before=before, raw=raw, after=after)
                self.assertEqual(result['kind'], 'not_verified'); self.assertIs(result['stop_proved'], False)
        for known in [{}, dict(IDENTITY, uid=1001), dict(IDENTITY, pid=True), dict(IDENTITY, extra=SECRET)]:
            self.assertEqual(decision(ns, known=known)['kind'], 'not_verified')

    def test_bounded_observer_orders_samples_and_never_reads_unbound_or_terminal_argv(self):
        ns, _ = load(); events = []; samples = [sample('R'), sample('S')]
        ns['graph_identity_sample'] = lambda pid: (events.append('metadata'), samples.pop(0))[1]
        class Stream:
            def __enter__(self): return self
            def __exit__(self, *_): return False
            def read(self, count): events.append(('read', count)); return RAW
        ns['Path'] = mock.Mock(return_value=types.SimpleNamespace(open=lambda _: Stream()))
        result, observed = ns['observe_graph_identity'](EXPECTED, ARGV, None)
        self.assertEqual(events, ['metadata', ('read', 8193), 'metadata'])
        self.assertEqual(result['kind'], 'initial_live'); self.assertNotIn(SECRET, json.dumps(observed))
        for samples, known, kind in [([{'observation': 'absent'}] * 2, None, 'not_verified'),
            ([sample('Z')] * 2, IDENTITY, 'repeat_terminal'),
            ([sample(start_ticks='98766')] * 2, IDENTITY, 'repeat_replaced')]:
            with self.subTest(kind=kind):
                ns['graph_identity_sample'] = mock.Mock(side_effect=samples)
                ns['Path'] = mock.Mock(side_effect=AssertionError('No unrelated process argv read'))
                result, observed = ns['observe_graph_identity'](EXPECTED, ARGV, known)
                self.assertEqual(result['kind'], kind); ns['Path'].assert_not_called()
                self.assertEqual(observed['argv'], {'kind': 'not_read', 'bytes': None})

    def test_sampler_absence_and_access_unknown_are_distinct_without_process_calls(self):
        ns, _ = load()
        for error, expected in [(FileNotFoundError(), 'absent'), (ProcessLookupError(), 'absent'),
            (PermissionError(SECRET), 'unobserved'), (ValueError(SECRET), 'unobserved')]:
            ns['support'] = types.SimpleNamespace(proc_identity=mock.Mock(side_effect=error))
            value = ns['graph_identity_sample'](EXPECTED['pid'])
            self.assertEqual(value, {'observation': expected}); self.assertNotIn(SECRET, json.dumps(value))


class MockedCollectorControls(unittest.TestCase):
    def fixture(self, root):
        ns, writes = load(); workspace = {'root': str(root / 'workspace'), 'id': 'workspace'}
        reports = Path(workspace['root']) / 'reports'; reports.mkdir(parents=True)
        request = {'task_binding': {'task_id': 'task'}, 'lineage': {'attempt_id': 'attempt', 'root_task_id': 'root'},
            'run_id': 'goal', 'node': {'id': 'node'}, 'plan_revision': 1}
        text = json.dumps(request); scope = {'id': 'scope', 'root_task_id': 'root', 'root_grant_id': 'grant'}
        context = {'nonce': 'nonce', 'program_sha256': 'p', 'settings_sha256': 's',
            'provider': ARGV[0], 'settings': ARGV[2], 'connection_id': 'connection'}
        row = {'fixture_nonce': 'nonce', 'mcp_servers': [], 'program_sha256': 'p', 'settings_sha256': 's',
            'provider_argv': ARGV, 'request_text': text, 'request_sha256': sha(text.encode()),
            'native_session_id': 'generated-child-goal-2001-nonce', 'owned_provider': EXPECTED}
        (reports / 'graph-request-2001.json').write_bytes(b'only synthetic input')
        raw = json.dumps(row).encode(); ns['base'].bounded_file = lambda *_: (raw, {'mode': 0o600})
        session = {'native_session_id': row['native_session_id'], 'id': 'session', 'task_id': 'task',
            'attempt_id': 'attempt', 'workspace_id': 'workspace', 'grant_id': 'grant', 'connection_id': 'connection'}
        class Rows(list):
            def fetchone(self): return self[0]
        class DB:
            def __enter__(self): return self
            def __exit__(self, *_): return False
            def execute(self, sql, params=None):
                self.commands.append(sql)
                if 'FROM sessions' in sql: return Rows([(json.dumps(session),)])
                if 'FROM session_turns' in sql: return Rows([(json.dumps({'text': text}),)])
                if 'FROM goal_run_nodes' in sql: return Rows([(json.dumps({'session_id': 'session', 'attempt_id': 'attempt'}),)])
                if 'FROM factory_child_goal_runs' in sql: return Rows([('scope', 'task')])
                require(sql in ('PRAGMA query_only=ON', 'BEGIN')); return Rows([])
            def rollback(self): self.rolled_back = True
        db = DB(); db.commands = []; db.rolled_back = False
        ns['sqlite3'] = types.SimpleNamespace(connect=mock.Mock(return_value=db))
        ns['support'] = types.SimpleNamespace(GRAPH_TASK_FIELDS=set(request))
        ns['planning'] = types.SimpleNamespace(unique_object=lambda pairs: dict(pairs))
        def install(before, raw, after):
            def observed(expected, expected_argv, known):
                result = ns['graph_binding_decision'](expected, known, before,
                    ns['graph_argv_observation'](raw, expected_argv), after, 1000)
                return result, {'kind': result['kind'], 'stop_proved': False}
            ns['observe_graph_identity'] = observed
        return ns, writes, workspace, scope, context, db, install

    def test_first_absent_or_replaced_or_unknown_never_appends_graph_record(self):
        for before, raw, after in [({'observation': 'absent'}, None, {'observation': 'absent'}),
            (sample(start_ticks='98766'), RAW, sample(start_ticks='98766')), (sample(), b'', sample()),
            (sample(), RAW, {'observation': 'unobserved'})]:
            with self.subTest(after=after), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve(); ns, writes, workspace, scope, context, db, install = self.fixture(root)
                install(before, raw, after); verified = []
                with self.assertRaises(AssertionError):
                    ns['collect_graph_records'](root / 'runtime', workspace, scope, context, verified, root)
                self.assertEqual(verified, []); self.assertEqual(context['_graph_provider_identities'], {})
                self.assertFalse(any(name.startswith('verified-graph') for name, _ in writes))

    def test_initial_then_repeat_terminal_absent_and_changed_live_keep_original_business_guards(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); ns, writes, workspace, scope, context, db, install = self.fixture(root)
            install(sample('R'), RAW, sample('S')); verified = []
            ns['collect_graph_records'](root / 'runtime', workspace, scope, context, verified, root)
            self.assertEqual(len(verified), 1); original = copy.deepcopy(verified)
            self.assertEqual(context['_graph_provider_identities'][verified[0]['path']], IDENTITY)
            for before, raw, after in [(sample(), b'', sample('Z')),
                ({'observation': 'absent'}, None, {'observation': 'absent'}),
                (sample(start_ticks='98766'), None, sample(start_ticks='98766'))]:
                install(before, raw, after)
                ns['collect_graph_records'](root / 'runtime', workspace, scope, context, verified, root)
                self.assertEqual(verified, original); self.assertTrue(db.rolled_back)
            install(sample(), SECRET.encode() + b'\0', sample())
            with self.assertRaises(AssertionError):
                ns['collect_graph_records'](root / 'runtime', workspace, scope, context, verified, root)
            self.assertEqual(verified, original)
            self.assertEqual(sum(name == 'verified-graph-2001.json' for name, _ in writes), 1)
            self.assertEqual(sum(name == 'verified-graph-identity-2001.json' for name, _ in writes), 1)
            self.assertFalse(any(value.get('stop_proved') is True for _, value in writes))
            self.assertFalse(any(sql.startswith(('INSERT', 'UPDATE', 'DELETE')) for sql in db.commands))

    def test_existing_legacy_record_without_initial_history_cannot_gain_strong_proof(self):
        ns, writes = load(); ns['observe_graph_identity'] = mock.Mock()
        with self.assertRaises(AssertionError):
            ns['bind_graph_observation']({'path': 'record', 'owned_provider': EXPECTED}, ARGV, {'path': 'record'}, {}, Path('/synthetic'))
        ns['observe_graph_identity'].assert_not_called(); self.assertEqual(writes, [])


class OriginalHarnessSourceBoundary(unittest.TestCase):
    def restore(self, text):
        from tests.infrastructure.test_graph_observation_handshake import strip_composed_outer_layer, strip_handshake_layer
        text = strip_composed_outer_layer(text, 'ours')
        self.assertEqual(sha(text.encode()), '4b0332152c2dff9e4d4d7df94dede8ce23b0735c1b76adbef14398c142811b88')
        text = strip_handshake_layer(text)
        start = text.index('GRAPH_IDENTITY_FIELDS = '); end = text.index('def collect_graph_records(', start)
        helpers = text[start:end]
        self.assertEqual(sha(helpers.encode()), '32714b9043182184e2487331956dd6ee78fbe106563d77b1db3af7d18fc11957')
        original = (ROOT / 'tests/infrastructure/fixtures/graph_identity_observation/pre_observation_block.py.txt').read_bytes()
        self.assertEqual(sha(original), '9af3fe5525eacfdcb1ecc805bc5b1676102812aaf87f430838c3a9361d33beb7')
        text = text[:start] + text[end:]
        call = "            bind_graph_observation(fixed,row['provider_argv'],old,context,output)\n"
        self.assertEqual(text.count(call), 1)
        return text.replace(call, original.decode(), 1)

    def test_exact_inverse_preserves_all_original_business_sql_schema_time_and_cleanup_source(self):
        restored = self.restore(DRIVER.read_text())
        self.assertEqual(sha(restored.encode()), '40607488a0f0e82a0146ba98b70267c1f828b9403d9e5b7336af2bca17b96419')
        for old, new in [("state']=='cancelled'", "state']in {'cancel_requested','cancelled'}"),
            ('len(v)==2', 'len(v)>=1'), ('time.monotonic()+15', 'time.monotonic()+30'),
            ("snapshot['schema'])is int and snapshot['schema']==17", "snapshot['schema'])is int and snapshot['schema']==15")]:
            self.assertIn(old, restored)
            try:
                changed = self.restore(DRIVER.read_text().replace(old, new, 1))
            except AssertionError:
                continue  # The exact newer handshake layer also refuses mutations.
            self.assertNotEqual(sha(changed.encode()), '40607488a0f0e82a0146ba98b70267c1f828b9403d9e5b7336af2bca17b96419')
        with self.assertRaises(AssertionError):
            self.restore(DRIVER.read_text().replace("kind == 'empty' and size != 0", "kind == 'empty' and size != 1", 1))


if __name__ == '__main__': unittest.main()
