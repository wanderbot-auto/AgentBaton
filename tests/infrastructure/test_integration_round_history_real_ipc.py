"""Pure controls for the optional new driver; no browser, Core or product Child."""
import copy
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_round_history_pure', ROOT / 'apps/desktop/tests/integration-round-history-real-ipc.py')
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)


def context():
    limits = {'attempts': 12, 'runtime_ms': 20000, 'output_bytes': 8192}
    usage = {'attempts': 1, 'active': 0, 'runtime_ms': 1300, 'output_bytes': 12}
    return {'task_revision': 2, 'bindings': {'task_id': 'task', 'spec_revision': 1, 'goal_scope_id': 'scope',
        'goal_scope_revision': 1, 'root_budget': copy.deepcopy(limits), 'project_budget': copy.deepcopy(limits)},
        'shared_usage': {'root': copy.deepcopy(usage), 'project': copy.deepcopy(usage)}}


class RoundSelectionTests(unittest.TestCase):
    def test_new_selection_only_has_its_explicit_three_round_headroom(self):
        ctx = context(); before = copy.deepcopy(ctx); ref = {'kind': 'command', 'policy_id': 'command', 'revision': 1}
        original = R.D.expected_selection(ctx, 'a' * 40, ref)
        selected = R.selected(ctx, 'a' * 40, ref)
        self.assertEqual(ctx, before)
        self.assertEqual((original['max_rounds'], original['on_target_advance'], original['ttl_ms']), (1, 'block', 60000))
        self.assertEqual((selected['max_rounds'], selected['on_target_advance'], selected['ttl_ms']), (3, 'bounded_descendant_reprepare', 30000))
        for key, output in [('preparation', 1024), ('cas', 1024)]:
            self.assertEqual(selected['stage_reservations'][key], {'runtime_ms': 2000, 'output_bytes': output, 'tokens': None, 'cost_microunits': None})
            self.assertEqual(original['stage_reservations'][key]['runtime_ms'], 5000)
        self.assertEqual(selected['stage_reservations']['validators'][0]['reservation']['runtime_ms'], 2000)
        self.assertEqual(selected['stage_reservations']['validators'][0]['reservation']['output_bytes'], 256)
        self.assertEqual(set(selected), set(original))

    def test_conservative_headroom_checks_both_domains_without_repair(self):
        R.headroom(context())
        mutations = [('runtime_ms', 2001), ('runtime_ms', None), ('runtime_ms', True), ('output_bytes', 1281),
            ('attempts', 2), ('active', 1)]
        for domain in ('root', 'project'):
            for field, value in mutations:
                ctx = context(); ctx['shared_usage'][domain][field] = value; before = copy.deepcopy(ctx)
                with self.subTest(domain=domain, field=field, value=value), self.assertRaises(R.D.Refusal):
                    R.headroom(ctx)
                self.assertEqual(ctx, before)

    def test_old_limits_are_not_silently_widened_by_headroom(self):
        for field, value in [('attempts', 8), ('runtime_ms', 40000), ('output_bytes', 16384)]:
            ctx = context(); ctx['bindings']['root_budget'][field] = value
            with self.subTest(field=field), self.assertRaises(R.D.Refusal): R.headroom(ctx)


class HostSelectionTests(unittest.TestCase):
    def setUp(self):
        self.host = object.__new__(R.RoundHost)
        self.host.gate = type('Gate', (), {'ready_path': Path('/owned/ready'), 'release_path': Path('/owned/release')})()

    def selected(self, method, params):
        before = copy.deepcopy(params)
        with mock.patch.object(R.D.Host, 'approved', side_effect=lambda method, params, *rest: params) as call:
            result = self.host.approved(method, params, 'fixed', 'fixed')
        self.assertEqual(params, before); self.assertEqual(call.call_count, 1)
        return result

    def test_original_grant_and_shared_budgets_only_new_attempt_count(self):
        grant = self.selected('grants.propose', {'max_attempts': 8, 'max_duration_ms': 30000, 'max_concurrency': 1})
        self.assertEqual(grant, {'max_attempts': 12, 'max_duration_ms': 30000, 'max_concurrency': 1})
        value = {'root_budget': {'attempts': 8, 'runtime_ms': 20000, 'output_bytes': 8192},
            'project_budget': {'attempts': 8, 'runtime_ms': 20000, 'output_bytes': 8192}}
        result = self.selected('goal_scopes.propose', value)
        self.assertEqual(result['root_budget'], {'attempts': 12, 'runtime_ms': 20000, 'output_bytes': 8192})
        self.assertEqual(result['project_budget'], result['root_budget'])

    def test_actual_bootstrap_alias_is_validated_before_either_domain_changes(self):
        # D.bootstrap intentionally supplies the same budget object twice.
        budget = {'attempts': 8, 'runtime_ms': 20000, 'output_bytes': 8192,
            'parallel': 1, 'children': 1, 'reworks': 0, 'tokens': None, 'cost_microunits': None}
        params = {'root_budget': budget, 'project_budget': budget, 'owner_acceptance_criteria': 'fixed'}
        result = self.selected('goal_scopes.propose', params)
        self.assertIs(params['root_budget'], params['project_budget'])
        self.assertEqual(budget['attempts'], 8)
        expected = {**budget, 'attempts': 12}
        self.assertEqual(result['root_budget'], expected)
        self.assertEqual(result['project_budget'], expected)
        self.assertEqual(result['owner_acceptance_criteria'], 'fixed')

    def test_command_only_changes_original_explicit_arguments_not_caps_or_image(self):
        params = {'wall_time_ms': 500, 'cleanup_time_ms': 500, 'expected_exit_codes': [0], 'mode': 'native_executable',
            'script': None, 'executable': '/owned/fixed-sh', 'arguments': ['-c', 'test -s reports/report.txt'], 'stdout_limit': 128,
            'stderr_limit': 128, 'require': {'filesystem_isolation': False}}
        result = self.selected('command_validation_policies.propose', params)
        self.assertEqual(result['arguments'], ['-c', R.SCRIPT, 'bounded-reprepare', '/owned/ready', '/owned/release'])
        self.assertEqual({k: v for k, v in result.items() if k != 'arguments'}, {k: v for k, v in params.items() if k != 'arguments'})

    def test_changed_original_reservations_or_success_codes_refuse(self):
        for field, value in [('wall_time_ms', 1000), ('cleanup_time_ms', 1000), ('expected_exit_codes', [0, 7]), ('script', 'injected')]:
            params = {'wall_time_ms': 500, 'cleanup_time_ms': 500, 'expected_exit_codes': [0], 'mode': 'native_executable', 'script': None}
            params[field] = value
            with self.subTest(field=field), self.assertRaises(R.D.Refusal):
                self.selected('command_validation_policies.propose', params)


