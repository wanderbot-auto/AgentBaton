#!/usr/bin/env python3
"""Static infrastructure checks; never proof of product correctness or deployed policy."""
import json
import re
from pathlib import Path

import yaml

from check_gate import assert_product_approval

ROOT = Path(__file__).resolve().parents[1]
READ_ONLY = {'contents': 'read'}
HOSTED_RUNNERS = {'ubuntu-24.04', 'macos-15'}
PRIVILEGED_EVENTS = {'pull_request_target', 'workflow_run'}
GITHUB_TOKEN = re.compile(
    r"\$\{\{\s*github\s*(?:\.\s*token|\[\s*'token'\s*\])\s*\}\}", re.IGNORECASE
)
QUOTED = r"'(?:[^']|'')*'"
EXPRESSION_START = '${{'
EXPRESSION_TOKEN = re.compile(QUOTED + r'|}}|[A-Za-z_][A-Za-z0-9_-]*|[^\s]')


class WorkflowLoader(yaml.BaseLoader):
    """Keep Actions scalars as strings and reject ambiguous YAML mappings."""

    def construct_mapping(self, node, deep=False):
        mapping = super().construct_mapping(node, deep=deep)
        keys = [key.casefold() if isinstance(key, str) else None for key in mapping]
        if len(keys) != len(node.value) or len(keys) != len(set(keys)) or {None, '<<'}.intersection(keys):
            raise yaml.YAMLError('Ambiguous workflow mapping')
        return mapping


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def fields(value, location):
    check(isinstance(value, dict), f'{location}: expected a mapping')
    return {key.casefold(): item for key, item in value.items()}


def read_workflow(file):
    location = '.github/workflows/' + file.name
    check(re.fullmatch(r'[A-Za-z0-9_.-]+\.ya?ml', file.name), '.github/workflows: invalid workflow filename')
    check(file.is_file() and not file.is_symlink(), f'{location}: expected a regular workflow file')
    try:
        parsed = yaml.load(file.read_text(encoding='utf-8'), Loader=WorkflowLoader)
    except (yaml.YAMLError, RecursionError, UnicodeError):
        # Parser errors can include fragments of raw YAML.
        raise AssertionError(f'{location}: invalid or ambiguous workflow YAML') from None
    return fields(parsed, location)


def workflow_events(triggers, location):
    if isinstance(triggers, dict):
        return fields(triggers, location)
    if isinstance(triggers, str):
        triggers = [triggers]
    check(isinstance(triggers, list) and triggers and all(isinstance(event, str) for event in triggers),
          f'{location}: invalid workflow triggers')
    return {event.casefold(): '' for event in triggers}


def contains_secret_context(value, *, implicit=False):
    opening = 0 if implicit else value.find(EXPRESSION_START)
    while opening != -1:
        previous = None
        start = opening if implicit else opening + len(EXPRESSION_START)
        for token in EXPRESSION_TOKEN.finditer(value, start):
            text = token.group()
            if text == '}}':
                break
            if text.casefold() == 'secrets' and previous != '.':
                return True
            previous = text
        else:
            return False
        implicit = False
        opening = value.find(EXPRESSION_START, token.end())
    return False


def validate_credential_expressions(workflow, location):
    # YAML aliases can share subtrees. Visit each collection once.
    pending = [workflow]
    seen = set()
    while pending:
        value = pending.pop()
        if isinstance(value, str):
            check(not contains_secret_context(value),
                  f'{location}: secret context forbidden in CI expressions')
        elif isinstance(value, (dict, list)) and id(value) not in seen:
            seen.add(id(value))
            if isinstance(value, dict):
                for key, item in value.items():
                    if (key.casefold() == 'if' and isinstance(item, str)
                            and not item.lstrip().startswith(EXPRESSION_START)):
                        check(not contains_secret_context(item, implicit=True),
                              f'{location}: secret context forbidden in CI expressions')
                pending.extend(value)
                pending.extend(value.values())
            else:
                pending.extend(value)


