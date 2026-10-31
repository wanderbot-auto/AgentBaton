"""Pure v4 closed metadata, inert collector and full-source inverse controls."""
import ast
import copy
import hashlib
import json
from pathlib import Path
import types
import unittest

ROOT=Path(__file__).resolve().parents[2]
P=types.ModuleType('v4_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')
exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)
SECRET='PRIVATE_CANARY_TOKEN_PATH_ID_TEXT'
LAYERS={'apps/desktop/src/FactoryChildGoals.tsx': {'steps': [(' // Read-only renderer diagnostics, never input to any action or authority check.\n', '\n  <p>{t.separate}', 'bb08ba8e55620d59343951f3ec13d7acd7b45efbd2514a6812b9217dcd45e4e7', ' return <section aria-label={t.title} data-testid="factory-child-goal-panel" aria-busy={busy||loading}>')], 'original_sha256': '7c386d3262f90876f0c72d9b12ff1ba25020a0510a70d18c08c6cf4dce1dc267'}, 'apps/desktop/tests/factory-child-goal-real-ipc.py': {'steps': [('\ndef sample_owner_ui(', '\ndef runtime_driver(', '3f7f121b3cc8c92f2f660cf09cf30744b204753409ae3b29ffbe910ddf40466d', '')], 'original_sha256': '7dc4732f18be492fddcc49bb989f233cd4fc409bb11de33a7951f7d5cf36e338'}, 'scripts/record-child-goal-ci-evidence.py': {'steps': [('    # V4 is an explicit extension; validate its unchanged v3 payload first.\n', '    if value is None:return unobserved_diagnostics()', '21f3e613468e5a7fa6661852663ba165f914f7fe296cea0c08987d2560dce836', ''), ('\n# V4 is renderer-local, untrusted metadata.', '\ndef project(report, candidate, summary, case, exit_code, harness_sha):', '675f9004711f3c1aa4f444fde5dcd411943453ec937c7cf1a9d742935e7a6ac5', '')], 'original_sha256': '5b515e01d220c1b0e9a2759ba3a74cc081e7122a1fa549583aed6a911fd47d69'}}

# Explicit source-proof variants, never executable historical harnesses.
# Root fcc2089 + Parent c675: the same SHA-checked v4 additions strip back to
# complete immutable Root17 files. The original Parent LAYERS/pins stay above.
ROOT17_V4_SOURCES = {
    'apps/desktop/src/FactoryChildGoals.tsx': (
        '57a6899da9526ae6289cafe54509fd2da5cae8146ca748a4640f3b042f7d3281',
        '7c386d3262f90876f0c72d9b12ff1ba25020a0510a70d18c08c6cf4dce1dc267'),
    'apps/desktop/tests/factory-child-goal-real-ipc.py': (
        'dd80aebe8fba1c1a031b10e77bdfba64d8d1d349819521fb2deb36a02435cd9d',
        '9acadb5d494e7cd055b871530867fc273bff9588384beeb9415ffbdaa68c2610'),
    'scripts/record-child-goal-ci-evidence.py': (
        '9907ea6815187bf57de2dd2706f2bc6f4c47456a5ff6c979e896d65b6ba7b768',
        'b51d1c3d4f371c6e1c908e7cd4be308f6c6f6262e2bcccbe1a073df2c545ae14'),
}
PARENT_V4_SHA256 = {
    'apps/desktop/src/FactoryChildGoals.tsx':'57a6899da9526ae6289cafe54509fd2da5cae8146ca748a4640f3b042f7d3281',
    'apps/desktop/tests/factory-child-goal-real-ipc.py':'b959489950486874cf9730b9877dc36ee6c0121688dbc3fd9ef567138279268d',
    'scripts/record-child-goal-ci-evidence.py':'44d55431fad475248094fabd50cab3543241530dee37537276fa140fb0463f1a',
}

def source_sha(text):return hashlib.sha256(text.encode()).hexdigest()

