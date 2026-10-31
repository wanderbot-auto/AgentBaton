"""Optional, read-only notification observations in the original real IPC run.

No runtime at import, IPC method dispatcher, HTTP override, SQL or fake backend.
The caller supplies its existing actual browser/Core fixture after all original
normal-case assertions. The separate report cannot pass before owned cleanup.
"""
from __future__ import annotations

from collections import Counter
import copy
import hashlib
import json
from pathlib import Path
import re
import time
import traceback

SECONDS = 15
MAX_EVENTS = 500
MAX_SOURCES_INSPECTED = 10
MAX_SCREENSHOTS = 4
READS = frozenset(('system.snapshot', 'projects.list', 'connections.list', 'sessions.list',
    'tasks.get', 'goal_scopes.list', 'events.subscribe', 'integration_context.get',
    'integration_targets.list', 'integration_policies.get', 'integration_policies.list',
    'integrations.get', 'integrations.list'))
CHECKS = ('actual accepted Task projection and mapped journal observations',
    'one Task notice for naturally repeated real Task/session events',
    'expanded real source metadata and exact original journal navigation',
    'same Task navigation and modal shortcut isolation',
    'explicit DND persists across UI reload with manual evidence retained',
    'probe bridge and private audit contain only existing bounded reads')
PNG_NAMES = ('accepted-task-desktop', 'real-source-desktop', 'original-journal-desktop', 'dnd-mobile')
CLEANUP = frozenset(('pending_reviews_stopped', 'browser_closed', 'playwright_stopped', 'http_shutdown',
    'http_thread_joined', 'private_transcript_saved', 'tools_assets_unchanged', 'fixture_removed'))
REPORT_KEYS = frozenset(('contract', 'status', 'flow_complete', 'dimension', 'not_claimed', 'source',
    'probe_source', 'checks', 'screenshots', 'cleanup_observed', 'observations'))


class Refusal(ValueError):
    pass


def require(value, message='Real notification probe refused'):
    if not value:
        raise Refusal(message)


def integer(value, minimum=0):
    return type(value) is int and minimum <= value <= 9007199254740991


def identifier(value):
    return type(value) is str and re.fullmatch(r'[A-Za-z0-9._:-]{1,256}', value) is not None


