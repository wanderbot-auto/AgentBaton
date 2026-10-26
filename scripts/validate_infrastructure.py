#!/usr/bin/env python3
"""Static infrastructure checks; never proof of product correctness or deployed policy."""
from pathlib import Path
import json, re, yaml
from check_gate import assert_product_approval
ROOT = Path(__file__).resolve().parents[1]
def check(condition, message):
    if not condition:
        raise AssertionError(message)
def main():
    gate = json.loads((ROOT / 'infra/execution-gate.json').read_text())
    assert_product_approval(gate)
    check((ROOT / gate['approval_reference']).is_file(), 'Missing owner approval record')
    for key in ('production_release_authorized', 'real_provider_calls_authorized', 'paid_resource_creation_authorized'):
        check(gate[key] is False, 'Separate approval required: ' + key)
    pins = json.loads((ROOT / 'infra/action-pins.json').read_text())['actions']
    files = list((ROOT / '.github/workflows').glob('*.yml'))
    check(any(f.name == 'environment-preflight.yml' for f in files), 'Required environment checks missing')
    action_count = 0
    for f in files:
        raw = f.read_text()
        d = yaml.load(raw, Loader=yaml.BaseLoader)
        check(d['permissions'] == {'contents': 'read'}, 'CI credentials must stay read-only')
        check('secrets.' not in raw, 'No unapproved secret injection')
        check('pull_request_target' not in raw, 'Privileged PR trigger forbidden')
        if f.name == 'environment-preflight.yml':
            check(set(d['on']) == {'workflow_dispatch', 'push', 'pull_request'}, 'Unexpected infrastructure trigger')
            check(d['on']['push'] == {'branches': ['main'], 'paths': ['infra/ci-request.json']}, 'Unexpected infrastructure main trigger')
            check(d['on']['pull_request'] == {'branches': ['main'], 'types': ['opened', 'synchronize', 'reopened', 'ready_for_review']}, 'Required checks must run on every main PR')
            check(d['jobs']['preflight']['strategy']['matrix']['os'] == ['ubuntu-24.04', 'macos-15', 'macos-15-intel'], 'Dual-architecture hosts missing')
        for job in d['jobs'].values():
            check(0 < int(job['timeout-minutes']) <= 30, 'Unbounded CI job')
            check('self-hosted' not in str(job.get('runs-on', '')), 'Personal runners not approved')
            for step in job['steps']:
                if 'uses' in step:
                    action, sha = step['uses'].split('@')
                    check(re.fullmatch('[0-9a-f]{40}', sha) is not None, 'Unpinned action')
                    check(pins[action]['commit'] == sha, 'Action pin mismatch')
                    action_count += 1
    check((ROOT / 'package-lock.json').is_file(), 'Missing Node lockfile')
    check((ROOT / 'tools/environment-smoke/Cargo.lock').is_file(), 'Missing Rust fixture lockfile')
    print(json.dumps({'status': 'passed', 'scope': 'static_infrastructure_only', 'workflow_files': len(files), 'pinned_action_references': action_count, 'remote_ci_executed': False, 'product_development_authorized': True, 'product_tests_executed': False}, indent=2))
if __name__ == '__main__':
    main()
