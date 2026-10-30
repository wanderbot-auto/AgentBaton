"""Pure finite-driver controls only; no Core, browser, Git fixture or authority."""
import ast
import copy
import importlib.util
import json
from pathlib import Path
import socket
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('_integration_browser_pure', ROOT / 'apps/desktop/tests/integration-real-ipc.py')
D = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = D
SPEC.loader.exec_module(D)


def ref():
    return {'kind': 'command', 'policy_id': 'command-display-only', 'revision': 1}


def boundary():
    return D.Boundary('task-display-only', 'project-display-only', 'scope-display-only', 'a' * 40, [ref()])


def selection():
    reserve = {'runtime_ms': 5000, 'output_bytes': 1024, 'tokens': None, 'cost_microunits': None}
    return {'task_id': 'task-display-only', 'expected_task_revision': 2, 'expected_spec_revision': 1,
        'goal_scope_id': 'scope-display-only', 'expected_scope_revision': 1, 'host_id': 'local',
        'target_ref': D.TARGET, 'expected_target_head': 'a' * 40, 'max_rounds': 1,
        'on_target_advance': 'block', 'ttl_ms': 60000,
        'stage_reservations': {'preparation': copy.deepcopy(reserve), 'cas': copy.deepcopy(reserve),
        'validators': [{'validator': ref(), 'reservation': copy.deepcopy(reserve)}]}}


def envelope(command='integration_policy_approve', args=None):
    return {'command': command, 'args': {'selection': selection(), 'idempotencyKey': 'same-key'} if args is None else args}


def context_and_proposal():
    # Symbolic display contracts, intentionally not real Core authority.
    definition = {'reference': ref(), 'rule_id': 'display', 'definition_digest': 'd' * 64,
        'definition_json': '{"display_only":true}', 'program': None, 'script': None}
    bindings = {'task_id': 'task-display-only', 'goal_scope_id': 'scope-display-only',
        'goal_scope_revision': 1, 'spec_revision': 1, 'workspace_binding': {'kind': 'task_managed',
        'receipt': {'task_id': 'task-display-only'}}, 'grant_lineage': [{'expires_at_ms': 9000}],
        'factory_scope_id': None, 'goal_scope_expires_at_ms': 9000, 'validators': [], 'binding_digest': 'b' * 64,
        'source': {'head': 'a' * 40, 'inode': '18446744073709551615'}}
    usage = {'root': {'tokens': None, 'cost_microunits': None}, 'project': {'tokens': None, 'cost_microunits': None}}
    context = {'bindings': copy.deepcopy(bindings), 'preview_only': True, 'shared_usage': usage}
    bindings['validators'] = [copy.deepcopy(definition)]
    bindings['binding_digest'] = 'c' * 64
    proposal = {'id': 'proposal-display-only', 'kind': 'integration', 'revision': 1,
        'consumed': False, 'created_at': 'display', 'expires_at_ms': 9000,
        'snapshot': {'intent': selection(), 'bindings': bindings, 'as_of_sequence': 3, 'shared_usage': usage,
        'authority_kind': 'authorized_local_integration', 'guarantees': copy.deepcopy(D.GUARANTEES),
        'warning': 'Display-only complete proposal bytes 中文雪😀'}}
    return context, definition, proposal