def validate_job_timeout(workflow_name, job_name, job):
    maximum = 30
    if workflow_name == 'product.yml' and job_name == 'desktop':
        # One explicit cold-build budget.
        check(job.get('runs-on') == '${{ matrix.os }}', 'Unexpected product build host selector')
        strategy = fields(job.get('strategy'), '.github/workflows/product.yml.jobs.desktop.strategy')
        check(strategy.get('matrix') == {'os': ['macos-15']}, 'Unexpected product build matrix')
        check(str(strategy.get('max-parallel')) == '2', 'Unexpected product build concurrency')
        check(str(strategy.get('fail-fast')).lower() == 'false', 'Product runner must complete')
        maximum = 60
    value = job.get('timeout-minutes')
    check(type(value) in (str, int), 'Invalid CI job timeout')
    try:
        timeout = int(value)
    except ValueError:
        raise AssertionError('Invalid CI job timeout') from None
    check(0 < timeout <= maximum, 'Unbounded CI job')


def validate_runner(job, location):
    runner = job.get('runs-on')
    if runner == '${{ matrix.os }}':
        strategy = fields(job.get('strategy'), f'{location}.strategy')
        matrix = fields(strategy.get('matrix'), f'{location}.strategy.matrix')
        hosts = matrix.get('os')
        check(set(matrix) == {'os'} and isinstance(hosts, list) and hosts
              and all(isinstance(host, str) and host in HOSTED_RUNNERS for host in hosts),
              f'{location}: approved GitHub-hosted runners required')
    else:
        check(isinstance(runner, str) and runner in HOSTED_RUNNERS,
              f'{location}: approved GitHub-hosted runners required')


def validate_registry_credentials(job, location):
    images = []
    if 'container' in job:
        images.append((f'{location}.container', job['container']))
    images.extend((f'{location}.services', service)
                  for service in fields(job.get('services', {}), f'{location}.services').values())
    for image_path, image in images:
        if isinstance(image, dict):
            check('credentials' not in fields(image, image_path),
                  f'{image_path}: registry credentials forbidden')
        else:
            check(isinstance(image, str) and EXPRESSION_START not in image,
                  f'{image_path}: expected a literal image or inspectable settings')


def validate_steps(steps, pins, location):
    check(isinstance(steps, list) and steps, f'{location}: expected job steps')
    action_count = 0
    for index, step in enumerate(steps):
        step_path = f'{location}.steps[{index}]'
        step = fields(step, step_path)
        check(not {'permissions', 'secrets', 'environment'}.intersection(step),
              f'{step_path}: workflow-level credential settings forbidden in steps')
        if 'uses' not in step:
            continue
        reference = step['uses']
        match = re.fullmatch(r'([A-Za-z0-9_./-]+)@([0-9a-f]{40})', reference) if isinstance(reference, str) else None
        check(match is not None, f'{step_path}: Unpinned action')
        action, sha = match.groups()
        check(action in pins and pins[action]['commit'] == sha, f'{step_path}: Action pin mismatch')
        inputs = fields(step.get('with', {}), f'{step_path}.with')
        if 'token' in inputs:
            token = inputs['token']
            check(isinstance(token, str) and GITHUB_TOKEN.fullmatch(token),
                  f'{step_path}: action token must use the built-in GitHub token')
        if action == 'actions/checkout':
            check(inputs.get('persist-credentials') == 'false',
                  f'{step_path}: checkout must disable credential persistence')
            check('ssh-key' not in inputs, f'{step_path}: checkout SSH key forbidden')
        action_count += 1
    return action_count