def compact(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def exact(a, b):
    if type(a) is not type(b):
        return False
    if type(a) is dict:
        return a.keys() == b.keys() and all(exact(a[key], b[key]) for key in a)
    if type(a) is list:
        return len(a) == len(b) and all(exact(x, y) for x, y in zip(a, b))
    return a == b


def remaining_ms(deadline):
    remaining = deadline - time.monotonic()
    require(remaining > 0, 'Fixed notification observation deadline expired')
    return max(1, int(remaining * 1000))


def readonly_counts(calls):
    require(type(calls) is list and len(calls) <= 100000)
    for call in calls:
        require(type(call) is dict and call.get('method') in READS,
            'Notification observation issued a write or an unknown method')
    return dict(Counter(call['method'] for call in calls))


def actual_observations(audit, task_id, project_id, acceptance):
    """Validate actual browser read replies without creating or replacing one."""
    readonly_counts(audit)
    require(identifier(task_id) and identifier(project_id) and type(acceptance) is dict
        and identifier(acceptance.get('id')) and integer(acceptance.get('sequence'), 1))
    latest = {}
    for call in audit:
        method = call['method']
        if method in ('system.snapshot', 'sessions.list', 'events.subscribe'):
            response = call.get('response')
            require(type(response) is dict and 'error' not in response and 'result' in response)
            latest[method] = response['result']
    require(set(latest) == {'system.snapshot', 'sessions.list', 'events.subscribe'})
    snapshot, sessions, batch = (latest[k] for k in ('system.snapshot', 'sessions.list', 'events.subscribe'))
    require(type(snapshot) is dict and snapshot.get('api_version') == 1
        and type(snapshot.get('api_version')) is int and snapshot.get('database_schema_version') == 17
        and type(snapshot.get('database_schema_version')) is int and integer(snapshot.get('sequence'))
        and type(snapshot.get('tasks')) is list and len(snapshot['tasks']) == 1)
    task = snapshot['tasks'][0]
    require(type(task) is dict and task.get('id') == task_id and task.get('project_id') == project_id
        and task.get('state') == 'accepted' and integer(task.get('revision'), 1)
        and integer(task.get('spec_revision'), 1) and identifier(task.get('acceptance_ref'))
        and task['acceptance_ref'] == acceptance.get('id')
        and integer(task.get('acceptance_sequence'), 1)
        and task['acceptance_sequence'] == acceptance.get('sequence') <= snapshot['sequence'])
    require(type(sessions) is list and 1 <= len(sessions) <= 1000)
    session_ids = set()
    for session in sessions:
        require(type(session) is dict and identifier(session.get('id')) and session['id'] not in session_ids
            and session.get('task_id') == task_id and session.get('project_id') == project_id
            and integer(session.get('revision'), 1) and session.get('state') == 'stopped')
        session_ids.add(session['id'])
    require(type(batch) is dict and integer(batch.get('sequence'))
        and integer(batch.get('next_cursor')) and batch['next_cursor'] <= batch['sequence']
        and batch.get('gap') is False and batch.get('byte_limited', False) is False
        and type(batch.get('events')) is list and len(batch['events']) <= MAX_EVENTS)
    mapped, all_ids = {}, set()
    for event in batch['events']:
        require(type(event) is dict and identifier(event.get('event_id')) and event['event_id'] not in all_ids
            and integer(event.get('sequence'), 1) and event['sequence'] <= batch['sequence']
            and integer(event.get('revision'), 1) and type(event.get('kind')) is str
            and len(event['kind']) <= 128 and type(event.get('source')) is str and len(event['source']) <= 256
            and type(event.get('occurred_at')) is str and len(event['occurred_at']) <= 80)
        all_ids.add(event['event_id'])
        if event.get('aggregate_id') == task_id or event.get('aggregate_id') in session_ids:
            mapped[event['event_id']] = event
    require(2 <= len(mapped) <= MAX_EVENTS, 'Natural journal did not supply multiple mapped events')
    accepted_events = [event for event in mapped.values() if event['kind'] == 'task.accepted'
        and event['aggregate_id'] == task_id and event['sequence'] == task['acceptance_sequence']]
    stopped_events = [event for event in mapped.values() if event['kind'] == 'session.stopped'
        and event['aggregate_id'] in session_ids]
    require(len(accepted_events) == 1 and bool(stopped_events), 'Actual acceptance and stopped-session sources required')
    return task, mapped, accepted_events[0], sha(compact(latest))


def pending_report(source, probe_source):
    require(type(source) is str and re.fullmatch('[a-f0-9]{40}', source) is not None)
    return {'contract': 'agentbaton.task-notifications-real-ipc.v1', 'status': 'NOT_RUN',
        'flow_complete': False, 'dimension': 'Linux built React/Chromium + existing finite TEST bridge + actual Core17 Unix IPC',
        'not_claimed': ['production Tauri IPC', 'macOS Native', 'live Provider/model', 'OS notifications',
            'background notification delivery', '100 real repeated events', 'whole product completion'],
        'source': source, 'probe_source': copy.deepcopy(probe_source), 'checks': [], 'screenshots': [],
        'cleanup_observed': False, 'observations': {}}


def seal_report(report, base_report):
    cleanup = base_report.get('cleanup', {})
    joined = cleanup.get('daemon_joined', {})
    clean = type(cleanup) is dict and set(cleanup) == CLEANUP | {'daemon_joined'} \
        and all(cleanup[k] is True for k in CLEANUP) and type(joined) is dict \
        and set(joined) == {'exit', 'joined', 'forced_kill'} and type(joined.get('exit')) is int \
        and joined['exit'] == 0 and joined['joined'] is True and joined['forced_kill'] is False
    report['cleanup_observed'] = clean
    checks = report.get('checks', [])
    screenshots = report.get('screenshots', [])
    passed = report.get('flow_complete') is True and base_report.get('status') == 'passed' and clean \
        and [row.get('name') for row in checks] == list(CHECKS) \
        and all(row.get('status') == 'passed' for row in checks) \
        and [row.get('name') for row in screenshots] == list(PNG_NAMES) and 'failure' not in report
    report['status'] = 'passed' if passed else 'failed_or_interrupted'
    return passed


def run_probe(page, host, bounds, fixture, accepted, server, report, output, write_private):
    """Only UI actions and observation of the original host audit; no new RPC API."""
    from playwright.sync_api import expect
    deadline = time.monotonic() + SECONDS
    with host.lock:
        audit_start = len(host.audit)
    calls_start = len(server.calls)
    def bounded(action):
        timeout = remaining_ms(deadline)
        page.set_default_timeout(timeout); page.set_default_navigation_timeout(timeout)
        value = action()
        remaining_ms(deadline)
        return value
    def visible(locator):
        expect(locator).to_be_visible(timeout=remaining_ms(deadline)); remaining_ms(deadline)
    def checked(name, **facts):
        remaining_ms(deadline)
        require(name == CHECKS[len(report['checks'])])
        report['checks'].append({'name': name, 'status': 'passed', **facts})
    def snapshot_audit():
        with host.lock:
            return copy.deepcopy(host.audit[audit_start:])
    def screenshot(name, focus):
        require(name == PNG_NAMES[len(report['screenshots'])] and len(report['screenshots']) < MAX_SCREENSHOTS)
        bounded(focus.scroll_into_view_if_needed)
        expect(focus).to_be_in_viewport(ratio=1, timeout=remaining_ms(deadline))
        box = bounded(focus.bounding_box)
        path = output / (name + '.png')
        bounded(lambda: page.screenshot(path=str(path)))
        raw = path.read_bytes(); require(0 < len(raw) <= 16777216)
        report['screenshots'].append({'name': name, 'sha256': sha(raw), 'bytes': len(raw),
            'viewport': page.viewport_size, 'focus_box_before_capture_observation': box,
            'pixel_synchronized': False, 'dimension': 'Linux Chromium only'})
    try:
        bounded(lambda: page.set_viewport_size({'width': 1440, 'height': 1100}))
        bounded(lambda: page.get_by_role('button', name='Refresh', exact=True).click())
        while True:
            audit = snapshot_audit()
            if all(any(row.get('method') == method and 'result' in row.get('response', {}) for row in audit)
                for method in ('system.snapshot', 'sessions.list', 'events.subscribe')):
                break
            bounded(lambda: page.wait_for_timeout(min(25, remaining_ms(deadline))))
        task, mapped, acceptance_event, reply_hash = actual_observations(audit, bounds.task, bounds.project, accepted['task_acceptance'])
        checked(CHECKS[0], actual_reply_sha256=reply_hash, task_revision=task['revision'],
            acceptance_sequence=task['acceptance_sequence'], mapped_event_count=len(mapped))
        trigger = page.get_by_role('button', name='Notifications (1)', exact=True)
        visible(trigger); bounded(trigger.click)
        dialog = page.get_by_role('dialog', name='Task notifications', exact=True)
        visible(dialog)
        row = dialog.get_by_test_id('task-notification')
        expect(row).to_have_count(1, timeout=remaining_ms(deadline))
        expect(row).to_have_attribute('data-task-id', bounds.task, timeout=remaining_ms(deadline))
        visible(row.get_by_text('Task accepted', exact=True))
        visible(row.get_by_text('Latest task status', exact=True))
        checked(CHECKS[1], notices=1, naturally_observed_events=len(mapped), synthetic_repeated_events=0)
        screenshot(PNG_NAMES[0], row.get_by_text('Task accepted', exact=True))
        bounded(lambda: row.get_by_text('Sources (' + str(len(mapped)) + ')', exact=True).click())
        sources = row.locator('details > ol > li > details')
        require(1 <= sources.count() <= MAX_SOURCES_INSPECTED)
        inspected, selected = [], None
        for index in range(sources.count()):
            source = sources.nth(index)
            bounded(lambda: source.locator(':scope > summary').click())
            labels = bounded(lambda: source.locator(':scope > dl > dt').all_text_contents())
            values = bounded(lambda: source.locator(':scope > dl > dd').all_text_contents())
            require(len(labels) == len(values) == 5 and len(set(labels)) == 5)
            fields = dict(zip(labels, values)); original = mapped.get(fields.get('Event ID'))
            require(original is not None and fields == {'Event ID': original['event_id'],
                'Aggregate ID': original['aggregate_id'], 'Observation source': original['source'],
                'revision': str(original['revision']), 'observed at': original['occurred_at']})
            require(bounded(lambda: source.locator(':scope > summary').inner_text()) == original['kind'] + ' · #' + str(original['sequence']))
            inspected.append(original['event_id'])
            if original['event_id'] == acceptance_event['event_id']:
                selected = source
            else:
                bounded(lambda: source.locator(':scope > summary').click())
        require(selected is not None, 'Real acceptance reference not present in rendered source page')
        screenshot(PNG_NAMES[1], selected.get_by_text(acceptance_event['event_id'], exact=True))
        bounded(lambda: selected.get_by_role('button', name='View loaded original event', exact=True).click())
        visible(page.get_by_role('heading', name='History', exact=True))
        original_row = page.locator('[id="journal-event-' + acceptance_event['event_id'] + '"]')
        expect(original_row).to_have_count(1, timeout=remaining_ms(deadline))
        expect(original_row.locator(':scope > details')).to_have_attribute('open', '', timeout=remaining_ms(deadline))
        visible(original_row.locator('pre'))
        require(exact(json.loads(bounded(lambda: original_row.locator('pre').inner_text())), acceptance_event['payload']))
        screenshot(PNG_NAMES[2], original_row.locator(':scope > details > summary'))
        checked(CHECKS[2], inspected_references=inspected, original_event_id=acceptance_event['event_id'],
            original_event_sequence=acceptance_event['sequence'], original_payload_matched=True)
        bounded(trigger.click)
        bounded(lambda: page.get_by_role('dialog').get_by_role('button', name='Open task', exact=True).click())
        thread = page.get_by_test_id('task-thread')
        expect(thread).to_have_attribute('data-task-id', bounds.task, timeout=remaining_ms(deadline))
        bounded(trigger.click)
        for shortcut in ('Control+k', 'Control+n', 'Meta+k', 'Meta+n'):
            bounded(lambda: page.keyboard.press(shortcut))
            expect(page.get_by_role('dialog')).to_have_count(1, timeout=remaining_ms(deadline))
            expect(thread).to_have_attribute('data-task-id', bounds.task, timeout=remaining_ms(deadline))
        bounded(lambda: page.keyboard.press('Escape'))
        expect(page.get_by_role('dialog')).to_have_count(0, timeout=remaining_ms(deadline))
        require(bounded(lambda: page.evaluate("(()=>{const shell=document.querySelector('.shell');const roots=Array.from(document.body.children).filter(e=>e.contains(shell));return !!shell&&roots.length===1&&roots.every(e=>!e.inert&&e.getAttribute('aria-hidden')===null)})()")))
        expect(trigger).to_be_focused(timeout=remaining_ms(deadline))
        checked(CHECKS[3], same_task=True, modal_count=1, shortcuts=4, escape_background_restored=True)
        bounded(trigger.click)
        dnd = page.get_by_role('dialog').get_by_role('checkbox', name='Do not disturb', exact=True)
        expect(dnd).not_to_be_checked(timeout=remaining_ms(deadline)); bounded(dnd.check)
        bounded(lambda: page.keyboard.press('Escape'))
        muted = page.get_by_role('button', name='Notifications · do not disturb', exact=True)
        visible(muted)
        expect(muted.locator('span')).to_have_count(0, timeout=remaining_ms(deadline))
        expect(muted).not_to_have_attribute('data-attention', 'true', timeout=remaining_ms(deadline))
        bounded(lambda: page.reload(wait_until='networkidle', timeout=remaining_ms(deadline)))
        visible(muted)
        bounded(muted.click)
        dialog = page.get_by_role('dialog', name='Task notifications', exact=True)
        expect(dialog.get_by_role('checkbox', name='Do not disturb', exact=True)).to_be_checked(timeout=remaining_ms(deadline))
        visible(dialog.get_by_text('Task accepted', exact=True))
        visible(dialog.get_by_text('Sources (' + str(len(mapped)) + ')', exact=True))
        bounded(lambda: page.set_viewport_size({'width': 390, 'height': 844}))
        screenshot(PNG_NAMES[3], dialog.get_by_role('checkbox', name='Do not disturb', exact=True))
        require(bounded(lambda: page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')))
        bounded(lambda: dialog.get_by_role('checkbox', name='Do not disturb', exact=True).uncheck())
        bounded(lambda: page.keyboard.press('Escape'))
        visible(trigger)
        checked(CHECKS[4], ui_reload=True, manual_evidence=True, os_notification_delivery_tested=False)
        bridge_counts = readonly_counts(copy.deepcopy(server.calls[calls_start:]))
        audit_counts = readonly_counts(snapshot_audit())
        require(all(bridge_counts.get(method, 0) > 0 and audit_counts.get(method, 0) > 0
            for method in ('system.snapshot', 'sessions.list', 'events.subscribe')))
        checked(CHECKS[5], bridge_read_counts=bridge_counts, private_read_counts=audit_counts)
        report['observations'] = {'task_id': bounds.task, 'acceptance_ref': task['acceptance_ref'],
            'source_event_count': len(mapped), 'inspected_event_count': len(inspected),
            'actual_reply_sha256': reply_hash, 'fixed_deadline_seconds': SECONDS}
        report['flow_complete'] = True; report['status'] = 'observed_pending_cleanup'
    except BaseException as error:
        report['failure'] = {'kind': type(error).__name__, 'stage': 'real_notification_observation'}
        write_private(output / 'failure.json', {'traceback': traceback.format_exc()})
    return report['flow_complete']