class FiniteBoundaryTests(unittest.TestCase):
    def test_proposal_route_only_has_exact_twelve_selection_fields(self):
        method, params, key, rev = boundary().route(envelope())
        self.assertEqual(method, 'integration_policies.propose')
        self.assertEqual(params, selection()); self.assertEqual(len(params), 12)
        self.assertEqual(key, 'same-key'); self.assertIsNone(rev)

    def test_every_direct_authority_or_generic_route_is_denied(self):
        for name in ['approvals.finalize', 'approvals_finalize', 'approvals.decline', 'approvals_decline',
            'rpc', 'integration_policies.propose', 'goal_node_approve', 'goal_graph_run_approve',
            'tasks_create', 'sessions_start', 'shell', 'integration_policy_confirm']:
            with self.subTest(name=name), self.assertRaises(D.Refusal):
                boundary().route(envelope(name, {'proposal_id': 'anything', 'token': 'CANARY', 'confirmed': True}))

    def test_extra_renderer_authority_is_rejected_before_transport(self):
        for field in ['approved', 'snapshot', 'snapshot_digest', 'caller', 'source_path', 'token', 'confirmed']:
            value = envelope(); value['args']['selection'][field] = True
            with self.subTest(field=field), self.assertRaises(D.Refusal): boundary().route(value)
        for field in ['token', 'confirmation', 'ownerApproved']:
            value = envelope(); value['args'][field] = True
            with self.assertRaises(D.Refusal): boundary().route(value)

    def test_foreign_task_scope_host_target_and_head_are_rejected(self):
        for field, wrong in [('task_id', 'other'), ('goal_scope_id', 'other'), ('host_id', 'remote'),
            ('target_ref', 'refs/heads/main'), ('expected_target_head', 'b' * 40)]:
            value = envelope(); value['args']['selection'][field] = wrong
            with self.subTest(field=field), self.assertRaises(D.Refusal): boundary().route(value)

    def test_integer_fields_do_not_accept_bool_float_null_or_unsafe(self):
        for field in ['expected_task_revision', 'expected_spec_revision', 'expected_scope_revision', 'max_rounds', 'ttl_ms']:
            for bad in [True, False, 1.0, None, -1, 0, 1 << 54]:
                value = envelope(); value['args']['selection'][field] = bad
                with self.subTest(field=field, bad=bad), self.assertRaises(D.Refusal): boundary().route(value)

    def test_validator_required_complete_no_duplicates_no_unknown_correctness(self):
        value = envelope(); value['args']['selection']['stage_reservations']['validators'] *= 2
        with self.assertRaises(D.Refusal): boundary().route(value)
        value = envelope(); value['args']['selection']['stage_reservations']['validators'][0]['validator']['policy_id'] = 'foreign'
        with self.assertRaises(D.Refusal): boundary().route(value)
        value = envelope(); value['args']['selection']['stage_reservations']['validators'] = []
        with self.assertRaises(D.Refusal): boundary().route(value)
        b = boundary(); b.required_validators = [{'kind': 'integrity', 'policy_id': 'required', 'revision': 1}]
        with self.assertRaises(D.Refusal): b.route(envelope())

    def test_reservations_keep_unknown_and_explicit_values_no_default_repair(self):
        value = envelope(); value['args']['selection']['stage_reservations']['preparation']['runtime_ms'] = 1000
        result = boundary().route(value)[1]
        self.assertEqual(result['stage_reservations']['preparation']['runtime_ms'], 1000)
        self.assertIsNone(result['stage_reservations']['preparation']['tokens'])
        for bad in [True, 1.5, -1, float('nan')]:
            value = envelope(); value['args']['selection']['stage_reservations']['cas']['tokens'] = bad
            with self.subTest(bad=bad), self.assertRaises(D.Refusal): boundary().route(value)

    def test_same_key_cannot_change_original_payload_or_domain_revision(self):
        b = boundary(); first = b.route(envelope()); self.assertEqual(b.route(envelope()), first)
        changed = envelope(); changed['args']['selection']['stage_reservations']['cas']['runtime_ms'] = 1000
        with self.assertRaises(D.Refusal): b.route(changed)
        b.policies.add('policy')
        args = {'policyId': 'policy', 'expectedRevision': 3, 'idempotencyKey': 'key'}
        self.assertEqual(b.route(envelope('integrations_start', args))[3], 3)
        args['expectedRevision'] = 4
        with self.assertRaises(D.Refusal): b.route(envelope('integrations_start', args))

    def test_record_mutations_require_observed_identity_and_exact_typed_arguments(self):
        for name in D.ID_READS | D.WRITES:
            entry = (D.ID_READS | D.WRITES)[name]; _, camel, _, collection = entry
            args = {camel: 'record'}
            if name in D.WRITES: args.update(expectedRevision=3, idempotencyKey='key')
            b = boundary()
            with self.assertRaises(D.Refusal): b.route(envelope(name, args))
            getattr(b, collection).add('record')
            method, params, key, rev = b.route(envelope(name, args))
            self.assertEqual(method, entry[0]); self.assertEqual(params, {entry[2]: 'record'})
            if name in D.WRITES: self.assertEqual(rev, 3)
            wrong = dict(args, caller=True)
            with self.assertRaises(D.Refusal): b.route(envelope(name, wrong))

    def test_foreign_lists_cannot_register_ids(self):
        b = boundary()
        with self.assertRaises(D.Refusal): b.observe('integration_policies.list', {'result': [
            {'id': 'p', 'selection': {'task_id': 'foreign', 'goal_scope_id': b.scope}}]})
        self.assertEqual(b.policies, set())
        with self.assertRaises(D.Refusal): b.observe('integrations.list', {'result': [
            {'id': 'r', 'task_id': b.task, 'policy_id': 'unseen'}]})
        self.assertEqual(b.runs, set())

    def test_loopback_exact_origin_host_content_type_and_byte_bound(self):
        origin = 'http://127.0.0.1:12345'; headers = {'Origin': origin, 'Host': origin[7:], 'Content-Type': 'application/json'}
        self.assertEqual(D.parse_http(D.compact(envelope()), headers, origin, boundary())[0], 'integration_policies.propose')
        for key, value in [('Origin', 'http://example.invalid'), ('Host', '127.0.0.1:12346'),
            ('Content-Type', 'text/plain'), ('Transfer-Encoding', 'chunked')]:
            with self.subTest(key=key), self.assertRaises(D.Refusal): D.parse_http(D.compact(envelope()),
                dict(headers, **{key: value}), origin, boundary())
        for raw in [b'', b' ' * (D.MAX_BODY + 1)]:
            with self.assertRaises(D.Refusal): D.parse_http(raw, headers, origin, boundary())

    def test_duplicate_json_nonfinite_and_nonobject_are_not_repaired(self):
        for raw in [b'{"command":"system_snapshot","command":"approvals_finalize"}', b'{"a":NaN}', b'{"a":Infinity}']:
            with self.assertRaises(D.Refusal): D.decode(raw)
        for value in [None, [], True, {'command': True}, {'command': 'system_snapshot', 'args': []}]:
            with self.assertRaises(D.Refusal): boundary().route(value)

    def test_readonly_abort_cannot_hide_unknown_write_or_core_failure(self):
        for stage in ['response_headers', 'response_body']:
            self.assertTrue(D.readonly_abort(BrokenPipeError(), 'integration_context.get', stage))
            for method in ['integration_policies.propose', 'integrations.start', 'integrations.cancel', None]:
                self.assertFalse(D.readonly_abort(ConnectionResetError(), method, stage))
        self.assertFalse(D.readonly_abort(BrokenPipeError(), 'integrations.get', 'core_call'))
        self.assertFalse(D.readonly_abort(RuntimeError(), 'integrations.get', 'response_body'))


