"""Exact failure-only source layer before unchanged historical v1-v4 inverses."""
import hashlib

LAYERS = {'apps/desktop/tests/factory-child-goal-real-ipc.py': {'before': 'dd80aebe8fba1c1a031b10e77bdfba64d8d1d349819521fb2deb36a02435cd9d',
                                                       'after': 'c3d374a58907e57840cb21f6a6ed55a406056035dc39b807d93bfe7d465903ee',
                                                       'legacy': ['4b0332152c2dff9e4d4d7df94dede8ce23b0735c1b76adbef14398c142811b88',
                                                                  '7dc4732f18be492fddcc49bb989f233cd4fc409bb11de33a7951f7d5cf36e338',
                                                                  '9acadb5d494e7cd055b871530867fc273bff9588384beeb9415ffbdaa68c2610',
                                                                  'b959489950486874cf9730b9877dc36ee6c0121688dbc3fd9ef567138279268d',
                                                                  'dd80aebe8fba1c1a031b10e77bdfba64d8d1d349819521fb2deb36a02435cd9d'],
                                                       'edits': [['        '
                                                                  'base.collect_started_records(runtime,workspace,scope,record_context,verified,output,reaper)\n'
                                                                  '        '
                                                                  'collect_graph_records(runtime,workspace,scope,record_context,graph_verified,output,finite)\n',
                                                                  '        try:\n'
                                                                  '            '
                                                                  'base.collect_started_records(runtime,workspace,scope,record_context,verified,output,reaper)\n'
                                                                  '        except BaseException as error:\n'
                                                                  '            '
                                                                  "try:finite.capture_failure('started_records',error)\n"
                                                                  '            except BaseException:pass\n'
                                                                  '            raise\n'
                                                                  '        try:\n'
                                                                  '            '
                                                                  'collect_graph_records(runtime,workspace,scope,record_context,graph_verified,output,finite)\n'
                                                                  '        except BaseException as error:\n'
                                                                  '            '
                                                                  "try:finite.capture_failure('graph_records',error)\n"
                                                                  '            except BaseException:pass\n'
                                                                  '            raise\n'],
                                                                 ['finite=finite_projection.FiniteRuntimeDiagnosticsV4()',
                                                                  'finite=finite_projection.FiniteRuntimeDiagnosticsV5()']]},
 'scripts/record-child-goal-ci-evidence.py': {'before': '9907ea6815187bf57de2dd2706f2bc6f4c47456a5ff6c979e896d65b6ba7b768',
                                              'after': 'a628fa04b8799a96d57e6cad97f1ecfab266468e287acf5f6d70693a696af02e',
                                              'legacy': ['44d55431fad475248094fabd50cab3543241530dee37537276fa140fb0463f1a',
                                                         '5b515e01d220c1b0e9a2759ba3a74cc081e7122a1fa549583aed6a911fd47d69',
                                                         '9907ea6815187bf57de2dd2706f2bc6f4c47456a5ff6c979e896d65b6ba7b768',
                                                         'b51d1c3d4f371c6e1c908e7cd4be308f6c6f6262e2bcccbe1a073df2c545ae14'],
                                              'edits': [['def finite_diagnostics(value):\n',
                                                         'def finite_diagnostics(value):\n'
                                                         '    # V5 is failure-only: preserve and validate '
                                                         'the complete original v4 first.\n'
                                                         '    if type(value)is dict and '
                                                         "type(value.get('version'))is int and "
                                                         "value['version']==5:\n"
                                                         '        '
                                                         "require(set(value)==set(unobserved_diagnostics())|{'owner_ui_state','capture_failure'})\n"
                                                         '        legacy={k:v for k,v in value.items()if '
                                                         "k!='capture_failure'};legacy['version']=4\n"
                                                         '        finite_diagnostics(legacy)\n'
                                                         "        require(value['failure_class']is not "
                                                         'None)\n'
                                                         '        '
                                                         "capture_failure_diagnostic(value['capture_failure'],value['failure_class'])\n"
                                                         '        return value\n'],
                                                        ["            if self._value['failure_class']is not "
                                                         'None:return\n',
                                                         "            if self._value['failure_class']is not "
                                                         'None:return\n'
                                                         '            self._failure_identity=error\n'],
                                                        ['def project(report, candidate, summary, case, '
                                                         'exit_code, harness_sha):\n',
                                                         '# Failure-only capture observations. No extra '
                                                         'probe/read/retry is performed.\n'
                                                         'def '
                                                         'capture_failure_diagnostic(value,failure_class):\n'
                                                         '    require(type(value)is dict and '
                                                         "set(value)=={'substage','errno','helper_source','helper_line'})\n"
                                                         "    require(type(value['substage'])is str and "
                                                         "value['substage']in "
                                                         "{'started_records','graph_records'})\n"
                                                         "    number=value['errno']\n"
                                                         '    require(number is None or type(number)is int '
                                                         'and 1<=number<=4095 and failure_class in '
                                                         "{'os_error','timeout'})\n"
                                                         '    '
                                                         "source,line=value['helper_source'],value['helper_line']\n"
                                                         '    require((source is None and line is '
                                                         'None)or(type(source)is str and source in '
                                                         "{'factory_base','child_goal_driver'}\n"
                                                         '        and type(line)is int and '
                                                         '1<=line<=1000000))\n'
                                                         '    return value\n'
                                                         '\n'
                                                         '\n'
                                                         'def capture_failure_observation(substage,error):\n'
                                                         '    require(type(substage)is str and substage in '
                                                         "{'started_records','graph_records'})\n"
                                                         '    number=error.errno if '
                                                         'isinstance(error,OSError)else None\n'
                                                         '    number=number if type(number)is int and '
                                                         '1<=number<=4095 else None\n'
                                                         '    # These are exact repository source names, '
                                                         'never an exception-supplied path.\n'
                                                         '    '
                                                         "tests=Path(__file__).absolute().parents[1]/'apps/desktop/tests'\n"
                                                         '    '
                                                         "sources={str(tests/'factory-real-ipc.py'):'factory_base',\n"
                                                         '             '
                                                         "str(tests/'factory-child-goal-real-ipc.py'):'child_goal_driver'}\n"
                                                         '    source,line=None,None\n'
                                                         '    tb=error.__traceback__\n'
                                                         '    for _ in range(4096):\n'
                                                         '        if tb is None:break\n'
                                                         '        '
                                                         'candidate=sources.get(tb.tb_frame.f_code.co_filename)\n'
                                                         '        if candidate is not None and '
                                                         'type(tb.tb_lineno)is int and '
                                                         '1<=tb.tb_lineno<=1000000:\n'
                                                         '            source,line=candidate,tb.tb_lineno\n'
                                                         '        tb=tb.tb_next\n'
                                                         '    if tb is not None:source,line=None,None\n'
                                                         '    return '
                                                         "{'substage':substage,'errno':number,'helper_source':source,'helper_line':line}\n"
                                                         '\n'
                                                         '\n'
                                                         'class '
                                                         'FiniteRuntimeDiagnosticsV5(FiniteRuntimeDiagnosticsV4):\n'
                                                         '    """Successful snapshots stay v4; only the '
                                                         'actual first capture failure extends it."""\n'
                                                         '    def capture_failure(self,substage,error):\n'
                                                         '        try:\n'
                                                         '            '
                                                         'observation=capture_failure_observation(substage,error)\n'
                                                         '            with self._lock:\n'
                                                         '                if not '
                                                         "hasattr(self,'_capture_error'):\n"
                                                         '                    '
                                                         'self._capture_error=error;self._capture_observation=observation\n'
                                                         '        except BaseException:\n'
                                                         '            # Metadata collection cannot replace '
                                                         'the original exception/raise.\n'
                                                         '            return\n'
                                                         '    def snapshot(self):\n'
                                                         '        value=super().snapshot()\n'
                                                         "        if value.get('version')!=4 or "
                                                         "value.get('failure_class')is None:return value\n"
                                                         '        with self._lock:\n'
                                                         '            '
                                                         "capture=getattr(self,'_capture_error',None)\n"
                                                         '            if capture is None or capture is not '
                                                         "getattr(self,'_failure_identity',None):return "
                                                         'value\n'
                                                         '            '
                                                         'observation=dict(self._capture_observation)\n'
                                                         '        '
                                                         'value.update(version=5,capture_failure=observation)\n'
                                                         '        return value\n'
                                                         '\n'
                                                         '\n'
                                                         'def project(report, candidate, summary, case, '
                                                         'exit_code, harness_sha):\n'],
                                                        ['    '
                                                         "diagnostics=finite_diagnostics(report.get('finite_diagnostics'))\n",
                                                         '    '
                                                         "diagnostics=finite_diagnostics(report.get('finite_diagnostics'))\n"
                                                         "    if diagnostics.get('version')==5:\n"
                                                         '        '
                                                         "require(report['status']=='failed_or_interrupted'and "
                                                         'exit_code!=0)\n']]}}


