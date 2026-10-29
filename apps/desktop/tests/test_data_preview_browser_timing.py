#!/usr/bin/env python3
"""Pure driver transport/observation regression; no browser, daemon or business fixture.

Executes the actual extracted fixed HTTP handler with in-memory request/reply
streams. Stub results are transport sentinels, never product acceptance evidence.
"""
import ast
from concurrent.futures import ThreadPoolExecutor
from email.message import Message
import http.server
import io
import json
from pathlib import Path
import socket
import threading
import time
import types
import unittest

SOURCE = Path(__file__).with_name('data-workspace-real-ipc.py').read_text()
TREE = ast.parse(SOURCE)


class TickClock:
    def __init__(self):
        self.calls = 0

    def monotonic_ns(self):
        self.calls += 1
        return self.calls * 1000


class DriverTimingTests(unittest.TestCase):
    def make_handler(self, call, clock=None):
        report = {'requests': [], 'faults': [], 'observations': {}}
        scope = dict(http=http, json=json, socket=socket, time=clock or time,
                     threading=threading, re=__import__('re'), ipc=call,
                     url='http://127.0.0.1:9876', report=report,
                     readonly={'system_snapshot': ('system.snapshot', {})}, lists={},
                     preview_ids={'preview-fixture': 'project-fixture'}, creation_ids={},
                     fault_lock=threading.Lock(), drop_cancel={'target': None},
                     remember=lambda *args: None, database=lambda: {'transport_probe_only': True}, safe=str)
        helpers = [n for n in TREE.body if isinstance(n, ast.FunctionDef) and n.name in {'unique_object', 'integer'}]
        handler = next(n for n in ast.walk(TREE) if isinstance(n, ast.ClassDef) and n.name == 'Handler')
        exec(compile(ast.Module(body=[*helpers, handler], type_ignores=[]), '<actual-fixed-handler>', 'exec'), scope)
        return scope, report

    def request(self, scope, body=None, origin=None):
        obj = object.__new__(scope['Handler'])
        raw = body if isinstance(body, bytes) else json.dumps(body or {'command': 'system_snapshot', 'args': {}}).encode()
        obj.path = '/__test_data_workspace_ipc'
        obj.headers = Message()
        for key, value in {'Origin': origin or scope['url'], 'Host': '127.0.0.1:9876',
                           'Content-Type': 'application/json', 'Content-Length': str(len(raw))}.items():
            obj.headers[key] = value
        obj.rfile, obj.wfile = io.BytesIO(raw), io.BytesIO()
        obj.statuses = []
        obj.send_error = lambda status, *args: obj.statuses.append(status)
        obj.send_response = lambda status: obj.statuses.append(status)
        obj.send_header = lambda *args: None
        obj.end_headers = lambda: None
        obj.connection = types.SimpleNamespace(shutdown=lambda _: None, close=lambda: None)
        return obj

    def test_handler_observes_call_boundaries_without_replacing_response(self):
        clock = TickClock()
        sentinel = {'transport_sentinel': ['not a product DTO']}
        seen = []

        def call(*args):
            seen.append((args, clock.calls))
            return sentinel

        scope, report = self.make_handler(call, clock)
        req = self.request(scope)
        req.do_POST()
        self.assertEqual(seen, [(('system.snapshot', {}, None, None), 2)])
        self.assertEqual(json.loads(req.wfile.getvalue()), sentinel)
        self.assertIs(report['requests'][0]['response'], sentinel)
        self.assertEqual(report['requests'][0]['timing'], {'http_received_ns': 1000, 'pre_ipc_ns': 2000, 'post_ipc_ns': 3000})
        self.assertEqual(req.statuses, [200])

    def test_no_cross_request_timing_or_response_association_under_threads(self):
        lock = threading.Lock()
        count = 0

        def call(*args):
            nonlocal count
            with lock:
                count += 1
                return {'transport_probe_index': count}

        scope, report = self.make_handler(call)
        with ThreadPoolExecutor(max_workers=4) as workers:
            requests = list(workers.map(lambda _: self.request(scope), range(32)))
            list(workers.map(lambda req: req.do_POST(), requests))
        self.assertEqual(count, 32)
        self.assertEqual(len(report['requests']), 32)
        self.assertEqual({r['response']['transport_probe_index'] for r in report['requests']}, set(range(1, 33)))
        self.assertEqual({json.loads(r.wfile.getvalue())['transport_probe_index'] for r in requests}, set(range(1, 33)))
        for row in report['requests']:
            t = row['timing']
            self.assertLessEqual(t['http_received_ns'], t['pre_ipc_ns'])
            self.assertLessEqual(t['pre_ipc_ns'], t['post_ipc_ns'])

    def test_refused_origin_never_dispatches_or_invents_call_timing(self):
        scope, report = self.make_handler(lambda *a: self.fail('refused origin dispatched'))
        req = self.request(scope, origin='http://foreign.invalid')
        req.do_POST()
        self.assertEqual(req.statuses, [403])
        self.assertEqual(report['requests'], [])

    def test_duplicate_or_forbidden_fields_never_dispatch(self):
        for raw in [b'{"command":"system_snapshot","command":"other"}',
                    b'{"command":"approvals_finalize","args":{}}',
                    b'{"command":"system_snapshot","args":{},"approved":true}']:
            scope, report = self.make_handler(lambda *a: self.fail('invalid request dispatched'))
            req = self.request(scope, raw)
            req.do_POST()
            self.assertIn(req.statuses[0], [400, 403])
            self.assertEqual(report['requests'], [])

    def test_failed_ipc_retains_boundary_observation_without_success_record(self):
        def fail(*args):
            raise OSError('transport-only failure')

        scope, report = self.make_handler(fail, TickClock())
        req = self.request(scope)
        req.do_POST()
        self.assertEqual(req.statuses, [502])
        self.assertEqual(report['requests'], [])
        self.assertEqual(report['observations']['adapter_failure_timings'], [{'method': 'system.snapshot', 'timing': {'http_received_ns': 1000, 'pre_ipc_ns': 2000, 'post_ipc_ns': 3000}}])

    def assert_connection_error_boundaries(self, error):
        calls = []

        def fail(*args):
            calls.append(args)
            raise error('IPC transport deliberately interrupted')

        clock = TickClock()
        scope, report = self.make_handler(fail, clock)
        req = self.request(scope)
        req.do_POST()
        self.assertEqual(len(calls), 1)
        self.assertEqual(clock.calls, 3)
        self.assertEqual(req.statuses, [])  # Preserve the original no-response failure.
        self.assertEqual(req.wfile.getvalue(), b'')
        self.assertEqual(report['requests'], [])
        self.assertEqual(report['observations'].get('client_aborts', 0), 0)
        self.assertEqual(report['observations']['adapter_failure_timings'], [{'method': 'system.snapshot', 'timing': {'http_received_ns': 1000, 'pre_ipc_ns': 2000, 'post_ipc_ns': 3000}, 'phase': 'ipc'}])
        self.assertIn('deliberately interrupted', report['observations']['adapter_error'])

        # The same exception from response writing is a genuine HTTP abort,
        # after a real return from the inert transport callback, not IPC failure.
        sentinel = {'transport_sentinel': 'not a product DTO'}
        scope, report = self.make_handler(lambda *args: sentinel, TickClock())
        req = self.request(scope)

        class InterruptedWriter:
            def write(self, raw):
                raise error('HTTP writer deliberately interrupted')

        req.wfile = InterruptedWriter()
        req.do_POST()
        self.assertEqual(req.statuses, [200])
        self.assertEqual(report['observations']['client_aborts'], 1)
        self.assertNotIn('adapter_failure_timings', report['observations'])
        self.assertIs(report['requests'][0]['response'], sentinel)
        self.assertEqual(report['requests'][0]['timing'], {'http_received_ns': 1000, 'pre_ipc_ns': 2000, 'post_ipc_ns': 3000})

    def test_ipc_broken_pipe_is_not_an_http_client_abort(self):
        self.assert_connection_error_boundaries(BrokenPipeError)

    def test_ipc_connection_reset_is_not_an_http_client_abort(self):
        self.assert_connection_error_boundaries(ConnectionResetError)

    def test_dropped_verified_response_is_never_rewritten_to_cancelled(self):
        sentinel = {'result': {'id': 'preview-fixture', 'state': 'verified', 'revision': 2}}
        scope, report = self.make_handler(lambda *a: sentinel, TickClock())
        scope['drop_cancel']['target'] = 'preview-fixture'
        req = self.request(scope, {'command': 'data_workspace_previews_cancel', 'args': {'previewId': 'preview-fixture', 'expectedRevision': 1, 'idempotencyKey': 'original-key'}})
        req.do_POST()
        self.assertIs(report['faults'][0]['actual_response'], sentinel)
        self.assertEqual(report['requests'][0]['response']['result']['state'], 'verified')
        self.assertEqual(req.wfile.getvalue(), b'')
        self.assertIsNone(scope['drop_cancel']['target'])

    def click_nodes(self):
        for node in ast.walk(TREE):
            if isinstance(node, ast.If):
                for i, statement in enumerate(node.body):
                    if isinstance(statement, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'click_timing' for t in statement.targets):
                        return node.body[i:i + 3]
        self.fail('Actual click observation not found')

    def test_click_boundaries_survive_actionability_failure_without_retry(self):
        calls = []

        def click():
            calls.append('actual click call')
            raise RuntimeError('transport-only actionability timeout')

        report = {'observations': {}}
        scope = dict(time=TickClock(), report=report, cancel_button=types.SimpleNamespace(click=click))
        with self.assertRaisesRegex(RuntimeError, 'actionability'):
            exec(compile(ast.Module(body=self.click_nodes(), type_ignores=[]), '<actual-click-observation>', 'exec'), scope)
        self.assertEqual(calls, ['actual click call'])
        self.assertEqual(report['observations']['preview_cancel_ui_click'], {'before_click_call_ns': 1000, 'after_click_call_ns': 2000})

    def test_preview_branch_has_no_extra_refresh_before_exact_button_click(self):
        segment = SOURCE.split("started, _ = start_preview(panel, 'preview-cancel')", 1)[1].split("terminal = wait(lambda: preview_record", 1)[0]
        before_click = segment.split('cancel_button.click()', 1)[0]
        self.assertNotIn('refresh(panel)', before_click)
        self.assertIn("name='Request preview cancellation', exact=True", before_click)
        self.assertIn("drop_cancel['target'] = started['id']", before_click)
        self.assertIn("panel = open_project('preview-cancel')", segment)  # Actual remount retained.

    def test_verified_is_still_incomplete_not_cancellation_evidence(self):
        self.assertIn("elif terminal['state'] != 'cancelled':", SOURCE)
        self.assertIn('Preview request was exercised but its actual terminal was verified; no cooperative cancellation hit is claimed', SOURCE)
        self.assertIn("report['status'] = 'passed' if not report['unrun'] else 'incomplete_race_coverage'", SOURCE)
        self.assertIn("return 0 if report['status'] == 'passed' else 2", SOURCE)


if __name__ == '__main__':
    unittest.main()
