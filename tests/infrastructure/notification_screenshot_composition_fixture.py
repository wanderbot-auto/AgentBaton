"""Exact composition of the reviewed notification and closed screenshot layers."""
import hashlib
from tests.infrastructure import integration_screenshot_failure_fixture as S

NOTIFICATION_BEFORE_SHA256 = '55f8144175c0a1d0809da7c95c66339c535a6f8e6d90ece9d11ecb89227df993'
NOTIFICATION_AFTER_SHA256 = '9561fb33327b2cbf3cf0e0d7a51b755245bf90ce4a9dabcdbeb39cfe523fd70f'
NOTIFICATION_EDITS = [('', 'import types\n'),
 ('',
  '\n'
  '\n'
  'def notification_probe_bytes(expected):\n'
  '    # Fixed repository path only; never accept a module/path from CLI or HTTP.\n'
  "    relative = 'apps/desktop/tests/task_notifications_real_ipc_probe.py'\n"
  "    require(subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() "
  '== expected\n'
  "        and subprocess.check_output(['git', 'status', '--porcelain=v1', "
  "'--untracked-files=no'], cwd=ROOT) == b'')\n"
  "    path = Path(__file__).with_name('task_notifications_real_ipc_probe.py')\n"
  '    pin = pinned_file(path, MAX_REVIEW)\n'
  '    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)\n'
  "    with os.fdopen(fd, 'rb') as stream:\n"
  '        raw = stream.read(MAX_REVIEW + 1)\n'
  "    tracked = subprocess.check_output(['git', 'show', 'HEAD:' + relative], cwd=ROOT)\n"
  "    require(0 < len(raw) <= MAX_REVIEW and len(raw) == pin['bytes']\n"
  "        and raw == tracked and digest(raw) == pin['sha256'])\n"
  "    return {'source': expected, 'relative_path': relative, 'sha256': pin['sha256'], 'bytes': "
  "pin['bytes']}, raw\n"
  '\n'
  '\n'
  'def notification_probe_source(expected):\n'
  '    return notification_probe_bytes(expected)[0]\n'
  '\n'
  '\n'
  'def load_notification_probe(expected):\n'
  '    record, raw = notification_probe_bytes(expected)\n'
  "    path = Path(__file__).with_name('task_notifications_real_ipc_probe.py')\n"
  '    # Compile the exact reviewed bytes. SourceFileLoader may instead execute an\n'
  '    # untracked timestamp/size-matching .pyc even when the .py hash is correct.\n'
  "    module = types.ModuleType('_fixed_task_notifications_real_ipc_probe')\n"
  '    module.__file__ = str(path)\n'
  "    exec(compile(raw, str(path), 'exec'), module.__dict__)\n"
  '    require(notification_probe_source(expected) == record)\n'
  '    return module, record\n'),
 ('',
  "    notification_probe = getattr(args, '_notification_probe', None) if not failing else None\n"
  '    notification_report = None\n'
  '    notification_passed = False\n'
  '    if notification_probe is not None:\n'
  "        notification_output = output / 'notification-probe'; "
  'notification_output.mkdir(mode=0o700)\n'
  "        notification_report = notification_probe.pending_report(attribution['source'], "
  'args._notification_probe_source)\n'),
 ('',
  '        if notification_probe is not None:\n'
  "            phase = 'notification_probe'\n"
  '            require(notification_probe.run_probe(page, host, bounds, fixture, accepted, '
  'server,\n'
  "                notification_report, notification_output, private_json), 'Real notification "
  "probe failed')\n"),
 ('',
  '            if notification_probe is not None:\n'
  "                require(notification_probe_source(attribution['source']) == "
  'args._notification_probe_source)\n'),
 ("    return report['status'] == 'passed'\n",
  '        if notification_probe is not None:\n'
  '            notification_passed = notification_probe.seal_report(notification_report, report)\n'
  "            private_json(notification_output / 'report.json', notification_report)\n"
  "    return report['status'] == 'passed' and (notification_probe is None or "
  'notification_passed)\n'),
 ('',
  "    parser.add_argument('--notification-probe', action='store_true',\n"
  "        help='After the original normal-case checks only, observe real in-app notification UI "
  "with existing read-only IPC')\n"),
 ('    except ImportError as error:\n',
  "        if args.notification_probe and args.case != 'command-failure':\n"
  '            args._notification_probe, args._notification_probe_source = '
  'load_notification_probe(args.expected_source_head)\n'
  '    except (ImportError, Refusal, OSError) as error:\n')]


def _digest(text):
    return hashlib.sha256(text.encode()).hexdigest()


def _inverse(text, edits, expected):
    for old, new in edits:
        if text.count(new) != 1:
            raise AssertionError('Closed composition anchor changed')
        text = text.replace(new, old, 1)
    if _digest(text) != expected:
        raise AssertionError('Closed composition did not restore complete original source')
    return text


def strip_composed_screenshot_layer(text):
    digest = _digest(text)
    if digest in (NOTIFICATION_BEFORE_SHA256, S.DRIVER_BEFORE_SHA256):
        return text
    if digest == S.DRIVER_AFTER_SHA256:
        return S.strip_screenshot_driver_layer(text)
    if digest != NOTIFICATION_AFTER_SHA256:
        raise AssertionError('Unknown notification/screenshot composition')
    return _inverse(text, S.DRIVER_EDITS, NOTIFICATION_BEFORE_SHA256)


def strip_composed_notification_layer(text):
    digest = _digest(text)
    if digest in (S.DRIVER_BEFORE_SHA256, S.DRIVER_AFTER_SHA256):
        return text
    if digest == NOTIFICATION_BEFORE_SHA256:
        return _inverse(text, NOTIFICATION_EDITS, S.DRIVER_BEFORE_SHA256)
    if digest != NOTIFICATION_AFTER_SHA256:
        raise AssertionError('Unknown notification/screenshot composition')
    return _inverse(text, NOTIFICATION_EDITS, S.DRIVER_AFTER_SHA256)
