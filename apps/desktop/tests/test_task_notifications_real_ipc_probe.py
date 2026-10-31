"""Pure notification-probe contract tests; no browser, Core, Node, SQL or network."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
PATH = Path(__file__).with_name('task_notifications_real_ipc_probe.py')


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


P = load('_notifications_probe_pure', PATH)
D = load('_notifications_base_pure', PATH.with_name('integration-real-ipc.py'))
EVIDENCE = load('_notifications_original_projector_fixture', ROOT / 'tests/infrastructure/test_integration_browser_ci_evidence.py')
E = EVIDENCE.E


def replies():
    task = {'id': 'task-one', 'project_id': 'project-one', 'state': 'accepted', 'revision': 3,
        'spec_revision': 1, 'acceptance_ref': 'acceptance-one', 'acceptance_sequence': 9}
    events = [{'event_id': 'event-stop', 'aggregate_id': 'session-one', 'sequence': 8, 'revision': 2,
        'kind': 'session.stopped', 'source': 'baton_supervisor', 'occurred_at': '2026-10-31T00:00:00Z',
        'payload': {'task_accepted': False}},
        {'event_id': 'event-accepted', 'aggregate_id': task['id'], 'sequence': 9, 'revision': 3,
        'kind': 'task.accepted', 'source': 'baton_goal_acceptance', 'occurred_at': '2026-10-31T00:00:01Z',
        'payload': {'acceptance_ref': 'acceptance-one'}}]
    values = {'system.snapshot': {'api_version': 1, 'database_schema_version': 17, 'sequence': 10, 'tasks': [task]},
        'sessions.list': [{'id': 'session-one', 'task_id': task['id'], 'project_id': task['project_id'],
            'revision': 2, 'state': 'stopped'}],
        'events.subscribe': {'sequence': 10, 'next_cursor': 10, 'gap': False, 'events': events}}
    return [{'method': method, 'response': {'result': value}, 'params': {}} for method, value in values.items()]


def observe(audit):
    return P.actual_observations(audit, 'task-one', 'project-one', {'id': 'acceptance-one', 'sequence': 9})


class RealObservationContracts(unittest.TestCase):
    def test_real_projection_and_multiple_mapped_event_references_required(self):
        task, mapped, accepted, witness = observe(replies())
        self.assertEqual(task['acceptance_ref'], 'acceptance-one')
        self.assertEqual(set(mapped), {'event-stop', 'event-accepted'})
        self.assertEqual((accepted['kind'], accepted['sequence']), ('task.accepted', 9))
        self.assertRegex(witness, '^[a-f0-9]{64}$')

    def test_payload_cannot_supply_task_mapping_acceptance_or_stopped_projection(self):
        for mutate in [lambda a: a[0]['response']['result']['tasks'][0].update(state='running'),
            lambda a: a[0]['response']['result']['tasks'][0].update(acceptance_ref=None),
            lambda a: a[0]['response']['result']['tasks'][0].update(acceptance_sequence=True),
            lambda a: a[1]['response']['result'][0].update(state='cancel_requested'),
            lambda a: a[1]['response']['result'][0].update(task_id='foreign'),
            lambda a: a[2]['response']['result']['events'][0].update(aggregate_id='foreign', payload={'task_id': 'task-one'}),
            lambda a: a[2]['response']['result']['events'][1].update(kind='session.update', payload={'accepted': True})]:
            audit = replies(); mutate(audit)
            with self.subTest(audit=audit), self.assertRaises(P.Refusal): observe(audit)

    def test_sequence_gap_partial_bounds_duplicate_ids_and_foreign_task_fail_closed(self):
        for mutate in [lambda a: a[0]['response']['result']['tasks'].append({'id': 'foreign'}),
            lambda a: a[2]['response']['result'].update(gap=True),
            lambda a: a[2]['response']['result'].update(byte_limited=True),
            lambda a: a[2]['response']['result'].update(sequence=True),
            lambda a: a[2]['response']['result']['events'][0].update(sequence=9007199254740991),
            lambda a: a[2]['response']['result']['events'][0].update(event_id='event-accepted'),
            lambda a: a[2]['response']['result']['events'].extend(copy.deepcopy(a[2]['response']['result']['events']) * 250)]:
            audit = replies(); mutate(audit)
            with self.subTest(), self.assertRaises(P.Refusal): observe(audit)

    def test_existing_read_allowlist_is_exact_and_rejects_any_write_or_unknown_method(self):
        self.assertEqual(P.READS, E.HTTP_READS)
        self.assertEqual(P.readonly_counts([{'method': method} for method in sorted(P.READS)]), dict.fromkeys(P.READS, 1))
        for method in ['approvals.finalize', 'integrations.start', 'sessions.send', 'tasks.create', 'generic_rpc', 'events_subscribe']:
            with self.subTest(method=method), self.assertRaises(P.Refusal): P.readonly_counts([{'method': method}])
        with self.assertRaises(P.Refusal): P.readonly_counts([{'method': 'system.snapshot'}] * 100001)

    def test_one_absolute_deadline_decreases_and_cannot_restart_on_retry(self):
        with patch.object(P.time, 'monotonic', side_effect=[10.0, 11.0, 25.0]):
            self.assertEqual(P.remaining_ms(25), 15000)
            self.assertEqual(P.remaining_ms(25), 14000)
            with self.assertRaises(P.Refusal): P.remaining_ms(25)
        self.assertEqual(P.SECONDS, 15)
        self.assertFalse(P.exact({'i': True}, {'i': 1}))

    def test_probe_cannot_pass_before_exact_original_owned_cleanup(self):
        base = {'status': 'passed', 'cleanup': dict.fromkeys(P.CLEANUP, True)
            | {'daemon_joined': {'exit': 0, 'joined': True, 'forced_kill': False}}}
        report = P.pending_report('a' * 40, {'sha256': 'b' * 64})
        self.assertEqual(set(report), P.REPORT_KEYS)
        report['flow_complete'] = True
        report['checks'] = [{'name': name, 'status': 'passed'} for name in P.CHECKS]
        report['screenshots'] = [{'name': name} for name in P.PNG_NAMES]
        self.assertTrue(P.seal_report(report, base))
        for mutate in [lambda b: b.update(status='failed_or_interrupted'),
            lambda b: b['cleanup'].update(fixture_removed=False),
            lambda b: b['cleanup'].update(http_thread_joined=1),
            lambda b: b['cleanup']['daemon_joined'].update(exit=False),
            lambda b: b['cleanup']['daemon_joined'].update(forced_kill=True)]:
            broken = copy.deepcopy(base); mutate(broken)
            with self.subTest(): self.assertFalse(P.seal_report(copy.deepcopy(report), broken))
        for key in ('checks', 'screenshots'):
            broken = copy.deepcopy(report); broken[key].pop()
            self.assertFalse(P.seal_report(broken, base))


class OriginalFlowPreserved(unittest.TestCase):
    def tree(self):
        return ast.parse(PATH.with_name('integration-real-ipc.py').read_text())

    def test_original_authority_boundary_runtime_timeouts_permissions_and_evidence_functions_unchanged(self):
        hashes = {
            'Boundary': '2577eb0fa76aaa058fc1e1e59bf255fcfea111d898b6cb5582d17a1920479fa5',
            'Host': '218fe7fb405c65a3a941bb87857e821f22decda7b1ab9c98aca53e573f225348',
            'Reviews': '637f234dfec4b57000f2d9c5ebcaa2f56a65171c2d41819789fb47a000e3ca17',
            'preflight': 'c5dbb2a0fc5da89778f2773a0b1b76667f398f9546a1fb833c4da8b297cf573b',
            'bootstrap': '33c07dbc2f6aee20752c62305b16e3f93364087a765440f60bc72e58194d0af5',
            'terminal_evidence': 'f445bf1615507240bcecf28c2f1c0409050a023a30b6c307e9cd6bc1bb308560',
            'accept_graph': '3f044ec949eba8b86c4f96a665a1b5ee157b08257d3a54c3d9e3f91fa9e8ed87',
            'observe': '7e788afe4c68b33165857e854829079118f9c8bb2009993264601c3c0a22f5e4',
            'parse_http': 'c194a3fd981a58fe3dd55fb3028a0c6478a4449b2193952e6feefc444c4cb9fb',
            'readonly_abort': 'b25d8c988cf40e44804680fee90a94de1a1cba46ce06a858aa2ebb1045a940a5',
        }
        for node in self.tree().body:
            if getattr(node, 'name', None) in hashes:
                self.assertEqual(hashlib.sha256(ast.dump(node, include_attributes=False).encode()).hexdigest(), hashes[node.name], node.name)

    def test_remove_only_optional_probe_statements_restores_entire_original_run_case_ast(self):
        node = next(node for node in self.tree().body if getattr(node, 'name', None) == 'run_case')
        class StripProbe(ast.NodeTransformer):
            def visit_Assign(self, node):
                if any(isinstance(target, ast.Name) and target.id.startswith('notification_') for target in node.targets): return None
                return self.generic_visit(node)
            def visit_If(self, node):
                if any(isinstance(value, ast.Name) and value.id == 'notification_probe' for value in ast.walk(node.test)): return None
                return self.generic_visit(node)
            def visit_Return(self, node):
                if isinstance(node.value, ast.BoolOp) and isinstance(node.value.op, ast.And) and any(
                    isinstance(value, ast.Name) and value.id == 'notification_probe' for value in ast.walk(node.value.values[-1])):
                    node.value = node.value.values[0]
                return node
        stripped = StripProbe().visit(node)
        self.assertEqual(hashlib.sha256(ast.dump(stripped, include_attributes=False).encode()).hexdigest(),
            '15e87db4229eb00e7f24fa9a2dce4c277be1b8e4533bf499f8af00570066508e')

    def test_flag_is_normal_only_and_final_counts_include_probe_reads_without_freezing(self):
        source = PATH.with_name('integration-real-ipc.py').read_text()
        self.assertIn("getattr(args, '_notification_probe', None) if not failing else None", source)
        self.assertIn("if args.notification_probe and args.case != 'command-failure':", source)
        self.assertLess(source.index("checked('actual built component displays separate Task/integration result"), source.index('notification_probe.run_probe('))
        self.assertLess(source.index('notification_probe.run_probe('), source.index("report['http_method_counts'] = dict(Counter"))
        self.assertIn("and (notification_probe is None or notification_passed)", source)
        self.assertIn('notification_probe_source(attribution[\'source\']) == args._notification_probe_source', source)

    def test_fixed_source_module_refuses_changed_bytes_and_uses_no_cli_import_path(self):
        raw = PATH.read_bytes(); head = 'a' * 40
        with patch.object(D.subprocess, 'check_output', side_effect=[head + '\n', b'', raw]):
            self.assertEqual(D.notification_probe_source(head)['sha256'], hashlib.sha256(raw).hexdigest())
        for values in [[head + '\n', b'', b'changed'], ['b' * 40 + '\n'], [head + '\n', b'M changed']]:
            with patch.object(D.subprocess, 'check_output', side_effect=values), self.assertRaises(D.Refusal): D.notification_probe_source(head)
        source = PATH.with_name('integration-real-ipc.py').read_text()
        self.assertIn("with_name('task_notifications_real_ipc_probe.py')", source)
        self.assertNotIn("add_argument('--probe-path'", source)


class OriginalProjectorContract(unittest.TestCase):
    def test_nine_checks_twelve_original_pngs_and_original_full_collector_survive_additional_probe_reads(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve(); private = EVIDENCE.seed(root)
            path = private / 'driver/normal/report.json'; report = json.loads(path.read_text())
            report['http_method_counts'].update({method: 7 for method in P.READS})
            EVIDENCE.write(path, report)
            # Additional private probe artifacts must never expand the old upload list.
            EVIDENCE.write(private / 'driver/normal/notification-probe/report.json', {'private': EVIDENCE.SECRET})
            EVIDENCE.write(private / 'driver/normal/notification-probe/accepted-task-desktop.png', EVIDENCE.IMAGES['desktop'])
            summary, payloads = E.collect(private, EVIDENCE.HEAD)
            self.assertEqual(summary['checks'], 9)
            self.assertEqual([case['checks'] for case in summary['cases']], [5, 4])
            self.assertEqual(set(payloads), E.PUBLIC_NAMES)
            self.assertEqual(len(payloads), 12)
            self.assertNotIn(EVIDENCE.SECRET, json.dumps(summary))
            self.assertEqual(E.MAX_REPORT, 2097152); self.assertEqual(E.MAX_STATUS, 65536)
            self.assertEqual(E.MAX_PNG, 16777216); self.assertEqual(E.MAX_TOTAL, 67108864)

    def test_original_projector_rejects_new_writes_unknown_reads_and_out_of_bound_counts(self):
        for method, count in [('integrations.start', 1), ('sessions.send', 1), ('unknown.read', 1),
            ('events.subscribe', 100001), ('system.snapshot', True), ('system.snapshot', 0)]:
            with self.subTest(method=method, count=count), tempfile.TemporaryDirectory() as temporary:
                private = EVIDENCE.seed(Path(temporary).resolve())
                path = private / 'driver/normal/report.json'; report = json.loads(path.read_text())
                report['http_method_counts'][method] = count
                EVIDENCE.write(path, report)
                with self.assertRaises(E.Refused): E.collect(private, EVIDENCE.HEAD)


if __name__ == '__main__':
    unittest.main()