def validate_workflows(directory, pins):
    files = sorted([*directory.glob('*.yml'), *directory.glob('*.yaml')])
    workflows = {file.name: read_workflow(file) for file in files}
    check('environment-preflight.yml' in workflows, 'Required environment checks missing')
    action_count = 0
    for name, workflow in workflows.items():
        location = f'.github/workflows/{name}'
        check(workflow.get('permissions') == READ_ONLY, f'{location}: CI credentials must stay read-only')
        triggers = workflow_events(workflow.get('on'), f'{location}.on')
        check(not PRIVILEGED_EVENTS.intersection(triggers), f'{location}: Privileged PR trigger forbidden')
        call = triggers.get('workflow_call')
        if isinstance(call, dict):
            check('secrets' not in fields(call, f'{location}.on.workflow_call'),
                  f'{location}.on.workflow_call: reusable workflow secret declarations forbidden')
        elif 'workflow_call' in triggers:
            check(call == '', f'{location}.on.workflow_call: invalid reusable workflow trigger')
        jobs = fields(workflow.get('jobs'), f'{location}.jobs')
        check(jobs, f'{location}: expected at least one job')
        if name == 'environment-preflight.yml':
            validate_preflight(triggers, jobs)
        for job_name, job in jobs.items():
            check(re.fullmatch(r'[a-z_][a-z0-9_-]*', job_name), f'{location}.jobs: invalid job id')
            job_path = f'{location}.jobs.{job_name}'
            job = fields(job, job_path)
            if 'permissions' in job:
                check(job['permissions'] in ({}, READ_ONLY), f'{job_path}: job credentials must stay read-only')
            check('environment' not in job, f'{job_path}: CI environment binding forbidden')
            check('secrets' not in job, f'{job_path}: reusable workflow secret transfer forbidden')
            validate_credential_expressions(job, job_path)
            check('uses' not in job, f'{job_path}: reusable workflow calls are not approved')
            validate_job_timeout(name, job_name, job)
            validate_runner(job, job_path)
            validate_registry_credentials(job, job_path)
            action_count += validate_steps(job.get('steps'), pins, job_path)
        validate_credential_expressions({key: value for key, value in workflow.items() if key != 'jobs'}, location)
    return len(files), action_count


def validate_preflight(triggers, jobs):
    check(set(triggers) == {'workflow_dispatch', 'push', 'pull_request'}, 'Unexpected infrastructure trigger')
    check(triggers['push'] == {'branches': ['main'], 'paths': ['infra/ci-request.json']},
          'Unexpected infrastructure main trigger')
    check(triggers['pull_request'] == {'branches': ['main'], 'types': ['opened', 'synchronize', 'reopened', 'ready_for_review']},
          'Required checks must run on every main PR')
    preflight = fields(jobs.get('preflight'), '.github/workflows/environment-preflight.yml.jobs.preflight')
    strategy = fields(preflight.get('strategy'), 'Environment strategy')
    check(preflight.get('name') == 'Environment / ${{ matrix.os }}'
          and preflight.get('runs-on') == '${{ matrix.os }}'
          and strategy.get('matrix') == {'os': ['ubuntu-24.04', 'macos-15']},
          'Expected Linux and Apple Silicon preflight hosts')


def main():
    gate = json.loads((ROOT / 'infra/execution-gate.json').read_text())
    assert_product_approval(gate)
    check((ROOT / gate['approval_reference']).is_file(), 'Missing owner approval record')
    for key in ('production_release_authorized', 'real_provider_calls_authorized', 'paid_resource_creation_authorized'):
        check(gate[key] is False, 'Separate approval required: ' + key)
    pins = json.loads((ROOT / 'infra/action-pins.json').read_text())['actions']
    workflow_count, action_count = validate_workflows(ROOT / '.github/workflows', pins)
    check((ROOT / 'package-lock.json').is_file(), 'Missing Node lockfile')
    check((ROOT / 'tools/environment-smoke/Cargo.lock').is_file(), 'Missing Rust fixture lockfile')
    print(json.dumps({
        'status': 'passed',
        'scope': 'static_infrastructure_only',
        'workflow_files': workflow_count,
        'pinned_action_references': action_count,
        'remote_ci_executed': False,
        'product_development_authorized': True,
        'product_tests_executed': False,
    }, indent=2))


if __name__ == '__main__':
    main()