def inverse(path,text):
    expected=LAYERS[path]['original_sha256']
    # Only an exact whole reviewed Root17+v4 input selects the new target.
    # Any unknown change still faces the original Parent full-source check.
    if source_sha(text)==ROOT17_V4_SOURCES[path][0]:expected=ROOT17_V4_SOURCES[path][1]
    for start,end,digest,replacement in LAYERS[path]['steps']:
        if text.count(start)!=1:raise AssertionError('missing/duplicate exact layer')
        index=text.index(start);stop=text.index(end,index)
        layer=text[index:stop]
        if hashlib.sha256(layer.encode()).hexdigest()!=digest:raise AssertionError('unknown layer mutation')
        text=text[:index]+replacement+text[stop:]
    substitutions={
        'apps/desktop/src/FactoryChildGoals.tsx':[(' data-cg-owner-control="review"','')],
        'apps/desktop/tests/factory-child-goal-real-ipc.py':[
            ('FiniteRuntimeDiagnosticsV4()','FiniteRuntimeDiagnostics()'),
            ("            sample_owner_ui(finite,goal_panel,query['goal_run_id'],'begin')\n",''),
            ("                sample_owner_ui(finite,goal_panel,query['goal_run_id'],'end')\n",'')],
        'scripts/record-child-goal-ci-evidence.py':[]}
    for before,after in substitutions[path]:
        if text.count(before)!=1:raise AssertionError('missing/duplicate fixed adaptation')
        text=text.replace(before,after)
    if hashlib.sha256(text.encode()).hexdigest()!=expected:
        raise AssertionError('original whole source changed')
    return text

def strip_owner_ui_v4_outer_layer(text):
    """Exact outer source layer before the unchanged two historical inverses."""
    if type(text)is not str:raise AssertionError('Unknown Owner UI outer source')
    path='apps/desktop/tests/factory-child-goal-real-ipc.py'
    digest=source_sha(text)
    if digest==ROOT17_V4_SOURCES[path][0]:return inverse(path,text)
    # These are exactly the old composed source and old two restored targets.
    # The original inverse still decides which side each target can satisfy.
    if digest in {ROOT17_V4_SOURCES[path][1],
        '4b0332152c2dff9e4d4d7df94dede8ce23b0735c1b76adbef14398c142811b88',
        '7dc4732f18be492fddcc49bb989f233cd4fc409bb11de33a7951f7d5cf36e338'}:return text
    raise AssertionError('Unknown Owner UI outer source')


def parent_v4_source_copy(path,text):
    """Reconstruct immutable c675 text for the old positive guard, never exec it.

    This is a counterfactual SOURCE inverse; live samples remain strict17.
    Whole input/output hashes and exact old composition targets are required.
    """
    if source_sha(text)!=ROOT17_V4_SOURCES[path][0]:raise AssertionError('Unknown composed source')
    if path=='apps/desktop/tests/factory-child-goal-real-ipc.py':
        from tests.infrastructure.test_graph_observation_handshake import strip_composed_outer_layer
        original=text
        text=strip_composed_outer_layer(strip_owner_ui_v4_outer_layer(text),'theirs')
        start,end,digest,_=LAYERS[path]['steps'][0]
        helper=original[original.index(start):original.index(end,original.index(start))]
        if source_sha(helper)!=digest:raise AssertionError('Unknown v4 helper')
        index=text.index(end);text=text[:index]+helper+text[index:]
        edits=[('FiniteRuntimeDiagnostics()','FiniteRuntimeDiagnosticsV4()'),
            ("            finite.owner_refresh_begin(bound_child_goal_query(query,bounds),time.monotonic_ns())\n",
             "            finite.owner_refresh_begin(bound_child_goal_query(query,bounds),time.monotonic_ns())\n            sample_owner_ui(finite,goal_panel,query['goal_run_id'],'begin')\n"),
            ("                finite.owner_refresh_end(time.monotonic_ns())\n",
             "                sample_owner_ui(finite,goal_panel,query['goal_run_id'],'end')\n                finite.owner_refresh_end(time.monotonic_ns())\n")]
    elif path=='scripts/record-child-goal-ci-evidence.py':
        edits=[("m['schema']==17 and", "m['schema']==16 and"),
            ("summary={'schema':17,", "summary={'schema':16,"),
            ("require(value['failure_class']in ({'assertion','refusal'}if value['version']==3 else {'assertion'}))",
             "require(value['failure_class']=='assertion')"),
            ("    def graph_argv(self,value):\n        with self._lock:\n            if self._value['graph_provider_argv']is None:\n                self._value['graph_provider_argv']=dict(graph_argv_diagnostic(value))\n",'')]
    else:edits=[]
    for before,after in edits:
        if text.count(before)!=1:raise AssertionError('Exact Parent source anchor changed')
        text=text.replace(before,after)
    if source_sha(text)!=PARENT_V4_SHA256[path]:raise AssertionError('Exact Parent v4 source changed')
    return text

def raw_sample():
    return {'epoch':'17',**{key:'false'for key in P.OWNER_UI_FLAGS},
        'connected':'true','has_view':'true','pending_kind':'none','pending_unknown':'none',
        'owner_present':True,'owner_disabled':False}

