"""Exact inverses for the closed screenshot-failure observation layer."""
import hashlib

DRIVER_BEFORE_SHA256 = '91ee42eb4112cab21f1ae3fba57e00743af1542dd4b3ef98833562605e43352a'
DRIVER_AFTER_SHA256 = '76f2915296d2aa2ac164b67bb221775dcea734e1efa8e694789138784bd5de92'
PROJECTOR_BEFORE_SHA256 = '86ad26c31679490177f2549ac4b468bfc02e7269ca277fe93b3c3e0a507e16ee'
PROJECTOR_AFTER_SHA256 = 'c938e5b16f3e833edd2d3bdef38ed39471b5172f9995f6f6f76c97e60d06f10c'
DRIVER_EDITS = [('        def shot(name, focus):\n',
  '        def shot(name, focus):\n'
  "            report['active_screenshot'] = {'name': name, 'step': 'scroll'}\n"),
 ('            expect(focus).to_be_in_viewport(ratio=1)\n',
  "            report['active_screenshot']['step'] = 'viewport'\n"
  '            expect(focus).to_be_in_viewport(ratio=1)\n'),
 ('            focus_box_before_capture = focus.bounding_box()\n',
  "            report['active_screenshot']['step'] = 'bounding_box'\n"
  '            focus_box_before_capture = focus.bounding_box()\n'),
 ("            path = output / (name + '.png'); page.screenshot(path=str(path))\n",
  "            report['active_screenshot']['step'] = 'capture'\n"
  "            path = output / (name + '.png'); page.screenshot(path=str(path))\n"
  "            report['active_screenshot']['step'] = 'record'\n"),
 ("                'dimension': 'Linux Chromium only'})\n",
  "                'dimension': 'Linux Chromium only'})\n"
  "            del report['active_screenshot']\n")]
PROJECTOR_EDITS = [('def runtime_report_observation(report, case, expected, manifest_hash, build_hash, harness):\n',
  'def failed_screenshot_observation(report):\n'
  "    if 'active_screenshot' not in report:\n"
  '        return None\n'
  "    require(report['status'] == 'failed_or_interrupted')\n"
  "    detail, completed = report['active_screenshot'], report['screenshots']\n"
  "    require(type(detail) is dict and set(detail) == {'name', 'step'}\n"
  "        and type(detail['name']) is str and detail['name'] in NAMES\n"
  "        and type(detail['step']) is str\n"
  "        and detail['step'] in ('scroll', 'viewport', 'bounding_box', 'capture', 'record')\n"
  '        and len(completed) < len(NAMES))\n'
  "    require([row.get('name') if type(row) is dict else None for row in completed] == "
  'list(NAMES[:len(completed)])\n'
  "        and detail['name'] == NAMES[len(completed)])\n"
  "    expected_phase = 'private_independent_review' if len(completed) < 2 else "
  "'real_owner_acceptance_and_watcher'\n"
  "    require(report['failure']['stage'] == expected_phase)\n"
  "    return {'name': detail['name'], 'step': detail['step'], 'completed_screenshots': "
  'len(completed)}\n'
  '\n'
  '\n'
  'def runtime_report_observation(report, case, expected, manifest_hash, build_hash, harness):\n'),
 ("        'http_faults', 'fixture_retained_privately'}\n",
  "        'http_faults', 'fixture_retained_privately', 'active_screenshot'}\n"),
 ("        'cleanup_observations': failed_cleanup_observation(report['cleanup'])}, source\n",
  "        'cleanup_observations': failed_cleanup_observation(report['cleanup']),\n"
  "        'screenshot_observation': failed_screenshot_observation(report)}, source\n")]


def _strip(text, edits, before, after, error):
    digest = hashlib.sha256(text.encode()).hexdigest()
    if digest == before:
        return text
    if digest != after:
        raise error('Unrecognized screenshot observation source')
    for old, new in edits:
        if text.count(new) != 1:
            raise error('Screenshot observation anchor changed')
        text = text.replace(new, old, 1)
    if hashlib.sha256(text.encode()).hexdigest() != before:
        raise error('Screenshot inverse does not restore complete old source')
    return text


def strip_screenshot_driver_layer(text):
    return _strip(text, DRIVER_EDITS, DRIVER_BEFORE_SHA256, DRIVER_AFTER_SHA256, AssertionError)


def strip_screenshot_projector_layer(text):
    return _strip(text, PROJECTOR_EDITS, PROJECTOR_BEFORE_SHA256, PROJECTOR_AFTER_SHA256, ValueError)