class PrivateReviewTests(unittest.TestCase):
    def test_complete_proposal_passes_with_lossless_large_decimal_and_unicode(self):
        context, definition, proposal = context_and_proposal()
        facts = D.review_proposal(proposal, selection(), context, definition)
        self.assertEqual(facts['proposal_sha256'], D.digest(D.compact(proposal)))
        self.assertIn('18446744073709551615', D.compact(proposal).decode())
        self.assertIn('中文雪😀', D.compact(proposal).decode())

    def test_snapshot_mismatch_consumed_kind_missing_fields_and_extra_fields_refuse(self):
        for mutate in [lambda p: p.update(kind='goal'), lambda p: p.update(consumed=True),
            lambda p: p['snapshot']['intent'].update(max_rounds=2),
            lambda p: p['snapshot'].update(extra='FORBIDDEN'), lambda p: p.pop('created_at'),
            lambda p: p['snapshot']['bindings']['source'].update(inode=18446744073709551615),
            lambda p: p['snapshot']['guarantees'].update(remote_delivery_authorized=True),
            lambda p: p['snapshot']['bindings']['validators'][0].update(definition_json='changed')]:
            c, d, p = context_and_proposal(); mutate(p)
            with self.assertRaises((D.Refusal, KeyError)): D.review_proposal(p, selection(), c, d)

    def test_original_whole_value_cap_cannot_be_satisfied_by_digest_or_truncation(self):
        c, d, p = context_and_proposal(); p['snapshot']['warning'] = 'x' * D.MAX_REVIEW
        with self.assertRaises(D.Refusal): D.review_proposal(p, selection(), c, d)
        c, d, p = context_and_proposal(); p['snapshot'] = {'digest': 'a' * 64}
        with self.assertRaises(D.Refusal): D.review_proposal(p, selection(), c, d)

    def test_policy_receipt_must_bind_exact_proposal_selection_bindings_and_digest(self):
        _, _, p = context_and_proposal()
        policy = {'id': p['id'], 'approval_id': p['id'], 'revision': 1, 'revoked': False,
            'selection': selection(), 'bindings': copy.deepcopy(p['snapshot']['bindings']),
            'snapshot_digest': D.digest(D.compact(p['snapshot'])), 'expires_at_ms': 8000, 'created_at': 'display'}
        self.assertIsNone(D.verify_policy(policy, p, selection()))
        for field, bad in [('id', 'foreign'), ('approval_id', 'foreign'), ('revision', 2),
            ('revoked', True), ('snapshot_digest', '0' * 64), ('expires_at_ms', 10000)]:
            wrong = copy.deepcopy(policy); wrong[field] = bad
            with self.subTest(field=field), self.assertRaises(D.Refusal): D.verify_policy(wrong, p, selection())

    def test_no_private_finalize_before_main_thread_full_review(self):
        host = mock.Mock(); b = boundary(); c, d, p = context_and_proposal()
        reviews = D.Reviews(host, b, c, d, selection())
        errors = []
        def run():
            try: reviews.service()
            except D.Refusal: errors.append('refused')
        t = threading.Thread(target=run); t.start(); t.join()
        self.assertEqual(errors, ['refused']); host.call.assert_not_called()

    def test_main_host_refuses_unplanned_renderer_selection_without_finalizing(self):
        host = mock.Mock(); c, d, p = context_and_proposal()
        reviews = D.Reviews(host, boundary(), c, d, selection())
        changed = selection(); changed['ttl_ms'] = 60001
        item = D.PendingReview(changed, {'result': p}, 'original-key'); reviews.pending.put(item)
        with self.assertRaises(D.Refusal): reviews.service()
        self.assertTrue(item.done.is_set()); self.assertIsNone(item.reply); host.call.assert_not_called(); host.result.assert_not_called()

    def test_complete_original_response_cap_not_just_inner_snapshot(self):
        host = mock.Mock(); c, d, p = context_and_proposal()
        reviews = D.Reviews(host, boundary(), c, d, selection())
        response = {'result': p, 'warnings': ['x' * D.MAX_REVIEW]}
        item = D.PendingReview(selection(), response, 'original-key'); reviews.pending.put(item)
        self.assertEqual(item.response, response)
        with self.assertRaises(D.Refusal): reviews.service()
        host.result.assert_not_called(); host.call.assert_not_called()

    def test_pending_cancel_unblocks_without_finalizing_or_faking_reply(self):
        host = mock.Mock(); c, d, p = context_and_proposal(); reviews = D.Reviews(host, boundary(), c, d, selection())
        item = D.PendingReview(selection(), {'result': p}, 'original-key'); reviews.pending.put(item)
        reviews.stop()
        self.assertTrue(item.done.is_set()); self.assertIsNone(item.reply); host.call.assert_not_called()


