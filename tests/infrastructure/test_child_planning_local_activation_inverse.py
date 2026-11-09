"""Exact whole-driver composition inverse; unknown/duplicate/moved edits refuse.

Pins bind the immutable db281 driver and this finite observation-only candidate.
No runtime, process, SQL, browser, network, or Git call is made by these tests.
"""
import ast
import hashlib
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / 'apps/desktop/tests/factory-child-planning-real-ipc.py'
ORIGINAL_SHA256 = '174931beb6999e4120e4e6419cd3485db373f8f5383bfd72415e0982e8e39c02'
CANDIDATE_SHA256 = '325b013eefa3978107016ecb644290316cb820aa7d4a0d46286ca3c364cf85a3'
FRAGMENTS = {'block': '# BEGIN LOCAL REFUSAL ACTIVATION OBSERVATION\n# Private harness observation only. Neither product state nor dispatch is changed.\nLOCAL_ACTIVATION_INSTALL = r"""button => {\n const root=button.closest(\'section\');\n if(!root||window.__childPlanActivation)throw new Error(\'Activation observer already installed or missing region\');\n const state={button,root,down:0,up:0,clicks:0};\n state.listener=e=>{if(e.target===button){if(e.type===\'pointerdown\')state.down++;if(e.type===\'pointerup\')state.up++;if(e.type===\'click\')state.clicks++;}};\n for(const type of [\'pointerdown\',\'pointerup\',\'click\'])document.addEventListener(type,state.listener,true);\n window.__childPlanActivation=state;\n}"""\nLOCAL_ACTIVATION_READ = r"""() => {\n const s=window.__childPlanActivation,b=window.__childPlanSubmitObservation;\n if(!s||!b)throw new Error(\'Missing local activation observation\');\n const alerts=Array.from(s.root.querySelectorAll(\':scope > p[role="alert"]\'),e=>e.textContent||\'\');\n return {down:s.down,up:s.up,clicks:s.clicks,connected:s.button.isConnected,\n  starts:b.starts,active:b.active,\n  denied:alerts.some(t=>/^Plan submission refused(?::|\\s|$)/.test(t)),\n  not_sent:alerts.some(t=>t.startsWith(\'No submission was sent.\')),\n  unknown:alerts.some(t=>t.startsWith(\'Submission outcome unknown.\'))};\n}"""\nLOCAL_ACTIVATION_REMOVE = r"""() => {\n const s=window.__childPlanActivation;if(!s)return;\n for(const type of [\'pointerdown\',\'pointerup\',\'click\'])document.removeEventListener(type,s.listener,true);\n delete window.__childPlanActivation;\n}"""\n\n\ndef local_activation_decision(snapshot, starts):\n    """Closed finite observations: absence of an HTTP row is never evidence."""\n    assert type(snapshot) is dict and set(snapshot) == {\n        \'down\',\'up\',\'clicks\',\'connected\',\'starts\',\'active\',\'denied\',\'not_sent\',\'unknown\'}\n    assert all(type(snapshot[k]) is int and 0 <= snapshot[k] <= 1 for k in (\'down\',\'up\',\'clicks\'))\n    assert all(type(snapshot[k]) is bool for k in (\'connected\',\'denied\',\'not_sent\',\'unknown\'))\n    assert all(type(snapshot[k]) is int and snapshot[k] >= 0 for k in (\'starts\',\'active\'))\n    assert type(starts) is int and starts >= 0\n    assert snapshot[\'starts\'] == starts and snapshot[\'active\'] == 0, \'A submit started; never replay local activation\'\n    assert snapshot[\'connected\'] and not snapshot[\'unknown\'], \'Disconnected or unknown activation cannot be replayed\'\n    if snapshot[\'denied\']:\n        return \'denied\'\n    if snapshot[\'clicks\'] == 1:\n        # Click capture alone is not proof that the React handler ran. Only its\n        # explicit no-send publication may permit another genuine activation.\n        return \'retry\' if snapshot[\'not_sent\'] else \'wait\'\n    if snapshot[\'down\'] == 1 and snapshot[\'up\'] == 1:\n        return \'retry\'  # Exact native press/release, but no click was delivered.\n    return \'wait\'\n\n\ndef observe_local_refusal(page, button, submit_count, *, clock=time.monotonic):\n    """At most four local activations within the original five-second bound.\n\n    This is for the fixed invalid local drafts only, never for recovery or any\n    successful/uncertain mutation. Invoke-start is recorded synchronously before\n    the unchanged fetch, so even a queued/failed request permanently stops retries.\n    """\n    deadline = clock() + 5\n    baseline = page.evaluate(\'() => window.__childPlanSubmitObservation.starts\')\n    initial_http_count = submit_count()\n    for attempt in range(1, 5):\n        assert clock() < deadline, \'Local refusal activation deadline expired\'\n        button.evaluate(LOCAL_ACTIVATION_INSTALL)\n        try:\n            before = page.evaluate(LOCAL_ACTIVATION_READ)\n            assert before[\'starts\'] == baseline and before[\'active\'] == 0 and not before[\'unknown\']\n            assert not before[\'denied\'], \'A previous denial is not this activation receipt\'\n            # Any Playwright error is the original failure, never a retry signal.\n            remaining = (deadline - clock()) * 1000\n            assert remaining > 0, \'Local refusal activation deadline expired\'\n            button.click(timeout=remaining)\n            while True:\n                assert clock() < deadline, \'Local refusal activation deadline expired\'\n                snapshot = page.evaluate(LOCAL_ACTIVATION_READ)\n                decision = local_activation_decision(snapshot, baseline)\n                assert submit_count() == initial_http_count, \'A submit reached HTTP; never replay activation\'\n                if decision == \'denied\':\n                    return deadline, {\'attempts\':attempt,\'outcome\':\'local_denied\',\'submit_starts_delta\':0}\n                if decision == \'retry\':\n                    break\n                page.wait_for_timeout(min(10, max(0, (deadline - clock()) * 1000)))\n        finally:\n            page.evaluate(LOCAL_ACTIVATION_REMOVE)\n    raise AssertionError(\'Local refusal activation attempts exhausted; no mutation was replayed\')\n# END LOCAL REFUSAL ACTIVATION OBSERVATION\n\n\n', 'old_bridge': 'context.add_init_script("""localStorage.setItem(\'agentbaton.language\',\'en\');window.__TAURI_INTERNALS__={invoke:async(command,args)=>{if(command===\'factory_scope_approve\')return window.__private_test_factory_owner(command,args);const r=await fetch(\'/__test_child_plan_ipc\',{method:\'POST\',headers:{\'Content-Type\':\'application/json\'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error(\'Fixed child-plan test bridge refused request\');return r.json();}};""")', 'new_bridge': 'context.add_init_script("""localStorage.setItem(\'agentbaton.language\',\'en\');window.__childPlanSubmitObservation={starts:0,active:0};window.__TAURI_INTERNALS__={invoke:async(command,args)=>{if(command===\'factory_scope_approve\')return window.__private_test_factory_owner(command,args);const observed=command===\'factory_child_plans_submit\';if(observed){window.__childPlanSubmitObservation.starts++;window.__childPlanSubmitObservation.active++;}try{const r=await fetch(\'/__test_child_plan_ipc\',{method:\'POST\',headers:{\'Content-Type\':\'application/json\'},body:JSON.stringify({command,args})});if(!r.ok)throw new Error(\'Fixed child-plan test bridge refused request\');return await r.json();}finally{if(observed)window.__childPlanSubmitObservation.active--;}}};""")', 'old_call': '            root_region.get_by_role(\'button\', name=\'Store next plan revision\', exact=True).click()\n            expect(root_region.locator(\':scope > p[role="alert"]\').filter(has_text=re.compile(r\'^Plan submission refused(?::|\\s|$)\'))).to_contain_text(\'Plan submission refused\')\n', 'new_call': '            activation_deadline, activation = observe_local_refusal(page,\n                root_region.get_by_role(\'button\', name=\'Store next plan revision\', exact=True),\n                lambda: sum(r[\'method\'] == \'factory_child_plans.submit\' for r in report[\'http\']))\n            report[\'observations\'].setdefault(\'local_negative_activations\', []).append({\'variant\':label, **activation})\n            activation_remaining = (activation_deadline - time.monotonic()) * 1000\n            assert activation_remaining > 0, \'Local refusal activation deadline expired\'\n            expect(root_region.locator(\':scope > p[role="alert"]\').filter(has_text=re.compile(r\'^Plan submission refused(?::|\\s|$)\'))).to_contain_text(\'Plan submission refused\', timeout=activation_remaining)\n'}

