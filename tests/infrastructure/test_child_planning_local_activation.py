"""Finite harness decision controls; no actual browser/Core/process execution.

actual_browser_refresh_press_release(page) is a separate real native DOM recipe
for the main agent's controlled Chromium run. unittest never calls it.
"""
import ast
from pathlib import Path
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / 'apps/desktop/tests/factory-child-planning-real-ipc.py'
NAMES = {'LOCAL_ACTIVATION_INSTALL', 'LOCAL_ACTIVATION_READ', 'LOCAL_ACTIVATION_REMOVE',
         'local_activation_decision', 'observe_local_refusal'}


def driver_namespace():
    tree = ast.parse(DRIVER.read_text())
    nodes = [n for n in tree.body if (isinstance(n, ast.FunctionDef) and n.name in NAMES)
             or (isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id in NAMES for t in n.targets))]
    assert len(nodes) == 5
    namespace = {'time': time}
    exec(compile(ast.Module(body=nodes, type_ignores=[]), str(DRIVER), 'exec'), namespace)
    return namespace


D = driver_namespace()


def state(**changes):
    return dict(down=0, up=0, clicks=0, connected=True, starts=0, active=0,
                denied=False, not_sent=False, unknown=False, **changes)


class Page:
    """Explicit finite native-event outcomes, not a Playwright implementation."""
    def __init__(self, modes):
        self.now = 0.; self.modes = modes; self.snapshot = state(); self.calls = 0
        self.installed = False; self.removes = 0; self.http = 0; self.timeouts = []
        self.next_snapshot = None

    def evaluate(self, script):
        if script == '() => window.__childPlanSubmitObservation.starts':
            return self.snapshot['starts']
        if script == D['LOCAL_ACTIVATION_REMOVE']:
            self.installed = False; self.removes += 1; return
        assert script == D['LOCAL_ACTIVATION_READ'] and self.installed
        result = self.snapshot.copy()
        if self.next_snapshot is not None:
            self.snapshot = self.next_snapshot; self.next_snapshot = None
        return result

    def wait_for_timeout(self, milliseconds):
        assert 0 <= milliseconds <= 10
        self.now += milliseconds / 1000


class Button:
    def __init__(self, page): self.page = page

    def evaluate(self, script):
        p = self.page
        assert script == D['LOCAL_ACTIVATION_INSTALL'] and not p.installed
        p.installed = True
        p.snapshot.update(down=0, up=0, clicks=0)

    def click(self, timeout):
        p = self.page; assert p.installed and 0 < timeout <= 5000
        p.calls += 1; p.timeouts.append(timeout); p.now += .01
        mode = p.modes[min(p.calls - 1, len(p.modes) - 1)]
        p.snapshot.update(down=1, up=1, clicks=1, denied=False, not_sent=False)
        if mode == 'lost_during_refresh': p.snapshot['clicks'] = 0
        elif mode == 'denied': p.snapshot['denied'] = True
        elif mode == 'not_sent': p.snapshot['not_sent'] = True
        elif mode == 'delivered_no_notice': pass
        elif mode == 'delayed_start': p.next_snapshot = {**p.snapshot, 'starts': 1, 'active': 1}
        elif mode == 'submit_started': p.snapshot.update(starts=1, active=1)
        elif mode == 'submit_finished': p.snapshot.update(starts=1, active=0)
        elif mode == 'unknown': p.snapshot['unknown'] = True
        elif mode == 'detached': p.snapshot['connected'] = False
        elif mode == 'http_started': p.http += 1
        elif mode == 'click_error': raise ValueError('original click failure')
        elif mode == 'release_unobserved': p.snapshot.update(clicks=0, up=0)
        else: raise AssertionError('Unknown synthetic event outcome')


def run(modes):
    page = Page(modes)
    value = D['observe_local_refusal'](page, Button(page), lambda: page.http, clock=lambda: page.now)
    return page, value