class SourceAndEvidenceControls(unittest.TestCase):
    def test_generated_graph_provider_is_valid_python_and_exact_extended_contract(self):
        parsed = ast.parse(D.PROVIDER)
        fields = [n for n in ast.walk(parsed) if isinstance(n, ast.Set) and len(n.elts) == 17]
        self.assertEqual(len(fields), 1)
        names = {n.value for n in fields[0].elts}
        self.assertEqual(names, {'schema_version', 'kind', 'goal', 'node', 'run_id', 'plan_revision',
            'plan_digest', 'workspace', 'inputs', 'outputs', 'policy', 'lineage', 'budget',
            'idempotency_key', 'return_location', 'task_binding', 'validation_contracts'})
        self.assertNotIn('requests', D.PROVIDER); self.assertNotIn('http', D.PROVIDER)

    def test_plan_has_real_work_owner_no_legacy_result_acceptance_or_retry(self):
        scope = {'outputs': {'report': 'output'}, 'contracts': {'output': ['goal-owner-review']},
            'workspace_id': 'w', 'binding': {}, 'program_sha256': 'a' * 64, 'task_id': 't',
            'inputs': {'fixed-input': {'contract_id': 'input', 'sha256': 'b' * 64}},
            'goal': 'g', 'authority_revision': 2, 'required_acceptance_rules': ['goal-owner-review']}
        plan = D.plan(scope); work, owner = plan['nodes']
        self.assertEqual(work['kind'], 'work'); self.assertEqual(work['reservation']['runtime_ms'], 4000)
        self.assertEqual(owner['kind'], 'owner_approval'); self.assertEqual(owner['dependencies'], [work['id']])
        self.assertTrue(owner['required_inputs'][0]['source']['candidate_for_validation'])
        self.assertEqual(owner['executor'], {'kind': 'owner', 'rule_id': 'goal-owner-review'})
        self.assertEqual(plan['scope_revision'], 2)

    def test_bounded_pinned_image_refuses_symlink_and_preserves_all_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); image = root / 'image'; image.write_bytes(b'whole image'); image.chmod(0o500)
            record = D.pinned_file(image)
            self.assertEqual(record['sha256'], D.digest(b'whole image')); self.assertEqual(record['bytes'], 11)
            link = root / 'link'; link.symlink_to(image)
            with self.assertRaises(D.Refusal): D.pinned_file(link)
            with self.assertRaises(D.Refusal): D.pinned_file(image, 3)

    def test_public_report_is_finite_without_private_core_payloads_tokens_or_database(self):
        tree = ast.parse((ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_text())
        function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'run_case')
        assignment = next(n for n in function.body if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'report' for t in n.targets))
        keys = {n.value for n in assignment.value.keys}
        self.assertFalse(keys & {'token', 'transcript', 'proposal', 'database', 'raw_rpc', 'runtime'})
        self.assertIn('not_claimed', keys); self.assertIn('dimension', keys)
        self.assertFalse(any(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
            and n.func.attr == 'execute' and n.args and isinstance(n.args[0], ast.Constant)
            and isinstance(n.args[0].value, str) and n.args[0].value.lstrip().upper().startswith(('INSERT ', 'UPDATE ', 'DELETE '))
            for n in ast.walk(tree)))

    def test_main_requires_explicit_run_and_no_runtime_at_import(self):
        tree = ast.parse((ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_text())
        self.assertFalse(any(isinstance(n, ast.Expr) and isinstance(n.value, ast.Call)
            and isinstance(n.value.func, ast.Attribute) and n.value.func.attr in ('Popen', 'run', 'launch') for n in tree.body))
        source = ast.get_source_segment((ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_text(),
            next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'main'))
        self.assertIn('if not args.run:', source); self.assertIn("'no_process_token_or_listener': True", source)


class FurtherGuardTests(unittest.TestCase):
    def test_budget_counterexample_requires_exact_actual_typed_code_and_no_effect(self):
        response = {'error': {'code': 'RESOURCE_LIMIT', 'side_effect_state': 'not_started',
            'retryable': False, 'message': 'display'}}
        self.assertIsNone(D.require_budget_refusal(response))
        for field, bad in [('code', 'PERMISSION_DENIED'), ('code', 'INTERNAL'),
            ('side_effect_state', 'unknown'), ('side_effect_state', 'confirmed'), ('retryable', True),
            ('retryable', 0), ('message', None)]:
            value = copy.deepcopy(response); value['error'][field] = bad
            with self.subTest(field=field, bad=bad), self.assertRaises(D.Refusal): D.require_budget_refusal(value)
        with self.assertRaises(D.Refusal): D.require_budget_refusal(dict(response, result={}))

    def test_consumed_original_document_and_result_must_match_all_fields(self):
        _, _, proposal = context_and_proposal(); consumed = copy.deepcopy(proposal); consumed['consumed'] = True
        policy = {'display_only': True}
        self.assertIsNone(D.verify_consumed(D.compact(consumed), D.compact(policy), proposal, policy))
        for key, wrong in [('consumed', False), ('revision', 2), ('id', 'foreign')]:
            value = copy.deepcopy(consumed); value[key] = wrong
            with self.subTest(key=key), self.assertRaises(D.Refusal): D.verify_consumed(D.compact(value), D.compact(policy), proposal, policy)
        value = copy.deepcopy(consumed); value['snapshot']['intent']['max_rounds'] = 2
        with self.assertRaises(D.Refusal): D.verify_consumed(D.compact(value), D.compact(policy), proposal, policy)
        with self.assertRaises(D.Refusal): D.verify_consumed(D.compact(consumed), b'{"display_only":false}', proposal, policy)

    def test_boolean_revision_and_extra_policy_fields_never_match(self):
        _, _, p = context_and_proposal()
        policy = {'id': p['id'], 'approval_id': p['id'], 'revision': True, 'revoked': False,
            'selection': selection(), 'bindings': p['snapshot']['bindings'], 'created_at': 'display',
            'snapshot_digest': D.digest(D.compact(p['snapshot'])), 'expires_at_ms': 8000}
        with self.assertRaises(D.Refusal): D.verify_policy(policy, p, selection())
        policy['revision'] = 1; policy['approved'] = True
        with self.assertRaises(D.Refusal): D.verify_policy(policy, p, selection())

    def test_invalid_loopback_port_is_rejected_even_if_headers_match(self):
        origin = 'http://127.0.0.1:65536'
        with self.assertRaises(D.Refusal): D.parse_http(D.compact(envelope()),
            {'Origin': origin, 'Host': origin[7:], 'Content-Type': 'application/json'}, origin, boundary())

    def test_host_frame_keeps_token_private_and_preserves_exact_key_cas(self):
        # Synthetic socket transport only, no actual token file/socket/server.
        host = D.Host.__new__(D.Host); host.runtime = Path('/display-only'); host.token = 'PRIVATE_CANARY'
        host.audit, host.lock = [], threading.Lock()
        with mock.patch.object(D.socket, 'socket') as factory:
            stream = factory.return_value.__enter__.return_value
            stream.makefile.return_value.__enter__.return_value.readline.return_value = b'{"result":{"display_only":true}}\n'
            reply = host.call('integrations.cancel', {'integration_id': 'r'}, 'original', 9)
            frame = D.decode(stream.sendall.call_args.args[0])
            self.assertEqual(frame['token'], 'PRIVATE_CANARY'); self.assertEqual(frame['expected_revision'], 9)
            self.assertEqual(frame['idempotency_key'], 'original')
            self.assertNotIn('PRIVATE_CANARY', json.dumps(reply)); self.assertNotIn('PRIVATE_CANARY', json.dumps(host.audit))
        with mock.patch.object(D.socket, 'socket') as factory:
            with self.assertRaises(D.Refusal): host.call('integrations.cancel', {'x': 'z' * 262144}, 'k', 1)
            factory.assert_not_called()

    def test_preflight_requires_exact_paired_source_schema_hashes_and_frontend_build(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); head = 'a' * 40; h = 'b' * 64
            args = SimpleNamespace(expected_source_head=head, batond=root/'batond', frontend=root/'frontend',
                candidate_manifest=root/'manifest.json', frontend_build_record=root/'build.json', chromium=root/'chromium')
            manifest = {'schema': 17, 'source_head': head, 'compiled_core_source_head': head,
                'dirty': False, 'binaries': {'batond': h, 'baton': h}}
            build = {'source': head, 'exit': 0, 'command': ['npm', '--prefix', 'apps/desktop', 'run', 'build'],
                'source_head_and_tracked_unchanged': True}
            args.frontend.mkdir(); args.chromium.write_bytes(b'display image')
            record = {'sha256': h, 'bytes': 10, 'mode': 0o500, 'device': '1', 'inode': '2', 'uid': 0, 'links': 1}
            def check(m, b):
                args.candidate_manifest.write_text(json.dumps(m)); args.frontend_build_record.write_text(json.dumps(b))
                with mock.patch.object(D, 'pinned_file', return_value=record), mock.patch.object(D, 'assets', return_value={'index.html': record}), \
                    mock.patch.object(D.os, 'access', return_value=True), mock.patch.object(D, 'source_guard', return_value={'head': head}):
                    return D.preflight(args)
            self.assertEqual(check(manifest, build)[3]['source'], head)
            for wrong in [16, 18, True, 17.0, '17']:
                m = dict(manifest, schema=wrong)
                with self.subTest(schema=wrong), self.assertRaises(D.Refusal): check(m, build)
            for m in [dict(manifest, compiled_core_source_head='c'*40), dict(manifest, dirty=True),
                dict(manifest, binaries={'batond': 'c'*64, 'baton': h})]:
                with self.assertRaises(D.Refusal): check(m, build)
            for b in [dict(build, source='c'*40), dict(build, exit=1), dict(build, exit=False),
                dict(build, dirty=True), dict(build, source_head_and_tracked_unchanged=False),
                dict(build, command=['npm', 'run', 'build'])]:
                with self.assertRaises(D.Refusal): check(manifest, b)

    def test_source_guard_requires_actual_tracked_driver_and_clean_exact_commit(self):
        raw = (ROOT / 'apps/desktop/tests/integration-real-ipc.py').read_bytes(); head = 'a' * 40
        with mock.patch.object(D.subprocess, 'check_output', side_effect=[head+'\n', b'', raw]):
            self.assertEqual(D.source_guard(head)['driver_sha256'], D.digest(raw))
        for values in [[head+'\n', b'M driver', raw], ['b'*40+'\n', b'', raw], [head+'\n', b'', b'changed']]:
            with mock.patch.object(D.subprocess, 'check_output', side_effect=values), self.assertRaises(D.Refusal): D.source_guard(head)

    def test_actual_run_is_not_implemented_with_legacy_goal_bootstrap_or_sql_writes(self):
        source = (ROOT/'apps/desktop/tests/integration-real-ipc.py').read_text()
        self.assertNotIn("'goal_runs.propose'", source); self.assertNotIn('load_graph_helpers', source)
        self.assertIn("'goal_graph_runs.propose'", source)
        self.assertIn('verify_consumed(document, stored, item.proposal', source)
        self.assertIn("'forced_kill': False", source)
        self.assertIn('thread.join(5); require(not thread.is_alive())', source)
        self.assertIn("cleanup('tools_assets_unchanged'", source)
        self.assertIn("'state': 'after_owned_cleanup_attempts'", source)



if __name__ == '__main__':
    unittest.main()