def sha(value): return hashlib.sha256(value).hexdigest()


def original_driver(candidate):
    from tests.infrastructure.schema18_source_fixture import strip_schema18_source
    assert type(candidate) is bytes, 'Unknown driver source'
    candidate=strip_schema18_source('apps/desktop/tests/factory-child-planning-real-ipc.py',candidate.decode()).encode()
    assert type(candidate) is bytes and sha(candidate) == CANDIDATE_SHA256, 'Unknown driver source'
    source = candidate.decode()
    for new, old in [(FRAGMENTS['block'], ''),
                     (FRAGMENTS['new_bridge'], FRAGMENTS['old_bridge']),
                     (FRAGMENTS['new_call'], FRAGMENTS['old_call'])]:
        assert source.count(new) == 1, 'Unknown, duplicated, or missing composition fragment'
        source = source.replace(new, old, 1)
    result = source.encode()
    assert sha(result) == ORIGINAL_SHA256, 'Whole original driver not restored'
    return result


class ExactDriverComposition(unittest.TestCase):
    def test_whole_inverse_restores_exact_db281_driver(self):
        self.assertEqual(sha(original_driver(DRIVER.read_bytes())), ORIGINAL_SHA256)

    def test_all_original_function_bodies_restore_and_new_helpers_are_only_additions(self):
        raw = DRIVER.read_bytes(); before = ast.parse(original_driver(raw)); after = ast.parse(raw)
        old_names = {n.name for n in before.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        new_names = {n.name for n in after.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        self.assertEqual(new_names - old_names, {'local_activation_decision', 'observe_local_refusal'})
        self.assertTrue(old_names <= new_names)

    def test_old_negative_backend_and_db_assertions_are_still_literal_and_ordered(self):
        source = DRIVER.read_text()
        expected = [
            "assert sum(r['method'] == 'factory_child_plans.submit' for r in report['http']) == n",
            "assert reply['status'] == 200 and reply['value']['error']['code'] == code, reply",
            "assert reply['value']['error']['side_effect_state'] == 'not_started'",
            "assert fingerprint_db(runtime) == before",
        ]
        start = source.index('for label, invalid, code in variants:')
        positions = [source.index(value, start) for value in expected]
        self.assertEqual(positions, sorted(positions))
        self.assertIn("to_contain_text('Plan submission refused', timeout=activation_remaining)", source)
        self.assertIn('deadline = clock() + 5', source)
        self.assertIn('for attempt in range(1, 5):', source)

    def test_unknown_outside_fragment_cannot_be_hidden_by_inverse(self):
        raw = DRIVER.read_bytes()
        for changed in [raw + b'\n# unknown edit\n', raw.replace(b'PLAN_BYTES = 262_144', b'PLAN_BYTES = 262_145')]:
            with self.assertRaisesRegex(AssertionError, 'Unknown driver source'): original_driver(changed)

    def test_duplicated_or_moved_known_block_is_rejected(self):
        raw = DRIVER.read_bytes(); block = FRAGMENTS['block'].encode()
        for changed in [block + raw, raw.replace(block, b'') + block]:
            with self.assertRaisesRegex(AssertionError, 'Unknown driver source'): original_driver(changed)

    def test_old_bridge_and_full_negative_loop_recovered(self):
        source = original_driver(DRIVER.read_bytes()).decode()
        self.assertEqual(source.count(FRAGMENTS['old_bridge']), 1)
        self.assertEqual(source.count(FRAGMENTS['old_call']), 1)
        self.assertNotIn('__childPlanSubmitObservation', source)
        self.assertNotIn('__childPlanActivation', source)


if __name__ == '__main__': unittest.main()
