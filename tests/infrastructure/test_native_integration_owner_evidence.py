"""Pure Native report/formatter controls; never construct Core policy or run OS."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import sys
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, relative):
    spec = importlib.util.spec_from_file_location(name, ROOT / relative)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


N = load('_integration_owner_pure', 'scripts/native-owner-dialog-smoke.py')
P = load('_integration_owner_projection_pure', 'scripts/record-integration-owner-ci-evidence.py')


def group(phase):
    before = {'schema_version': 17, 'sequence': 23, 'policies': 0, 'runs': 0, 'attempts': 0, 'ref_fences': 0}
    after = dict(before, policies=1, sequence=25) if phase == 'approve' else dict(before)
    return {'read_only': True, 'phase': phase, 'before': before, 'after': after,
            'proposal_matches': True, 'proposal_result_matches': True, 'policy_matches': True,
            'owner_event_matches': True, 'policy_event_matches': True,
            'owner_event_sequence': 24 if phase == 'approve' else None,
            'policy_event_sequence': 25 if phase == 'approve' else None}


def report(phase):
    return {'status': 'os_button_paths_completed_database_check_passed',
            'dimension': 'macos_own_process_real_sheet', 'provider_model_calls': 'not_run',
            'automated_fixture_not_human_owner': True, 'integration_approval_database': group(phase)}


def proposal():
    # Typed display fixture only, deliberately not a valid Native/Core authority.
    return {'id': 'proposal-integration', 'kind': 'integration', 'revision': 1, 'consumed': False,
            'snapshot': {'intent': {'task_id': 'DISPLAY_ONLY'},
                         'warning': '中文雪😀 e\u0301 "quoted"\nALL fields remain visible {ACTION} {page}'}}


class IntegrationDatabaseContractControls(unittest.TestCase):
    def test_separate_exact_mutation_container_and_cancel_unchanged(self):
        self.assertEqual(len(N.INTEGRATION_GROUP_FIELDS), 11)
        for phase in ['approve', 'cancel']:
            self.assertIsNone(N.integration_database_error(group(phase), phase, 23))
            for field in N.INTEGRATION_GROUP_FIELDS:
                value = group(phase); del value[field]
                self.assertIsNotNone(N.integration_database_error(value, phase, 23), field)
            value = group(phase); value['unchanged'] = True
            self.assertIsNotNone(N.integration_database_error(value, phase, 23))
        self.assertIsNotNone(N.integration_database_error(group('approve'), 'cancel', 23))

    def test_every_count_is_exact_u64_and_schema17_never_a_bool_float_or_range(self):
        for checkpoint in ['before', 'after']:
            for field in N.INTEGRATION_CHECKPOINT_FIELDS:
                for bad in [True, False, 17.0, '17', -1, 1 << 64, None]:
                    value = group('approve'); value[checkpoint][field] = bad
                    self.assertIsNotNone(N.integration_database_error(value, 'approve', 23), (checkpoint, field, bad))
            for schema in [15, 16, 18]:
                value = group('approve'); value[checkpoint]['schema_version'] = schema
                self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))
            for mode in ['extra', 'missing']:
                value = group('approve')
                if mode == 'extra': value[checkpoint]['raw_rpc'] = 'PRIVATE_CANARY'
                else: del value[checkpoint]['runs']
                self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))

    def test_each_native_binding_requires_boolean_true_without_coercion(self):
        for field in ['read_only', 'proposal_matches', 'proposal_result_matches', 'policy_matches', 'owner_event_matches', 'policy_event_matches']:
            for bad in [False, 1, 1.0, None, 'true']:
                value = json.loads(json.dumps(group('approve'))); value[field] = bad
                self.assertIsNotNone(N.integration_database_error(value, 'approve', 23), (field, bad))

    def test_original_zero_domain_and_sequence_bindings_cannot_be_forged(self):
        for checkpoint in ['before', 'after']:
            for field in ['runs', 'attempts', 'ref_fences']:
                value = group('approve'); value[checkpoint][field] = 1
                self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))
        for bad in [0, 22, 24, True, 23.0, -1, 1 << 64]:
            self.assertIsNotNone(N.integration_database_error(group('approve'), 'approve', bad))
        for count in [0, 2]:
            value = group('approve'); value['after']['policies'] = count
            self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))
        value = group('approve'); value['before']['policies'] = 1
        self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))

    def test_trusted_owner_before_policy_order_and_typed_sequences(self):
        for owner, policy, after in [(23, 25, 25), (25, 24, 25), (24, 24, 25), (24, 26, 25),
                                      (24.0, 25, 25), (24, 25.0, 25), (True, 25, 25), (None, 25, 25)]:
            value = group('approve'); value['owner_event_sequence'] = owner
            value['policy_event_sequence'] = policy; value['after']['sequence'] = after
            self.assertIsNotNone(N.integration_database_error(value, 'approve', 23))

    def test_cancel_never_relabels_mutation_or_leaves_event_proof(self):
        for key, bad in [('owner_event_sequence', 24), ('policy_event_sequence', 25), ('owner_event_sequence', False)]:
            value = group('cancel'); value[key] = bad
            self.assertIsNotNone(N.integration_database_error(value, 'cancel', 23))
        for field in ['sequence', 'policies', 'runs', 'attempts', 'ref_fences']:
            value = group('cancel'); value['after'][field] += 1
            self.assertIsNotNone(N.integration_database_error(value, 'cancel', 23))


class IntegrationFormatterControls(unittest.TestCase):
    def source(self):
        path = Path(os.environ.get('AGENTBATON_INTEGRATION_REVIEW_SOURCE', ROOT / 'apps/desktop/src-tauri/src/integration_approval.rs'))
        return path.read_text()

    def fixture(self, phase='approve'):
        p = proposal()
        return N.integration_fixture(p, {'selection': p['snapshot']['intent'], 'idempotencyKey': N.INTEGRATION_KEY},
            Path('/private/generated/runtime'), Path('/private/generated/output'), {'sequence': 23}, self.source(), phase)

    def test_real_production_captions_preserve_full_utf8_json_and_original_placeholders(self):
        p = proposal(); p['snapshot']['warning'] *= 300
        messages = N.integration_messages(p, self.source())
        raw = N.compact(p['snapshot'])
        self.assertGreater(len(messages), 2)
        chunks = [message.split('\n\n', 3)[-1] for message in messages[:-1]]
        # Caption has a bilingual ACTION with internal blank lines; extract at
        # the exact end-of-caption boundary, not a substring of the payload.
        chunks = [message.split('Cancel creates no approval or rejection receipt.\n\n', 1)[1] for message in messages[:-1]]
        self.assertEqual(''.join(chunks).encode(), raw)
        self.assertTrue(all(len(c.encode()) <= 12288 for c in chunks))
        self.assertIn('{ACTION} {page}', ''.join(chunks))
        self.assertIn('独立本地合入批准', messages[-1])
        self.assertIn('FINAL independent integration approval', messages[-1])
        self.assertTrue(all(len(m.encode()) <= 16384 for m in messages))

    def test_exact_descriptor_keys_nested_pages_and_utf8_boundary_are_bounded(self):
        fixture, pages, contexts = self.fixture()
        self.assertEqual(set(fixture), N.INTEGRATION_DESCRIPTOR_FIELDS)
        self.assertEqual(len(fixture), 13)
        self.assertEqual(fixture['kind'], N.INTEGRATION_KIND)
        self.assertEqual(fixture['snapshot_sha256'], N.digest(fixture['proposal']['snapshot']))
        self.assertEqual(fixture['presentation_sha256'], N.digest(pages))
        self.assertEqual(fixture['presentation_count'], len(pages))
        self.assertTrue(all(len(page.encode()) <= 2048 for page in pages))
        self.assertEqual(len(pages), len(contexts))
        self.assertEqual(len(fixture['review_nonce']), 32)
        self.assertTrue(contexts[-1]['outer_final'] and contexts[-1]['inner_final'])

    def test_renderer_selection_key_or_phase_cannot_change_the_fixed_descriptor(self):
        p = proposal()
        for invoke in [{'selection': {}, 'idempotencyKey': N.INTEGRATION_KEY},
                       {'selection': p['snapshot']['intent'], 'idempotencyKey': 'proposal:' + p['id']},
                       {'selection': p['snapshot']['intent'], 'idempotencyKey': N.INTEGRATION_KEY, 'approve': True}]:
            with self.assertRaises(ValueError):
                N.integration_fixture(p, invoke, Path('/r'), Path('/o'), {'sequence': 23}, self.source(), 'approve')
        for phase in ['grant', 'factory', 'before', None]:
            with self.assertRaises(ValueError): self.fixture(phase)
        for bad in [True, -1, 1 << 64]:
            with self.assertRaises(ValueError):
                N.integration_fixture(p, {'selection': p['snapshot']['intent'], 'idempotencyKey': N.INTEGRATION_KEY},
                    Path('/r'), Path('/o'), {'sequence': bad}, self.source(), 'approve')

    def test_native_ready_still_requires_exact_text_title_layout_nonce_and_all_contexts(self):
        fixture, pages, contexts = self.fixture()
        index = 0
        value = {'phase': 'approve', 'presentation_index': index, 'presentation_count': len(pages),
                 'proposal_id': fixture['proposal']['id'], 'proposal_revision': 1,
                 'message': pages[index], 'title': N.INTEGRATION_TITLE,
                 'parent_matches': True, 'visible': True, 'complete_text_visible': True,
                 'review_nonce': fixture['review_nonce'], 'integration_page': contexts[index]}
        self.assertIsNone(N.integration_presentation_error(value, fixture, pages, contexts, 0))
        for field, bad in [('message', pages[0].strip() + ' changed'), ('title', N.FACTORY_TITLE),
                           ('parent_matches', False), ('visible', False), ('complete_text_visible', 1),
                           ('review_nonce', 'b' * 32), ('presentation_index', False), ('proposal_revision', 1.0)]:
            changed = json.loads(json.dumps(value)); changed[field] = bad
            self.assertIsNotNone(N.integration_presentation_error(changed, fixture, pages, contexts, 0), field)
        for field in contexts[0]:
            changed = json.loads(json.dumps(value))
            changed['integration_page'][field] = (not contexts[0][field] if type(contexts[0][field]) is bool
                else contexts[0][field] + 1 if type(contexts[0][field]) is int else '0' * 64)
            self.assertIsNotNone(N.integration_presentation_error(changed, fixture, pages, contexts, 0), field)

    def test_original_envelope_and_cap_refuse_incomplete_or_oversize_display(self):
        for field, bad in [('kind', 'factory'), ('consumed', True), ('revision', True), ('snapshot', None)]:
            value = proposal(); value[field] = bad
            with self.assertRaises(ValueError): N.integration_messages(value, self.source())
        value = proposal(); value['snapshot']['warning'] = 'x' * 1048576
        with self.assertRaises(ValueError): N.integration_messages(value, self.source())
        with self.assertRaises((ValueError, IndexError)): N.integration_messages(proposal(), 'unsupported formatter')


class IntegrationFinitePublisherControls(unittest.TestCase):
    def tree(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        root = Path(directory.name).resolve(); root.chmod(0o700)
        private = root / 'private'; private.mkdir(mode=0o700)
        for phase in ['cancel', 'approve']:
            parent = private / phase; parent.mkdir(mode=0o700)
            p = parent / 'owner-dialog.json'; p.write_text(json.dumps(report(phase))); p.chmod(0o600)
        return root, private

    def invoke(self, root, private, output='public.json', extra=None):
        args = ['--private', str(private), '--output', str(root / output), '--baseline-sequence', '23']
        return P.main(args + ([] if extra is None else extra))

    def test_projection_is_finite_and_does_not_claim_independent_os_cleanup_authority(self):
        projected = P.project(report('cancel'), report('approve'), 23)
        self.assertEqual(projected['status'], 'valid_native_report_projection')
        self.assertEqual(projected['origin'], 'native_report_asserted')
        self.assertFalse(projected['authority_or_runtime_acceptance'])
        self.assertFalse(projected['private_history_published'])
        self.assertEqual(projected['visual_review'], 'not_established_by_collector')
        self.assertEqual(projected['driver_cleanup'], 'not_established_by_collector')
        self.assertEqual([v['policy_count'] for v in projected['phases']], [0, 1])
        with self.assertRaises(ValueError):
            P.project(dict(report('cancel'), raw_rpc='PRIVATE_CANARY'), report('approve'), 23)
        for phase in ['cancel', 'approve']:
            value = report(phase); value['status'] = 'os_button_paths_completed_database_check_pending'
            with self.assertRaises(ValueError):
                P.project(value if phase == 'cancel' else report('cancel'), value if phase == 'approve' else report('approve'), 23)

    def test_real_file_main_reads_only_fixed_private_reports_and_publishes_exclusive_600(self):
        root, private = self.tree()
        (private / 'daemon.log').write_text('PRIVATE_STDERR_PATH_TOKEN_CANARY')
        with mock.patch.object(N.subprocess, 'Popen', side_effect=AssertionError('no Actor/OS')):
            self.assertEqual(self.invoke(root, private), 0)
        public = root / 'public.json'
        self.assertEqual(public.stat().st_mode & 0o777, 0o600)
        self.assertLess(len(public.read_bytes()), 131072)
        self.assertNotIn('PRIVATE_', public.read_text())
        before = public.read_bytes(); self.assertEqual(self.invoke(root, private), 1)
        self.assertEqual(before, public.read_bytes())

    def test_private_fd_rejects_symlink_hardlink_mode_fifo_parent_alias_and_missing(self):
        for mode in ['symlink', 'hardlink', 'filemode', 'dirmode', 'fifo', 'missing', 'oversize', 'parentalias']:
            root, private = self.tree(); path = private / 'approve/owner-dialog.json'
            if mode == 'symlink':
                raw = path.read_bytes(); path.unlink(); (root/'target').write_bytes(raw); path.symlink_to(root/'target')
            elif mode == 'hardlink': os.link(path, root/'hardlink')
            elif mode == 'filemode': path.chmod(0o644)
            elif mode == 'dirmode': path.parent.chmod(0o755)
            elif mode == 'fifo': path.unlink(); os.mkfifo(path, 0o600)
            elif mode == 'missing': path.unlink()
            elif mode == 'oversize': path.write_bytes(b'x'*65537)
            else:
                path.parent.rename(private/'original-approve'); (private/'approve').symlink_to(private/'original-approve', target_is_directory=True)
            self.assertEqual(self.invoke(root, private), 1, mode)
            self.assertFalse((root/'public.json').exists(), mode)

    def test_malformed_duplicate_float_or_false_native_proof_refuses_without_raw_error(self):
        for raw in ['{"status":1,"status":2}', '{broken', 'NaN', json.dumps(dict(report('approve'), extra='PRIVATE_CANARY')),
                    json.dumps(report('approve')).replace('"sequence": 25', '"sequence": 25.0')]:
            root, private = self.tree(); (private/'approve/owner-dialog.json').write_text(raw)
            self.assertEqual(self.invoke(root, private), 1)
            self.assertFalse((root/'public.json').exists())
        root, private = self.tree()
        with mock.patch.object(P.sys, 'stderr') as err:
            self.assertEqual(self.invoke(root, private, extra=['--PRIVATE_CANARY']), 1)
            self.assertNotIn('PRIVATE_CANARY', str(err.write.call_args_list))

    def test_relative_unknown_baseline_and_existing_output_symlink_do_not_bypass_guard(self):
        root, private = self.tree()
        args = ['--private', str(private), '--output', str(root/'public.json'), '--baseline-sequence']
        for raw in ['023', '23.0', 'true', '-1', str(1 << 64), 'PRIVATE_CANARY']:
            self.assertEqual(P.main(args+[raw]), 1)
        self.assertEqual(P.main(['--private', str(private), '--output', 'relative.json', '--baseline-sequence', '23']), 1)
        target = root/'target'; target.write_text('original'); (root/'public.json').symlink_to(target)
        self.assertEqual(self.invoke(root, private), 1); self.assertEqual(target.read_text(), 'original')


F = load('_integration_actual_seed_source_controls', 'scripts/integration-owner-fixture.py')


class IntegrationSeedSourceControls(unittest.TestCase):
    def test_generated_resource_bootstrap_cannot_finalize_target_or_unknown_approval(self):
        client = F.ResourceClient(Path('/unexecuted-runtime'), N)
        with mock.patch.object(F.socket, 'socket', side_effect=AssertionError('must refuse before transport')):
            for identifier in ['integration-proposal', 'foreign']:
                with self.assertRaises(ValueError):
                    client.call('approvals.finalize', {'proposal_id': identifier}, 'proposal:'+identifier, 1)
            for method in ['integrations.start', 'integration_policies.revoke', 'sessions.send', 'approvals.decline', 'generic_ipc']:
                with self.assertRaises(ValueError): client.call(method, {})
            with self.assertRaises(ValueError): client.approved('integration_policies.propose', {}, 'never-auto-approve')
        self.assertNotIn('integration', F.ResourceClient.PROPOSALS.values())

    def test_private_binary_copy_preserves_source_and_rejects_alias_fifo_or_existing_destination(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve(); source = root/'generated-source'
            source.write_bytes(b'never executed generated tool bytes'); source.chmod(0o500)
            before = source.stat(); original = source.read_bytes()
            observed = F.tool_copy(source, root/'copy')
            self.assertEqual(observed['sha256'], hashlib.sha256(original).hexdigest())
            self.assertEqual((root/'copy').stat().st_mode & 0o777, 0o500)
            self.assertEqual((root/'copy').stat().st_nlink, 1)
            self.assertEqual(source.read_bytes(), original)
            self.assertEqual((before.st_dev, before.st_ino, before.st_mode, before.st_mtime_ns, before.st_ctime_ns),
                (source.stat().st_dev, source.stat().st_ino, source.stat().st_mode, source.stat().st_mtime_ns, source.stat().st_ctime_ns))
            with self.assertRaises(FileExistsError): F.tool_copy(source, root/'copy')
            alias = root/'alias'; alias.symlink_to(source)
            with self.assertRaises(ValueError): F.tool_copy(alias, root/'alias-copy')
            fifo = root/'fifo'; os.mkfifo(fifo,0o700)
            with self.assertRaises(ValueError): F.tool_copy(fifo, root/'fifo-copy')

    def test_actual_sqlite_raw_reader_is_readonly_bounded_and_refuses_missing_or_extra_tables(self):
        import sqlite3
        H = load('_integration_actual_table_names', 'scripts/prepare-schema16-migration-fixture.py')
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw).resolve(); path = root/'baton.sqlite3'
            with sqlite3.connect(path) as db:
                db.execute('PRAGMA user_version=17')
                # Inert count/raw reader fixture, not a genuine Core schema.
                for table in sorted((*H.SCHEMA_TABLES, *N.INTEGRATION_TABLES)):
                    if table == 'sqlite_sequence': continue
                    db.execute('CREATE TABLE '+table+'(sequence INTEGER PRIMARY KEY AUTOINCREMENT,document TEXT)' if table == 'events'
                        else 'CREATE TABLE '+table+'(id TEXT,document TEXT)')
                db.execute("INSERT INTO projects VALUES('original',?)", ('原source',))
            before = path.read_bytes(); value = F.database(root)
            self.assertEqual(len(value['counts']), 59)
            self.assertEqual(value['counts']['projects'], 1)
            self.assertEqual(value['schema_version'], 17)
            self.assertTrue(value['read_only']); self.assertEqual(path.read_bytes(), before)
            for old in [16,18]:
                with sqlite3.connect(path) as db: db.execute('PRAGMA user_version='+str(old))
                with self.assertRaises(ValueError): F.database(root)
            with sqlite3.connect(path) as db:
                db.execute('PRAGMA user_version=17'); db.execute('CREATE TABLE unexpected(id INTEGER)')
            with self.assertRaises(ValueError): F.database(root)
            with sqlite3.connect(path) as db:
                db.execute('DROP TABLE unexpected'); db.execute('DROP TABLE integration_attempts')
            with self.assertRaises(ValueError): F.database(root)

    def test_raw_predecode_total_limit_refuses_inert_oversize_history(self):
        import sqlite3
        H = load('_integration_actual_table_names_large', 'scripts/prepare-schema16-migration-fixture.py')
        with tempfile.TemporaryDirectory() as raw:
            root=Path(raw).resolve(); path=root/'baton.sqlite3'
            with sqlite3.connect(path) as db:
                db.execute('PRAGMA user_version=17')
                for table in sorted((*H.SCHEMA_TABLES,*N.INTEGRATION_TABLES)):
                    if table=='sqlite_sequence':continue
                    db.execute('CREATE TABLE '+table+'(sequence INTEGER PRIMARY KEY AUTOINCREMENT,document TEXT)' if table=='events' else 'CREATE TABLE '+table+'(id TEXT,document TEXT)')
                db.execute("INSERT INTO projects VALUES('oversize',zeroblob(8388609))")
            with self.assertRaisesRegex(ValueError,'raw history'):F.database(root)

    def test_os_run_on_linux_refuses_before_any_process_or_output(self):
        with tempfile.TemporaryDirectory() as raw:
            output=Path(raw)/'not-created'
            with mock.patch.object(F.sys,'platform','linux'),mock.patch.object(F.subprocess,'Popen',side_effect=AssertionError('no product/OS')):
                with self.assertRaisesRegex(ValueError,'NOT_RUN'):F.run(N,Path('/binary'),Path('/daemon'),Path('/companion'),output)
            self.assertFalse(output.exists())

    def test_real_cli_branch_requires_explicit_integration_and_cannot_change_old_kind_options(self):
        import argparse,ast
        tree=ast.parse((ROOT/'scripts/native-owner-dialog-smoke.py').read_text())
        actual_main=next(n for n in tree.body if isinstance(n,ast.If) and ast.unparse(n.test)=="__name__ == '__main__'")
        code=compile(ast.Module(body=actual_main.body,type_ignores=[]),'<actual-driver-cli>','exec')
        namespace={'argparse':argparse,'Path':Path,'sys':sys,'save':mock.Mock(),
            'run':mock.Mock(),'run_factory':mock.Mock(),'run_integration':mock.Mock()}
        base=['script','--binary','/binary','--daemon','/daemon','--output','/unexecuted-output']
        with mock.patch.object(sys,'argv',base+['--kind','integration','--companion','/companion','--integration-cancel-at','1']):exec(code,namespace)
        namespace['run_integration'].assert_called_once_with(Path('/binary'),Path('/daemon'),Path('/companion'),Path('/unexecuted-output'),1)
        namespace['run'].assert_not_called();namespace['run_factory'].assert_not_called()
        for argv in [base+['--kind','integration'],base+['--kind','grant','--integration-cancel-at','1'],
                     base+['--kind','integration','--companion','/companion','--factory-cancel-at','1']]:
            with mock.patch.object(sys,'argv',argv),mock.patch('sys.stderr'),self.assertRaises(SystemExit):exec(code,namespace)

    def test_provider_is_fixed_complete_legacy_contract_without_an_arbitrary_command_or_provider(self):
        import ast
        ast.parse(F.PROVIDER)
        self.assertIn("assert v['schema_version']==1 and v['kind']=='agentbaton_goal_task_request'",F.PROVIDER)
        self.assertIn("Path('reports/report.txt').write_bytes(b'fixed report')",F.PROVIDER)
        self.assertIn("'stopReason':'end_turn'",F.PROVIDER)
        self.assertNotIn('subprocess',F.PROVIDER)
        self.assertNotIn('requests',F.PROVIDER)
        self.assertNotIn('approvals.',F.PROVIDER)


if __name__ == '__main__':
    unittest.main()