def recorder():
    r=P.FiniteRuntimeDiagnosticsV4();r.owner_refresh_begin({'scope_id':SECRET,'task_id':'task','goal_run_id':'goal'},100)
    return r

def collector():
    path=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
    node=next(n for n in ast.parse(path.read_text()).body if isinstance(n,ast.FunctionDef)and n.name=='sample_owner_ui')
    env={'time':types.SimpleNamespace(monotonic_ns=lambda:110)}
    exec(compile(ast.Module(body=[node],type_ignores=[]),str(path),'exec'),env)
    return env['sample_owner_ui']

class V4DiagnosticsTests(unittest.TestCase):
    def test_full_source_inverse_preserves_all_original_ui_harness_projector_bytes(self):
        for path in LAYERS:inverse(path,(ROOT/path).read_text())

    def test_unknown_code_change_missing_optin_marker_and_extra_flag_cannot_hide_in_inverse(self):
        for path in LAYERS:
            original=(ROOT/path).read_text()
            for text in (original+'\n# extra canary\n',original.replace('owner-ui-v4','owner-ui-foreign')):
                if text==original:continue
                with self.assertRaises((AssertionError,ValueError)):inverse(path,text)
        path='apps/desktop/tests/factory-child-goal-real-ipc.py'
        text=(ROOT/path).read_text().replace('FiniteRuntimeDiagnosticsV4()','FiniteRuntimeDiagnostics()')
        with self.assertRaises(AssertionError):inverse(path,text)

    def test_v3_default_and_v1_v2_semantics_are_unchanged(self):
        old=P.FiniteRuntimeDiagnostics().snapshot();self.assertEqual(old,P.unobserved_diagnostics());self.assertEqual(old['version'],3)
        for version in (1,2):
            value=copy.deepcopy(old);value['version']=version
            value.pop('owner_refresh');value.pop('graph_provider_identity')
            if version==1:value.pop('graph_provider_argv')
            self.assertEqual(P.finite_diagnostics(value),value)
        old['owner_refresh']['full_fresh']=True
        with self.assertRaises(P.Refused):P.finite_diagnostics(old)

    def test_empty_v4_is_explicitly_unobserved_and_not_authority(self):
        value=P.finite_diagnostics(P.FiniteRuntimeDiagnosticsV4().snapshot())
        self.assertEqual(value['version'],4);self.assertEqual(value['owner_ui_state'],P.unobserved_owner_ui_state())
        self.assertEqual(value['owner_refresh'],P.unobserved_owner_refresh())

    def test_observed_false_values_are_preserved_not_coerced_to_null(self):
        r=recorder();r.owner_ui_sample('begin',raw_sample(),101);r.owner_ui_sample('end',raw_sample(),109);r.owner_refresh_end(110)
        safe=P.finite_diagnostics(r.snapshot());sample=safe['owner_ui_state']['end']
        self.assertEqual(sample['epoch'],17);self.assertIs(sample['fresh'],False);self.assertIs(sample['loading'],False)
        self.assertIs(sample['owner_disabled'],False);self.assertEqual(sample['pending_kind'],'none');self.assertIsNone(sample['pending_unknown'])
        self.assertIsNone(safe['owner_refresh']['source_epoch']);self.assertIsNone(safe['owner_refresh']['full_fresh'])
        self.assertEqual(safe['owner_refresh']['pending_kind'],'unobserved');self.assertNotIn(SECRET,json.dumps(safe))

    def test_owner_absence_is_distinct_from_disabled_and_missing_component(self):
        raw=raw_sample();raw.update(owner_present=False,owner_disabled=None)
        sample=P.owner_ui_dom_sample(raw,101);self.assertIs(sample['owner_present'],False);self.assertIsNone(sample['owner_disabled'])
        self.assertEqual(P.owner_ui_dom_sample(None,101),P.unobserved_owner_ui_sample())
        raw['owner_disabled']=False
        self.assertEqual(P.owner_ui_dom_sample(raw,101),P.unobserved_owner_ui_sample())

    def test_every_pending_kind_and_exact_unknown_boolean(self):
        for kind in P.OWNER_UI_PENDING-{'none'}:
            for unknown in ('true','false'):
                raw=raw_sample();raw.update(pending_kind=kind,pending_unknown=unknown)
                sample=P.owner_ui_dom_sample(raw,101)
                self.assertEqual(sample['pending_kind'],kind);self.assertIs(sample['pending_unknown'],unknown=='true')

    def test_missing_extra_wrong_type_unknown_and_oversized_dom_are_wholly_unobserved(self):
        mutations=[lambda r:r.update(raw=SECRET),lambda r:r.pop('fresh'),lambda r:r.update(epoch='01'),lambda r:r.update(epoch='9007199254740992'),
            lambda r:r.update(epoch=True),lambda r:r.update(epoch='1.0'),lambda r:r.update(fresh=False),lambda r:r.update(fresh='FALSE'),
            lambda r:r.update(pending_kind=SECRET),lambda r:r.update(pending_unknown='true'),lambda r:r.update(owner_present=1),
            lambda r:r.update(owner_disabled=0),lambda r:r.update(loading=SECRET*10),lambda r:r.update(has_view='false')]
        for mutation in mutations:
            raw=raw_sample();mutation(raw)
            self.assertEqual(P.owner_ui_dom_sample(raw,101),P.unobserved_owner_ui_sample())

    def test_bad_timestamps_are_not_observed(self):
        for stamp in (True,0,-1,2**63,1.0,SECRET,None):
            self.assertEqual(P.owner_ui_dom_sample(raw_sample(),stamp),P.unobserved_owner_ui_sample())

    def test_duplicate_endpoint_is_permanently_unobserved(self):
        r=recorder()
        for stamp in (101,102,103):r.owner_ui_sample('begin',raw_sample(),stamp)
        self.assertEqual(P.finite_diagnostics(r.snapshot())['owner_ui_state']['begin'],P.unobserved_owner_ui_sample())

    def test_public_field_forgery_unknown_and_false_to_null_are_rejected(self):
        r=recorder();r.owner_ui_sample('begin',raw_sample(),101);r.owner_refresh_end(110);original=r.snapshot()
        mutations=[lambda d:d.update(raw=SECRET),lambda d:d['owner_ui_state'].update(authority='owner'),lambda d:d['owner_ui_state'].update(source=SECRET),
            lambda d:d['owner_ui_state']['begin'].update(fresh=None),lambda d:d['owner_ui_state']['begin'].update(owner_disabled=0),
            lambda d:d['owner_ui_state']['begin'].update(epoch=True),lambda d:d['owner_ui_state']['begin'].update(sampled_ns=True),
            lambda d:d['owner_ui_state']['begin'].update(raw=SECRET),lambda d:d['owner_ui_state']['end'].update(fresh=False)]
        for mutation in mutations:
            bad=copy.deepcopy(original);mutation(bad)
            with self.assertRaises(P.Refused)as error:P.finite_diagnostics(bad)
            self.assertNotIn(SECRET,str(error.exception))

    def test_samples_require_original_window_and_order_but_never_claim_click_causality(self):
        for begin,end in ((99,105),(101,111),(109,102)):
            r=recorder();r.owner_ui_sample('begin',raw_sample(),begin);r.owner_ui_sample('end',raw_sample(),end);r.owner_refresh_end(110)
            with self.assertRaises(P.Refused):P.finite_diagnostics(r.snapshot())
        r=P.FiniteRuntimeDiagnosticsV4();r.owner_ui_sample('begin',raw_sample(),101)
        with self.assertRaises(P.Refused):P.finite_diagnostics(r.snapshot())

    def test_collector_is_single_inert_eval_and_exceptions_are_only_unobserved(self):
        class Panel:
            calls=[]
            def evaluate_all(self,source,goal):self.calls.append((source,goal));return raw_sample()
        panel=Panel();r=recorder();collector()(r,panel,SECRET,'begin')
        self.assertEqual(len(panel.calls),1);self.assertEqual(panel.calls[0][1],SECRET)
        script=panel.calls[0][0]
        for guard in ('nodes.length!==1','histories.length!==1',"getAttribute('data-run-id')!==expectedGoal",'marked.length>1','marked.length!==captioned.length','value.length>32'):
            self.assertIn(guard,script)
        for forbidden in ('fetch(','.click(','.setAttribute(','.wait_for','setTimeout','setInterval'):
            self.assertNotIn(forbidden,script)
        self.assertNotIn(SECRET,json.dumps(P.finite_diagnostics(r.snapshot())))
        class Missing:
            def evaluate_all(self,*args):raise RuntimeError(SECRET)
        r=recorder();collector()(r,Missing(),SECRET,'begin')
        self.assertEqual(r.snapshot()['owner_ui_state']['begin'],P.unobserved_owner_ui_sample())

    def test_original_pass_conditions_still_deny_failed_cleanup_and_failure(self):
        # Use exact existing pure fixture. No Core/SQL/browser is imported/run.
        path=ROOT/'tests/infrastructure/test_child_goal_ci_evidence.py'
        old=types.ModuleType('v4_existing_fixture');old.__file__=str(path)
        exec(compile(path.read_bytes(),str(path),'exec'),old.__dict__)
        _,candidate,summary,report=old.inputs();report['finite_diagnostics']=P.FiniteRuntimeDiagnosticsV4().snapshot()
        self.assertEqual(P.project(report,candidate,summary,'normal',0,'e'*64)['status'],'passed')
        for mutation in (lambda v:v.update(cleanup_complete=False),lambda v:v.update(failure={'private':SECRET}),lambda v:v.update(checks=[])):
            bad=copy.deepcopy(report);mutation(bad)
            self.assertEqual(P.project(bad,candidate,summary,'normal',0,'e'*64)['status'],'failed')

    def test_both_exact_reviewed_source_variants_keep_original_parent_and_root17_targets(self):
        for path,(merged,restored)in ROOT17_V4_SOURCES.items():
            text=(ROOT/path).read_text();self.assertEqual(source_sha(text),merged)
            self.assertEqual(source_sha(inverse(path,text)),restored)
            parent=parent_v4_source_copy(path,text)
            self.assertEqual(source_sha(parent),PARENT_V4_SHA256[path])
            self.assertEqual(source_sha(inverse(path,parent)),LAYERS[path]['original_sha256'])
            anchor=LAYERS[path]['steps'][0][0]
            for changed in (parent+'\n# unreviewed Parent source\n',parent.replace(anchor,anchor+' ',1)):
                with self.assertRaises(AssertionError):inverse(path,changed)

    def test_outer_inverse_rejects_unknown_guard_schema_wait_helper_or_missing_endpoint(self):
        path='apps/desktop/tests/factory-child-goal-real-ipc.py';text=(ROOT/path).read_text()
        mutations=[("snapshot['schema']==17","snapshot['schema']==16"),
            ('time.monotonic()+15','time.monotonic()+16'),
            ("require(result['kind'] != 'not_verified'", "require(result['kind'] == 'not_verified'"),
            ('nodes.length!==1','nodes.length>1'),
            ('FiniteRuntimeDiagnosticsV4()','FiniteRuntimeDiagnostics()'),
            ("            sample_owner_ui(finite,goal_panel,query['goal_run_id'],'begin')\n",''),
            ("                sample_owner_ui(finite,goal_panel,query['goal_run_id'],'end')\n",'')]
        for before,after in mutations:
            self.assertIn(before,text)
            changed=text.replace(before,after,1)
            with self.assertRaises(AssertionError):strip_owner_ui_v4_outer_layer(changed)
            with self.assertRaises(AssertionError):inverse(path,changed)
        begin="            sample_owner_ui(finite,goal_panel,query['goal_run_id'],'begin')\n"
        end="                sample_owner_ui(finite,goal_panel,query['goal_run_id'],'end')\n"
        for changed in (text+'\n# outside unreviewed\n',text.replace(begin,begin*2,1),text.replace(begin,'__ENDPOINT_SWAP__',1).replace(end,begin,1).replace('__ENDPOINT_SWAP__',end,1)):
            with self.assertRaises(AssertionError):strip_owner_ui_v4_outer_layer(changed)
        with self.assertRaises(AssertionError):inverse(path,strip_owner_ui_v4_outer_layer(text))

    def test_root17_samples_and_v3_refusal_semantics_survive_v4_only_metadata(self):
        path=ROOT/'tests/infrastructure/test_child_goal_ci_evidence.py'
        old=types.ModuleType('v4_root17_fixture');old.__file__=str(path)
        exec(compile(path.read_bytes(),str(path),'exec'),old.__dict__)
        raw,candidate,summary,report=old.inputs()
        self.assertIs(type(candidate['schema']),int);self.assertEqual(candidate['schema'],17)
        P.candidate_summary(raw)
        for bad_schema in (15,16,18,17.0,True):
            bad=copy.deepcopy(candidate);bad['schema']=bad_schema
            with self.assertRaises(P.Refused):P.candidate_summary(json.dumps(bad).encode())
        r=P.FiniteRuntimeDiagnosticsV4();r.graph_argv({'reason':'empty','previously_bound':True})
        r.failure(P.Refused('fixed inert source control'),'runtime',__file__,P.Refused)
        safe=P.finite_diagnostics(r.snapshot())
        self.assertEqual(safe['failure_class'],'refusal');self.assertEqual(safe['version'],4)
        self.assertEqual(safe['graph_provider_argv'],{'reason':'empty','previously_bound':True})
        self.assertEqual(safe['owner_ui_state'],P.unobserved_owner_ui_state())
        report['finite_diagnostics']=safe
        self.assertEqual(P.project(report,candidate,summary,'normal',0,'e'*64)['status'],'failed')

if __name__=='__main__':unittest.main()