class GateControls(unittest.TestCase):
    def test_fifo_pins_reject_path_replacement_without_starting_processes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); gate = R.RoundGate(root)
            try:
                gate.check(); gate.ready_path.unlink(); gate.ready_path.write_bytes(b'not FIFO')
                with self.assertRaises(R.D.Refusal): gate.check()
            finally: self.assertTrue(gate.close()['thread_joined'])

    def test_git_timeout_kills_and_waits_original_child_and_never_returns_success(self):
        class Child:
            returncode = None
            def __init__(self): self.killed = self.waited = False
            def poll(self): return None
            def kill(self): self.killed = True
            def wait(self): self.waited = True; return -9
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); gate = R.RoundGate(root); child = Child()
            try:
                with mock.patch.object(R.subprocess, 'Popen', return_value=child) as spawned, mock.patch.object(R.time, 'monotonic', side_effect=[0, 2]):
                    with self.assertRaises(R.D.Refusal): gate.git(root, ['rev-parse', 'HEAD'], 1)
                self.assertTrue(child.killed and child.waited)
                self.assertEqual(spawned.call_count, 1)
                self.assertEqual(spawned.call_args.kwargs['stdin'], subprocess.DEVNULL)
            finally: gate.close()

    def test_proc_missing_is_gone_but_zombie_or_same_birth_not_reaped(self):
        identity = {'pid': 123, 'birth': '100'}
        with mock.patch.object(R, 'proc_identity', side_effect=FileNotFoundError): self.assertTrue(R.original_gone(identity))
        with mock.patch.object(R, 'proc_identity', return_value={'pid': 123, 'birth': '100'}): self.assertFalse(R.original_gone(identity))
        with mock.patch.object(R, 'proc_identity', return_value={'pid': 123, 'birth': '101'}): self.assertTrue(R.original_gone(identity))
        with mock.patch.object(R, 'proc_identity', side_effect=R.D.Refusal('zombie')):
            with self.assertRaises(R.D.Refusal): R.original_gone(identity)


class DriverLifecycleOrderTests(unittest.TestCase):
    @staticmethod
    def assert_order(source):
        observe = "        done = D.observe(read_terminal, finished, pump=pump)\n"
        refresh = "        panel.get_by_role('button', name='Refresh integration records').click()\n"
        enabled = "        expect(panel.get_by_role('button', name='Check accepted task for the same run')).to_be_enabled()\n"
        wake = "        panel.get_by_role('button', name='Check accepted task for the same run').click()\n"
        accepted = source.index('        accepted = D.accept_graph(host, fixture)\n')
        observed = source.index(observe, accepted)
        refreshed = source.index(refresh, accepted)
        checked = source.index(enabled, accepted)
        written = source.index(wake, accepted)
        audit = source.index("        starts = [a for a in host.audit if a['method'] == 'integrations.start']\n", accepted)
        assert accepted < observed < refreshed < checked < written < audit
        assert source.count(wake) == 1
        assert "require(not any(a['method'] == 'integrations.start' for a in host.audit))" in source[observed:refreshed]
        assert "require(len(starts) == 1 and starts[0]['response']['result']['run']['id'] == done['run']['id'])" in source[audit:]

    def test_watcher_terminal_precedes_fresh_ui_same_run_check(self):
        source = (ROOT / 'apps/desktop/tests/integration-round-history-real-ipc.py').read_text()
        self.assert_order(source)

    def test_active_context_ui_order_cannot_satisfy_the_terminal_precondition(self):
        source = (ROOT / 'apps/desktop/tests/integration-round-history-real-ipc.py').read_text()
        block = ("        panel.get_by_role('button', name='Refresh integration records').click()\n"
            "        expect(panel.get_by_role('button', name='Check accepted task for the same run')).to_be_enabled()\n"
            "        panel.get_by_role('button', name='Check accepted task for the same run').click()\n")
        self.assertEqual(source.count(block), 1)
        inverse = source.replace(block, '', 1).replace('        def read_terminal():\n', block + '        def read_terminal():\n', 1)
        with self.assertRaises(AssertionError):
            self.assert_order(inverse)


if __name__ == '__main__':
    unittest.main()