def strip_capture_failure_layer(path,text):
    if type(path)is not str or path not in LAYERS or type(text)is not str:
        raise AssertionError('Unknown capture failure source arguments')
    from tests.infrastructure.schema18_source_fixture import strip_schema18_source
    text=strip_schema18_source(path,text,legacy=LAYERS[path]['legacy'])
    record=LAYERS[path];digest=hashlib.sha256(text.encode()).hexdigest()
    if digest in record['legacy']:
        return text
    if digest!=record['after']:
        raise AssertionError('Unknown complete capture failure source')
    for before,after in record['edits']:
        if text.count(after)!=1:raise AssertionError('Capture failure layer anchor changed')
        text=text.replace(after,before,1)
    if hashlib.sha256(text.encode()).hexdigest()!=record['before']:
        raise AssertionError('Whole original capture source not restored')
    return text


TEST_GUARD_LAYER = {'before': '6b192b0069a833e74ac4e7c1519a2f7169f292f4be923e522c89cb5afda2d5c0',
 'after': '4631a519a858fb75028dda5723da85a2d33a5b4cb26a24e73bc0c3dd62c7af37',
 'edits': [['import types\nimport unittest\n\nROOT=Path(__file__).resolve().parents[2]\n',
            'import types\n'
            'import unittest\n'
            'from tests.infrastructure.child_goal_capture_source_fixture import LAYERS as CAPTURE_LAYERS, '
            'strip_capture_failure_layer\n'
            '\n'
            'ROOT=Path(__file__).resolve().parents[2]\n'],
           ['\n'
            'def inverse(path,text):\n'
            '    if '
            "path=='apps/desktop/src/FactoryChildGoals.tsx':text=strip_plan_read_reuse_outer_layer(text)\n"
            "    expected=LAYERS[path]['original_sha256']\n",
            '\n'
            'def inverse(path,text):\n'
            '    if path in CAPTURE_LAYERS:text=strip_capture_failure_layer(path,text)\n'
            '    if '
            "path=='apps/desktop/src/FactoryChildGoals.tsx':text=strip_plan_read_reuse_outer_layer(text)\n"
            "    expected=LAYERS[path]['original_sha256']\n"],
           ["    if type(text)is not str:raise AssertionError('Unknown Owner UI outer source')\n"
            "    path='apps/desktop/tests/factory-child-goal-real-ipc.py'\n"
            '    digest=source_sha(text)\n'
            '    if digest==ROOT17_V4_SOURCES[path][0]:return inverse(path,text)\n',
            "    if type(text)is not str:raise AssertionError('Unknown Owner UI outer source')\n"
            "    path='apps/desktop/tests/factory-child-goal-real-ipc.py'\n"
            '    text=strip_capture_failure_layer(path,text)\n'
            '    digest=source_sha(text)\n'
            '    if digest==ROOT17_V4_SOURCES[path][0]:return inverse(path,text)\n'],
           ['\n'
            'def parent_v4_source_copy(path,text):\n'
            '    """Reconstruct immutable c675 text for the old positive guard, never exec it.\n'
            '\n',
            '\n'
            'def parent_v4_source_copy(path,text):\n'
            '    if path in CAPTURE_LAYERS:text=strip_capture_failure_layer(path,text)\n'
            '    """Reconstruct immutable c675 text for the old positive guard, never exec it.\n'
            '\n'],
           ['                with self.assertRaises((AssertionError,ValueError)):inverse(path,text)\n'
            "        path='apps/desktop/tests/factory-child-goal-real-ipc.py'\n"
            '        '
            "text=(ROOT/path).read_text().replace('FiniteRuntimeDiagnosticsV4()','FiniteRuntimeDiagnostics()')\n"
            '        with self.assertRaises(AssertionError):inverse(path,text)\n'
            '\n',
            '                with self.assertRaises((AssertionError,ValueError)):inverse(path,text)\n'
            "        path='apps/desktop/tests/factory-child-goal-real-ipc.py'\n"
            '        '
            "text=strip_capture_failure_layer(path,(ROOT/path).read_text()).replace('FiniteRuntimeDiagnosticsV4()','FiniteRuntimeDiagnostics()')\n"
            '        with self.assertRaises(AssertionError):inverse(path,text)\n'
            '\n'],
           ['        for path,(merged,restored)in ROOT17_V4_SOURCES.items():\n'
            '            text=(ROOT/path).read_text()\n'
            '            if '
            "path=='apps/desktop/src/FactoryChildGoals.tsx':text=strip_plan_read_reuse_outer_layer(text)\n"
            '            self.assertEqual(source_sha(text),merged)\n',
            '        for path,(merged,restored)in ROOT17_V4_SOURCES.items():\n'
            '            text=(ROOT/path).read_text()\n'
            '            if path in CAPTURE_LAYERS:text=strip_capture_failure_layer(path,text)\n'
            '            if '
            "path=='apps/desktop/src/FactoryChildGoals.tsx':text=strip_plan_read_reuse_outer_layer(text)\n"
            '            self.assertEqual(source_sha(text),merged)\n'],
           ['    def test_outer_inverse_rejects_unknown_guard_schema_wait_helper_or_missing_endpoint(self):\n'
            "        path='apps/desktop/tests/factory-child-goal-real-ipc.py';text=(ROOT/path).read_text()\n"
            '        mutations=[("snapshot[\'schema\']==17","snapshot[\'schema\']==16"),\n'
            "            ('time.monotonic()+15','time.monotonic()+16'),\n",
            '    def test_outer_inverse_rejects_unknown_guard_schema_wait_helper_or_missing_endpoint(self):\n'
            "        path='apps/desktop/tests/factory-child-goal-real-ipc.py';text=(ROOT/path).read_text()\n"
            '        text=strip_capture_failure_layer(path,text)\n'
            '        mutations=[("snapshot[\'schema\']==17","snapshot[\'schema\']==16"),\n'
            "            ('time.monotonic()+15','time.monotonic()+16'),\n"],
           ['        for path,digest in pins.items():\n'
            '            raw=(ROOT/path).read_bytes()\n'
            '            self.assertEqual(hashlib.sha256(raw).hexdigest(),ROOT17_V4_SOURCES[path][0])\n'
            '            self.assertEqual(source_sha(parent_v4_source_copy(path,raw.decode())),digest)\n',
            '        for path,digest in pins.items():\n'
            '            raw=(ROOT/path).read_bytes()\n'
            '            if path in '
            'CAPTURE_LAYERS:raw=strip_capture_failure_layer(path,raw.decode()).encode()\n'
            '            self.assertEqual(hashlib.sha256(raw).hexdigest(),ROOT17_V4_SOURCES[path][0])\n'
            '            self.assertEqual(source_sha(parent_v4_source_copy(path,raw.decode())),digest)\n']]}

def strip_capture_test_guard_layer(text):
    from tests.infrastructure.schema18_source_fixture import strip_schema18_source
    text=strip_schema18_source('tests/infrastructure/test_child_goal_diagnostics_v4.py',text,legacy=(TEST_GUARD_LAYER['before'],))
    if type(text)is not str:raise AssertionError("Unknown capture test source")
    digest=hashlib.sha256(text.encode()).hexdigest()
    if digest==TEST_GUARD_LAYER["before"]:return text
    if digest!=TEST_GUARD_LAYER["after"]:raise AssertionError("Unknown capture test layer")
    for before,after in reversed(TEST_GUARD_LAYER["edits"]):
        if text.count(after)!=1:raise AssertionError("Missing or duplicate capture test layer")
        text=text.replace(after,before,1)
    if hashlib.sha256(text.encode()).hexdigest()!=TEST_GUARD_LAYER["before"]:raise AssertionError("Changed original test controls")
    return text