class LocalActivationControls(unittest.TestCase):
    def test_completed_local_denial_is_observed_once(self):
        p, (deadline, value) = run(['denied'])
        self.assertEqual(value, {'attempts':1, 'outcome':'local_denied', 'submit_starts_delta':0})
        self.assertEqual(deadline, 5); self.assertEqual(p.removes, 1)

    def test_refresh_press_release_without_click_retries_only_local_activation(self):
        p, (_, value) = run(['lost_during_refresh', 'denied'])
        self.assertEqual(value['attempts'], 2); self.assertEqual(p.http, 0)
        self.assertEqual(p.removes, 2); self.assertFalse(p.installed)

    def test_explicit_react_not_sent_allows_one_later_fresh_activation(self):
        p, (_, value) = run(['not_sent', 'denied'])
        self.assertEqual(value['attempts'], 2); self.assertEqual(p.http, 0)

    def test_delivered_click_without_no_send_witness_is_never_replayed(self):
        p = Page(['delivered_no_notice', 'denied'])
        with self.assertRaisesRegex(AssertionError, 'deadline expired'):
            D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
        self.assertEqual(p.calls, 1); self.assertLessEqual(p.now, 5.000001)
        self.assertEqual(p.removes, 1); self.assertFalse(p.installed)

    def test_unobserved_release_is_not_treated_as_no_click(self):
        p = Page(['release_unobserved', 'denied'])
        with self.assertRaisesRegex(AssertionError, 'deadline expired'):
            D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
        self.assertEqual(p.calls, 1)

    def test_bridge_start_active_completed_or_async_gap_never_replays(self):
        for mode in ['submit_started', 'submit_finished', 'delayed_start']:
            with self.subTest(mode=mode):
                p = Page([mode, 'denied'])
                with self.assertRaisesRegex(AssertionError, 'submit started'):
                    D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
                self.assertEqual(p.calls, 1); self.assertEqual(p.removes, 1)

    def test_unknown_detached_and_server_observation_fail_closed(self):
        for mode in ['unknown', 'detached', 'http_started']:
            with self.subTest(mode=mode):
                p = Page([mode, 'denied'])
                with self.assertRaises(AssertionError):
                    D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
                self.assertEqual(p.calls, 1); self.assertFalse(p.installed)

    def test_original_click_exception_is_not_swallowed(self):
        p = Page(['click_error', 'denied'])
        with self.assertRaisesRegex(ValueError, '^original click failure$'):
            D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
        self.assertEqual(p.calls, 1); self.assertEqual(p.removes, 1)

    def test_retry_count_is_finite_and_no_deadline_reset(self):
        p = Page(['lost_during_refresh'])
        with self.assertRaisesRegex(AssertionError, 'attempts exhausted'):
            D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
        self.assertEqual(p.calls, 4); self.assertEqual(p.removes, 4)
        self.assertTrue(all(t <= 5000 for t in p.timeouts))
        self.assertTrue(all(b < a for a,b in zip(p.timeouts,p.timeouts[1:])))

    def test_existing_unknown_or_denial_is_not_a_new_activation_receipt(self):
        for name in ['unknown', 'denied']:
            with self.subTest(name=name):
                p = Page(['denied']); p.snapshot[name] = True
                with self.assertRaises(AssertionError):
                    D['observe_local_refusal'](p, Button(p), lambda:p.http, clock=lambda:p.now)
                self.assertEqual(p.calls, 0); self.assertEqual(p.removes, 1)

    def test_closed_snapshot_rejects_extra_fields_types_and_nonunit_event_counts(self):
        for s in [{**state(), 'extra':0}, {**state(), 'clicks':True},
                  {**state(), 'up':2}, {**state(), 'unknown':'false'}]:
            with self.subTest(keys=list(s)):
                with self.assertRaises(AssertionError): D['local_activation_decision'](s, 0)

    def test_zero_http_rows_or_old_no_send_copy_alone_cannot_authorize_retry(self):
        self.assertEqual(D['local_activation_decision'](state(), 0), 'wait')
        s = state(); s['not_sent'] = True
        self.assertEqual(D['local_activation_decision'](s, 0), 'wait')

    def test_started_submit_blocks_even_denial_or_no_click_witness(self):
        for s in [dict(state(), denied=True, starts=1), dict(state(), down=1, up=1, starts=1)]:
            with self.assertRaisesRegex(AssertionError, 'submit started'):
                D['local_activation_decision'](s, 0)


def actual_browser_refresh_press_release(page):
    """True native DOM test to run separately; no network, product or Core claim.

    First pointerdown withdraws readiness before release. The second real click
    publishes the fixed local-denied alert. Passive listeners and original five
    seconds are the same driver code, not synthetic click/evaluate dispatch.
    """
    page.set_content('''<section><button>Store next plan revision</button></section>
    <script>window.__childPlanSubmitObservation={starts:0,active:0};
    const b=document.querySelector('button');let refresh=true;
    b.addEventListener('pointerdown',()=>{if(refresh){refresh=false;b.disabled=true;setTimeout(()=>{b.disabled=false},50)}});
    b.addEventListener('click',()=>{const p=document.createElement('p');p.setAttribute('role','alert');p.textContent='Plan submission refused: controlled local input';b.parentElement.append(p)});
    </script>''')
    deadline, value = D['observe_local_refusal'](page,
        page.get_by_role('button', name='Store next plan revision', exact=True), lambda:0)
    assert time.monotonic() < deadline and value['attempts'] == 2
    assert page.evaluate('() => window.__childPlanActivation === undefined')
    assert page.evaluate('() => window.__childPlanSubmitObservation.starts') == 0
    return {'dimension':'controlled real Chromium native DOM; no actual Core or product IPC',
            'refresh_between_press_release':True,'genuine_activations':2,'submit_starts':0}


if __name__ == '__main__': unittest.main()
