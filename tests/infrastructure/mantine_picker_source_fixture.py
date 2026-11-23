"""Restore exact pre-picker bytes before the protected historical driver inverses."""
import hashlib


PICKER_LAYERS = {
    'apps/desktop/src-tauri/src/bin/native_smoke.rs': (
        '25eee6d1495ba49ea928eb6afd50bcbebbe5f324fba827b54688ed8920e464b8',
        '58721865993d21d3aa7a34be91ae1080bcb850f9c157e7daa9611bf7d5f8ee9c',
        (
            (1220, 1236, 'f5335081f5ac5110a9fd9c29a3d2464aac65ff7d8449764fcc4209e4b9a0071a',
             b"  let projectSelect;\n  while(!(projectSelect=document.querySelector('.composer-project select'))||![...projectSelect.options].some(option=>option.value===fixtureProject.id)){require(Date.now()<deadline,'real project composer choice did not load');await new Promise(r=>setTimeout(r,25));}\n  Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(projectSelect,fixtureProject.id);\n  projectSelect.dispatchEvent(new Event('change',{bubbles:true}));\n"),
        ),
    ),
    'apps/desktop/tests/integration-real-ipc.py': (
        '53f56d613c667c3d5f9b5ce2d185c45f0ff57512afe41f18a139ff688f11c730',
        'f730621d77cb75f94e214794615b1125ba0f33cc454431a3432a8b43e8ddd0bb',
        (
            (66, 74, '247bdef43c8f08c6d7b54852dbc2dd420f9d21a08b8e9d78d7dabddd3b52126b',
             b''),
            (1173, 1174, '408625249215eaa680911df331f187abc4751eea2ddf5772172eee984bdfea84',
             b"        panel.get_by_label('Existing local target branch').select_option(TARGET)\n"),
        ),
    ),
    'apps/desktop/tests/task-workspace-binding-real-ipc.py': (
        '5323ce6cc0364912b23b6ced5deb6b88150a2bd64d4f48cebd50acb25d8c0860',
        '9c5a21b9a1964fb478ef1e0405ef36bf7652c28d658dbaf89c7b5ae20902595d',
        (
            (57, 79, 'cff017d0ed24201b7a2e98056d2abccbf71d10961bd6568334f8ac4f70fc0bbf',
             b''),
            (944, 947, '65d68865dc206f46464442aa6f8ebd8fac65ff25ec77959aeb7ae531f501b369',
             b"            expect(dialog.locator('#session-task')).to_have_value(current_task['id'])\n            expect(dialog.locator('#session-project')).to_have_value(project['id'])\n            expect(dialog.locator('#session-workspace')).to_have_value(workspace_id)\n"),
            (950, 954, '856e41924455185ad7a425c7bba437db7fb00c898cffa834c03ba7780f50e566',
             b"            expect(dialog.locator('#session-connection')).to_have_value('')\n            expect(dialog.locator('#session-connection option')).to_have_count(1)\n            expect(dialog.locator('#session-grant')).to_have_value('')\n            expect(dialog.locator('#session-grant option')).to_have_count(1)\n"),
            (968, 969, '4546f54baa2c48130180ab8f5b6587889cad3cafb7aaa42e134ba6264763b41b',
             b"        dialog.locator('#session-task').select_option(tasks[0]['id'])\n"),
            (971, 972, '60ac923ed3a2417751be23d69949a6a36b71809c11791353f81b09bfffabed04',
             b"        dialog.locator('#session-task').select_option(tasks[1]['id'])\n"),
            (981, 982, 'b69806c5f6ec2cccf7e8617d699807032cccd3534c02258f7d501e5da18fe429',
             b"        expect(dialog.locator('#session-workspace')).to_have_value(b1['receipt']['workspace_id'])\n"),
            (1078, 1079, 'cb5aff24b4ac0440351d507569079fc9b9f51761d86f61e835031c66a4bb3007',
             b"        expect(dialog.locator('#session-workspace')).to_have_value(b3['receipt']['workspace_id'])\n"),
        ),
    ),
    'apps/desktop/tests/factory-child-goal-real-ipc.py': (
        '7e2df003b6ec45dbea0aa70103a49d592f0e99c2fd442d5b37aedc5b87c67657',
        '73470592d65790678a57a17bb14b13351f261902fc87411039e6ec5ff31c2e11',
        (
            (34, 35, 'e3610cbbec7e26d1ff5fb4cf9c5895fc97c49218907937721d334b12b62008c7',
             b"PLANNING_SHA256 = 'fe607d2c8bfeaf9ee34a0656b823bec9e00c11c9152501ebbd1e273bfbf79f39'\n"),
            (2043, 2045, 'a0d10e694c6dfff4e0c1805831e563451336753cfef6fd2f5ff21bc4836ca947',
             b"        planning.fixed_form_control(form,'Existing root Grant','select').select_option(grant['id'])\n        planning.fixed_form_control(form,'Existing connections','select').select_option([cid]);planning.fixed_form_control(form,'Existing workspaces','select').select_option([workspace['id']])\n"),
        ),
    ),
    'apps/desktop/tests/factory-child-planning-real-ipc.py': (
        '0a70100307a52439c63d66c537b98a0ab16e5b15fa76830276aa24155ced6c13',
        'fe607d2c8bfeaf9ee34a0656b823bec9e00c11c9152501ebbd1e273bfbf79f39',
        (
            (580, 591, 'ce9a89d69cb8df18daab45e562ca719c5d6d31f80bcd227a57dc8321ef5690b2',
             b''),
            (1203, 1206, 'c6486e4607a5a50f9233d830d48757125f8bfd96465e760ec684e17bcf108497',
             b"        fixed_form_control(form,'Existing root Grant','select').select_option(grant['id'])\n        fixed_form_control(form,'Existing connections','select').select_option([cid])\n        fixed_form_control(form,'Existing workspaces','select').select_option([workspace['id']])\n"),
        ),
    ),
}


def restore_mantine_picker_source(path, raw, *, error=AssertionError):
    layer = PICKER_LAYERS.get(path)
    if layer is None or type(raw) is not bytes:
        return raw
    current, previous, spans = layer
    if hashlib.sha256(raw).hexdigest() != current:
        return raw  # Historical and unknown inputs still face the unchanged inner pins.
    lines = raw.splitlines(keepends=True)
    for start, end, span_sha, replacement in reversed(spans):
        if hashlib.sha256(b''.join(lines[start:end])).hexdigest() != span_sha:
            raise error('Unexpected Mantine picker source span')
        lines[start:end] = replacement.splitlines(keepends=True)
    restored = b''.join(lines)
    if hashlib.sha256(restored).hexdigest() != previous:
        raise error('Mantine picker inverse did not restore the pinned predecessor')
    return restored
