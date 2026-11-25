"""Exact test-only inverse of the Child Goal schema 19 outer layer.

The prior schema 18 sources are those at b8573a221e152c6f47d8590739f031a7bbbd9058.
Verify the whole input and each reversed span before accepting the pinned result.
"""
import hashlib
import json
from pathlib import Path
from types import ModuleType

PROJECTOR = 'scripts/record-child-goal-ci-evidence.py'

LAYERS = {'apps/desktop/tests/factory-child-goal-real-ipc.py': ('33bc3a72c30e509ae64cfce573743f58aa4406e4632ac42e3354d125f7ac242c',
                                                       '706c588d4d28d66ab4b5ee47b96df68ffe458a5eb37beacbab51ad2f739b8a1d',
                                                       [(0,
                                                         9,
                                                         '4104a813b56402af3b2c2d7bfa720ada048a6f08a15bec4ed35e7667ac7ea911',
                                                         '#!/usr/bin/env python3\n'
                                                         '"""Schema18 real child-Goal browser/Unix IPC harness, '
                                                         'not native/Provider proof.\n'
                                                         '\n'
                                                         'The five Owner calls preserve the frozen '
                                                         'camelCase/snake_case wire contract.\n'
                                                         'Default and --self-check start no listener, daemon, '
                                                         'browser or subprocess.\n'
                                                         'Explicit --run requires Root runtime readiness and '
                                                         'exclusive execution lease;\n'
                                                         'all successful business receipts then come from the '
                                                         'supplied real Core18.\n'
                                                         'Private Owner simulation is test-only and never an '
                                                         'actual OS confirmation.\n'
                                                         '"""\n'),
                                                        (32,
                                                         49,
                                                         '599f7104ab83342b1576ca9109eb1f20d18e944029649626fad4355cf497ec62',
                                                         '\n'
                                                         'PLANNING_PATH = '
                                                         "Path(__file__).with_name('factory-child-planning-real-ipc.py')\n"
                                                         '_spec = '
                                                         "importlib.util.spec_from_file_location('_child_goal_planning_support', "
                                                         'PLANNING_PATH)\n'
                                                         'planning = importlib.util.module_from_spec(_spec)\n'
                                                         'sys.modules[_spec.name] = planning\n'
                                                         '_spec.loader.exec_module(planning)\n'
                                                         'base, support = planning.base, planning.base.support\n'
                                                         'require, Refusal, FixedCall = planning.require, '
                                                         'planning.Refusal, planning.FixedCall\n'
                                                         "_diag_spec=importlib.util.spec_from_file_location('_child_goal_finite_diagnostics',Path(__file__).resolve().parents[3]/'scripts/record-child-goal-ci-evidence.py')\n"
                                                         'finite_projection=importlib.util.module_from_spec(_diag_spec);_diag_spec.loader.exec_module(finite_projection)\n'
                                                         '# This private support instance only: execution scopes '
                                                         'add exactly these fields.\n'
                                                         'base.REQUEST_FIELDS = base.REQUEST_FIELDS | '
                                                         "{'factory_run_revision', "
                                                         "'child_goal_execution_binding'}\n"),
                                                        (946,
                                                         976,
                                                         '32431026c1019c4842e1a174f088fdc497ea7ab3cc6049700a9068480b21e96b',
                                                         '\n'
                                                         '\n'
                                                         'def database_snapshot(runtime):\n'
                                                         '    """Actual readonly SQL observations, never '
                                                         'synthesized Store receipts."""\n'
                                                         '    with '
                                                         "sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as "
                                                         'db:\n'
                                                         "        db.execute('PRAGMA "
                                                         "query_only=ON');db.execute('BEGIN')\n"
                                                         '        counts=support.current_factory_counts(db)\n'
                                                         '        '
                                                         "tables=set(support.FACTORY_RECORD_TABLES.values())|set(support.INTEGRATION_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|{'events','attempts','sessions','dispatch_intents','workspace_leases','budgets','execution_grants'}\n"
                                                         '        result={};total=0\n'
                                                         '        for table in sorted(tables):\n'
                                                         "            rows=db.execute('SELECT * FROM "
                                                         '"\'+table+\'" ORDER BY rowid\').fetchall();assert '
                                                         'len(rows)<=2048\n'
                                                         '            '
                                                         "raw=json.dumps(rows,separators=(',',':'),allow_nan=False).encode();total+=len(raw);assert "
                                                         'total<=16*1024*1024\n'
                                                         '            '
                                                         "result[table]={'count':len(rows),'sha256':base.sha(raw)}\n"
                                                         "        sequence=db.execute('SELECT "
                                                         "COALESCE(max(sequence),0)FROM events').fetchone()[0]\n"
                                                         '        active_leases=db.execute("SELECT count(*)FROM '
                                                         'workspace_leases WHERE holder!=\'\'").fetchone()[0]\n'
                                                         '        active_intents=db.execute("SELECT count(*)FROM '
                                                         'dispatch_intents WHERE '
                                                         'state!=\'stopped\'").fetchone()[0];db.rollback()\n'
                                                         '    return '
                                                         "{'schema':18,'factory_counts':counts,'tables':result,'sequence':sequence,'active_workspace_leases':active_leases,'active_dispatch_intents':active_intents}\n"
                                                         '\n'
                                                         '\n'),
                                                        (1274,
                                                         1288,
                                                         '735a854b05c3a96ceaad17d0693b80d3c4843648f3ca6b6689ec6fa465a46e61',
                                                         '    require(type(snapshot)is dict and '
                                                         "set(snapshot)=={'schema','factory_counts','tables','sequence','active_workspace_leases','active_dispatch_intents'})\n"
                                                         "    require(type(snapshot['schema'])is int and "
                                                         "snapshot['schema']==18)\n"
                                                         "    require(type(snapshot['factory_counts'])is dict and "
                                                         "set(snapshot['factory_counts'])==set(support.FACTORY_RECORD_TABLES))\n"
                                                         "    require(type(snapshot['tables'])is dict and "
                                                         "set(snapshot['tables'])==PREAPPROVAL_ZERO_TABLES|PREAPPROVAL_COUNTS.keys()|{'events'})\n"),
                                                        (1692,
                                                         1731,
                                                         '63247f5fbddddc0ee0252757badaa4d217859f0f085dca120a8918cec1d9adae',
                                                         '\n'
                                                         '\n'
                                                         'def runtime_driver(args, output):\n'
                                                         '    """Real built UI / authenticated Core15 / generated '
                                                         'processes, never a mock.\n'
                                                         '\n'
                                                         '    Each --case owns fresh resources. Private Owner '
                                                         'simulation is outside HTTP\n'
                                                         '    and checks the complete exact server proposal; no '
                                                         'real OS consent is claimed.\n'
                                                         '    """\n'
                                                         '    from playwright.sync_api import expect, '
                                                         'sync_playwright, TimeoutError as '
                                                         'PlaywrightTimeoutError\n'
                                                         '    '
                                                         "raw_daemon=(args.tools/'batond').resolve(strict=True);raw_cli=(args.tools/'baton').resolve(strict=True)\n"
                                                         '    '
                                                         'frontend=args.frontend.resolve(strict=True);manifest_raw=args.manifest.resolve(strict=True).read_bytes()\n'
                                                         '    '
                                                         'manifest=json.loads(manifest_raw,object_pairs_hook=planning.unique_object)\n'
                                                         "    require(type(manifest['schema'])is int and "
                                                         "manifest['schema']==18)\n"
                                                         '    '
                                                         "require(all(re.fullmatch('[a-f0-9]{40}',manifest[k])for "
                                                         "k in ['source_head','compiled_core_source_head']))\n"
                                                         '    '
                                                         "require(manifest['binaries']['batond']==base.sha(raw_daemon.read_bytes()) "
                                                         'and '
                                                         "manifest['binaries']['baton']==base.sha(raw_cli.read_bytes()))\n"
                                                         '    '
                                                         "files=list(frontend.rglob('*'));require(len(files)<=4096 "
                                                         'and not any(p.is_symlink()for p in files))\n'
                                                         '    require(sum(p.stat().st_size for p in files if '
                                                         'p.is_file())<=64*1024*1024 and '
                                                         "(frontend/'index.html').is_file())\n"
                                                         '    '
                                                         'assets={str(p.relative_to(frontend)):base.sha(p.read_bytes())for '
                                                         'p in files if p.is_file()}\n'
                                                         "    require(assets==manifest['frontend_assets'])\n"
                                                         '    '
                                                         'capacity=os.statvfs(output);free=capacity.f_bavail*capacity.f_frsize\n'
                                                         '    '
                                                         "require(free>=200*1024*1024+raw_daemon.stat().st_size+raw_cli.stat().st_size+32*1024*1024,'Keep200MiB "
                                                         'guard plus exact private tools and bounded evidence '
                                                         "allowance')\n"
                                                         '    '
                                                         'source=Path(__file__).read_bytes();old_support={str(p):base.sha(p.read_bytes())for '
                                                         'p in '
                                                         '[PLANNING_PATH,planning.BASE_PATH,base.SUPPORT_PATH]}\n'
                                                         '    '
                                                         "report={'status':'failed_or_interrupted','dimension':'actual "
                                                         'built React/Linux Chromium/fixed test-only '
                                                         'adapter/authenticated Unix Core15/generated '
                                                         "account-free ACP/MCP7',\n"
                                                         '        '
                                                         "'case':args.case,'capacity_preflight':{'free_bytes':free,'guard_bytes':200*1024*1024,'private_tool_copy_bytes':raw_daemon.stat().st_size+raw_cli.stat().st_size,'evidence_allowance_bytes':32*1024*1024},'candidate':manifest,'manifest_sha256':base.sha(manifest_raw),'harness_sha256':base.sha(source),\n"
                                                         '        '
                                                         "'support_sha256':old_support,'frontend_assets':assets,'checks':[],'http':[],'faults':[],'private_owner_reviews':[],\n"
                                                         '        '
                                                         "'screenshots':[],'observations':{},'goal_owner_binding':new_goal_owner_binding(),'cleanup':[],'http_unexpected_errors':[],'readonly_aborts':[],'cleanup_complete':False,\n"),
                                                        (1734,
                                                         1739,
                                                         '0ac14d077ffb0c3a02cb78997d96c7c5257aaf735f01f26a2e51d03b9651cfd7',
                                                         '    '
                                                         "root=Path(tempfile.mkdtemp(prefix='baton-child-goal-browser-')).resolve();root.chmod(0o700)\n"
                                                         "    runtime=root/'runtime';runtime.mkdir(mode=0o700)\n"
                                                         '    '
                                                         'daemon=server=http_thread=browser=playwright=log=reaper=host=workspace=scope=None\n'
                                                         '    '
                                                         "token='';pump=lambda:None;active={};lock=threading.Lock();verified=[];graph_verified=[];record_context={};resources={};bounds=None\n"
                                                         "    def clean(v):return str(v).replace(token,'[redacted "
                                                         "generated Owner token]')if token else str(v)\n"),
                                                        (1773,
                                                         1779,
                                                         '899a1a4ff1f765bed963774b28321c0b1a06b9b4f8bf9ac9a2dbcfc857f1213e',
                                                         "        report['private_installation']={}\n"
                                                         '        '
                                                         "binary,companion=base.install_private_tools(raw_daemon,raw_cli,root/'installed',report['private_installation'])\n"
                                                         '        '
                                                         "provider=root/'generated-child-goal-acp';settings=root/'generated-settings.json'\n"
                                                         '        '
                                                         "provider.write_text('#!'+sys.executable+'\\n'+generated_acp());provider.chmod(0o700)\n"),
                                                        (1803,
                                                         1809,
                                                         '0bb0670ca9d287972135b28acf00ce1729c1727c29d3995d9e748169e49e2489',
                                                         '                '
                                                         "finally:timing['finished']=time.monotonic_ns()\n"
                                                         '        host=ObservedHost(runtime);token=host.token\n'
                                                         '        def approve(method,params,kind,key):\n'
                                                         '            '
                                                         "proposal=host.result(method,params,key);require(proposal['kind']==kind)\n"),
                                                        (2152,
                                                         2157,
                                                         '9f52e32400285245261c88de8b7c351844b9b090dde6b866a347310130ac1e61',
                                                         '        '
                                                         "snapshot=database_snapshot(runtime);report['observations']['final_snapshot']=snapshot\n"
                                                         '        '
                                                         "require(snapshot['factory_counts']['child_goal_runs']==1 "
                                                         'and '
                                                         "snapshot['factory_counts']['child_plan_heads']==snapshot['factory_counts']['child_plan_revisions']==1)\n"
                                                         '        with '
                                                         "sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as "
                                                         'db:\n'
                                                         "            db.execute('PRAGMA "
                                                         "query_only=ON');db.execute('BEGIN')\n"
                                                         '            '
                                                         "require(support.exact_json_value(json.loads(db.execute('SELECT "
                                                         'document FROM execution_grants WHERE '
                                                         "id=?',[grant['id']]).fetchone()[0]),grant))\n"
                                                         '            # The same Grant explicitly changed0->3 '
                                                         'only at the separate Native\n'),
                                                        (2201,
                                                         2206,
                                                         '4aa170c1a965215b65ffe6f4d880ff05bcbda40fce6c236008916b02110a9c00',
                                                         '            def diagnostics():\n'
                                                         '                '
                                                         "info,_=base.diagnostic_snapshot(runtime,workspace,output,token);report['observations']['final_diagnostics']=info\n"
                                                         "                require(info['complete']is True and "
                                                         "info['schema']==18)\n"
                                                         '                if scope is not '
                                                         "None:report['observations']['last_actual_SQL_snapshot']=database_snapshot(runtime)\n"
                                                         '            '
                                                         "base.cleanup_call(report,'readonly_failure_diagnostics',diagnostics,clean)\n"),
                                                        (2211,
                                                         2228,
                                                         '43987ca84aee9e495ac6797b845e2542dabe2f52914b414c866e01b69ff6c464',
                                                         '                capture()\n'
                                                         '            '
                                                         "base.cleanup_call(report,'owned_scope_cleanup_request',cancel_remaining,clean)\n"
                                                         '        '
                                                         'planning.close_browser_engine(browser,playwright,report,clean)\n'
                                                         '        http_complete=True\n'
                                                         '        if server is not None:\n'
                                                         '            '
                                                         "base.cleanup_call(report,'HTTP_shutdown',server.shutdown,clean);http_thread.join(timeout=5)\n"
                                                         '            http_complete=not '
                                                         "http_thread.is_alive();base.cleanup_call(report,'HTTP_handlers_close',server.server_close,clean);http_complete=http_complete "
                                                         'and not active\n'
                                                         '            '
                                                         "report['cleanup'].append({'action':'HTTP_serve_and_handler_join','confirmed':http_complete,'active':dict(active)})\n"
                                                         '            if not '
                                                         "http_complete:fail('HTTP_join',AssertionError('Owned "
                                                         "HTTP threads remain'))\n"),
                                                        (2285,
                                                         2305,
                                                         'f289378008cb86855965ae592e80fcd4ed33ee6d20eb0f475c12c2e9687b911e',
                                                         '            '
                                                         "database_retained=base.cleanup_call(report,'private_raw_DB_after_owned_join',preserve_database,clean)is "
                                                         'True\n'
                                                         '        if log is not None:\n'
                                                         "            log.close();p=output/'daemon.log'\n"
                                                         '            if '
                                                         "token:p.write_bytes(p.read_bytes().replace(token.encode(),b'[redacted "
                                                         "generated Owner token]'))\n"
                                                         '        if reaper is not '
                                                         "None:base.cleanup_call(report,'controller_subreaper_restore',reaper.restore,clean)\n"
                                                         '        def unchanged_inputs():\n'
                                                         '            require(Path(__file__).read_bytes()==source '
                                                         'and args.manifest.read_bytes()==manifest_raw)\n'
                                                         '            '
                                                         'require(all(base.sha(Path(p).read_bytes())==digest for '
                                                         'p,digest in old_support.items()))\n'
                                                         '            '
                                                         "require(base.sha(raw_daemon.read_bytes())==manifest['binaries']['batond']and "
                                                         "base.sha(raw_cli.read_bytes())==manifest['binaries']['baton'])\n"
                                                         '            '
                                                         'require({str(p.relative_to(frontend)):base.sha(p.read_bytes())for '
                                                         "p in frontend.rglob('*')if p.is_file()}==assets)\n"
                                                         '        '
                                                         "base.cleanup_call(report,'immutable_inputs_after',unchanged_inputs,clean)\n"
                                                         '        # Reconcile every persisted session to a '
                                                         'previously Core-bound process\n'
                                                         '        # record; an empty glob or daemon exit alone '
                                                         'proves no descendant gone.\n'),
                                                        (2306,
                                                         2311,
                                                         'f2cdc667ac43df6ed91d36ed06b9c3078a2f78cb56a7ea0bd45bffae46c0e0d2',
                                                         '        if daemon_complete:\n'
                                                         '            def confirm_session_coverage():\n'
                                                         '                with '
                                                         "sqlite3.connect((runtime/'baton.sqlite3').as_uri()+'?mode=ro',uri=True)as "
                                                         'db:\n'
                                                         "                    db.execute('PRAGMA "
                                                         "query_only=ON');db.execute('BEGIN')\n"
                                                         '                    sessions=[json.loads(raw)for raw, '
                                                         "in db.execute('SELECT document FROM sessions')]\n"
                                                         '                    connections=[json.loads(raw)for '
                                                         "raw, in db.execute('SELECT document FROM "
                                                         "connections')]\n"),
                                                        (2321,
                                                         2344,
                                                         'c38b772732641ca641c9df1dd71ea1a180a46d5b4d00e63681eb10fa11de6eaf',
                                                         '            '
                                                         "'http_complete':http_complete,'graph_complete':graph_gone,'raw_database_retained':database_retained,\n"
                                                         '            '
                                                         "'session_coverage_confirmed':session_coverage,'business_passed':report['status']=='passed'and "
                                                         "not report.get('failure')}\n"
                                                         '        if fixture_cleanup_permitted(**states):\n'
                                                         '            '
                                                         "base.cleanup_call(report,'private_fixture_directory_cleanup_after_all_confirmed',lambda:shutil.rmtree(root),clean)\n"
                                                         '        else:\n'
                                                         '            '
                                                         "retention={'root':str(root),'runtime':str(runtime),'source':str(root/'source'),\n"
                                                         '                '
                                                         "'root_mode':oct(root.stat().st_mode&0o777),'cleanup_gate':states,\n"
                                                         '                '
                                                         "'bound_factory_records':verified,'bound_graph_records':graph_verified,\n"
                                                         '                '
                                                         "'automatic_finalizer':False,'reason':'Retain original "
                                                         'private files/DB/PID evidence on failure or any '
                                                         "unconfirmed stop; no clean-stop inference'}\n"
                                                         '            '
                                                         "report['observations']['retained_private_fixture']=retention\n"
                                                         '            '
                                                         "base.write_private_json(output/'retained-private-fixture.json',retention)\n"
                                                         '        '
                                                         'planning.seal_runtime_report(report,daemon_complete,identities_complete '
                                                         'and graph_gone,http_complete,clean)\n'
                                                         "        report['finite_diagnostics']=finite.snapshot()\n"
                                                         '        '
                                                         "base.write_private_json(output/'report.json',json.loads(clean(json.dumps(report))))\n"),
                                                        (2362,
                                                         2378,
                                                         '4910cc00b2ac7ac6372333e5fcf1c1f66b2b00dd777fca98a2e978cb26f581f9',
                                                         '        '
                                                         "base.write_private_json(out/'wire-recording-check.json',value);print(json.dumps({'status':value['status'],'count':value['count'],'threads':value['threads'],'parallel_files':value['parallel_files'],'actual_runtime':value['actual_runtime']}));return "
                                                         '0\n'
                                                         '    if args.run:\n'
                                                         '        require(args.tools is not None and '
                                                         'args.manifest is not None and args.frontend is not '
                                                         'None)\n'
                                                         '        try:return runtime_driver(args,out)\n'
                                                         '        except BaseException as error:\n'
                                                         '            if not '
                                                         "(out/'report.json').exists():base.write_private_json(out/'preflight-refusal.json',{'status':'preflight_refused','actual_runtime':'not_run','error':str(error),'traceback':traceback.format_exc()})\n"
                                                         '            raise\n'
                                                         '    value=self_check() if args.self_check else '
                                                         "{'status':'pending_dependencies','actual_runtime':'not_run',\n"
                                                         '        '
                                                         "'runtime_driver':'source_implemented_not_run','implemented_cases':['normal','lost-start','cancel'],'matrix':MATRIX,'pending':[\n"
                                                         "            'Core18 exact lifecycle "
                                                         'success/failure/stop-chain fixtures and independent '
                                                         "review',\n"
                                                         "            'Fresh immutable Core18 tools/source "
                                                         "manifest and composed frontend assets',\n"
                                                         "            'Independent review of real browser driver; "
                                                         "Root exclusive browser/daemon lease',\n"
                                                         "            'Additional "
                                                         'permission/budget/unknown/adoption/recovery matrix '
                                                         "beyond these three cases remains pending']}\n"
                                                         '    '
                                                         'value.update(schema_required=18,harness_sha256=base.sha(Path(__file__).read_bytes()),\n'
                                                         '        '
                                                         'support_sha256=base.sha(PLANNING_PATH.read_bytes()))\n'
                                                         "    base.write_private_json(out/('self-check.json' if "
                                                         "args.self_check else 'NOT_RUN.json'),value)\n")]),
 'scripts/record-child-goal-ci-evidence.py': ('6d11ba710b957c5cc894960211457ba1771767b25603a1584c7c3b2b71ca02bb',
                                              '973464fd5deed4a55fbf5ba57cf1c7da10479231fe2ec0b8a21816bc985f91a3',
                                              [(1,
                                                18,
                                                '733000f2bc307289c1d91240f37ec8db0c6753e0e28fb75eac0325dcfa59f3a7',
                                                '"""Publish finite child-Goal CI results; never copy private '
                                                'browser evidence."""\n'
                                                'import argparse\n'
                                                'import hashlib\n'
                                                'import json\n'
                                                'import math\n'
                                                'import os\n'
                                                'from pathlib import Path\n'
                                                'import re\n'
                                                'import stat\n'
                                                'import threading\n'
                                                '\n'
                                                "CASES = ('normal', 'lost-start', 'cancel')\n"
                                                "EXPECTED = {'normal': {'CG01','CG02','CG05','CG06'},\n"),
                                               (34,
                                                44,
                                                '8c37c0f76a6730590b66b4887856015fb5f1f4087c49869eca7a920846fb4bd0',
                                                '        require(stat.S_ISREG(info.st_mode) and info.st_nlink==1 '
                                                'and 0<info.st_size<=maximum)\n'
                                                '        if private:require(stat.S_IMODE(info.st_mode)==0o600)\n'
                                                '        raw=stream.read(maximum+1);require(len(raw)<=maximum)\n'
                                                '        return raw\n'
                                                '\n'),
                                               (63,
                                                72,
                                                '7ff86095ea696f6b2ab945748021d2df26aa88a80e122781ef8e416ceefac3d5',
                                                '\n'
                                                'def candidate_summary(raw):\n'
                                                '    m=decode(raw);require(type(m)is dict and '
                                                "type(m.get('schema'))is int and m['schema']==18 and "
                                                "m.get('dirty')is False)\n"
                                                '    for key in '
                                                "('git_head','source_head','compiled_core_source_head'):\n"
                                                '        require(type(m.get(key))is str and '
                                                "re.fullmatch('[0-9a-f]{40}',m[key])is not None)\n"),
                                               (75,
                                                104,
                                                '3b975542cd5eb2731f20f87ac0d4710e0d7da204e7d1935b3e82fd429f372a6b',
                                                "    assets=m.get('frontend_assets');require(type(assets)is dict "
                                                'and 0<len(assets)<=4096)\n'
                                                '    for key,value in assets.items():require(type(key)is str and '
                                                "type(value)is str and re.fullmatch('[0-9a-f]{64}',value)is not "
                                                'None)\n'
                                                '    '
                                                "summary={'schema':18,'source_head':m['source_head'],'compiled_core_source_head':m['compiled_core_source_head'],\n"
                                                '        '
                                                "'manifest_sha256':sha(raw),'tools_sha256':{'batond':tools['batond'],'baton':tools['baton']},\n"
                                                '        '
                                                "'frontend_asset_count':len(assets),'frontend_assets_sha256':sha(json.dumps(assets,sort_keys=True,separators=(',',':')).encode())}\n"
                                                '    return m,summary\n'
                                                '\n'
                                                '# Fixed Core15 method/code vocabulary, not user-supplied display '
                                                'strings.\n'
                                                "DIAGNOSTIC_METHODS = frozenset(['approvals.decline', "
                                                "'approvals.finalize', 'command_validation_policies.list', "
                                                "'command_validation_policies.propose', "
                                                "'command_validation_policies.revoke', "
                                                "'command_validations.cancel', 'command_validations.get', "
                                                "'command_validations.list', 'command_validations.run', "
                                                "'connections.list', 'connections.propose', "
                                                "'connections.register', 'data_validation_policies.list', "
                                                "'data_validation_policies.propose', "
                                                "'data_validation_policies.revoke', 'data_validations.list', "
                                                "'data_validations.run', 'data_workspace_previews.cancel', "
                                                "'data_workspace_previews.get', 'data_workspace_previews.list', "
                                                "'data_workspace_previews.start', 'data_workspaces.approve', "
                                                "'data_workspaces.cancel', 'data_workspaces.create', "
                                                "'data_workspaces.get', 'data_workspaces.list', "
                                                "'data_workspaces.propose', 'events.subscribe', "
                                                "'factory_child_goals.advance_plan', "
                                                "'factory_child_goals.cancel', 'factory_child_goals.get', "
                                                "'factory_child_goals.list', 'factory_child_goals.start', "
                                                "'factory_child_plans.context', 'factory_child_plans.get', "
                                                "'factory_child_plans.list', 'factory_child_plans.submit', "
                                                "'factory_runs.cancel', 'factory_runs.get', 'factory_runs.start', "
                                                "'factory_scopes.get', 'factory_scopes.list', "
                                                "'factory_scopes.propose', 'factory_scopes.revoke', "
                                                "'goal_graph_runs.advance_plan', 'goal_graph_runs.cancel', "
                                                "'goal_graph_runs.dispatch', 'goal_graph_runs.get', "
                                                "'goal_graph_runs.list', 'goal_graph_runs.propose', "
                                                "'goal_node_approvals.propose', 'goal_plans.accept', "
                                                "'goal_plans.dispatch', 'goal_plans.get', 'goal_plans.list', "
                                                "'goal_plans.propose', 'goal_plans.ready_preview', "
                                                "'goal_plans.reserve', 'goal_plans.revise', "
                                                "'goal_runs.advance_plan', 'goal_runs.cancel', "
                                                "'goal_runs.dispatch', 'goal_runs.get', 'goal_runs.list', "
                                                "'goal_runs.propose', 'goal_scopes.approve', 'goal_scopes.get', "
                                                "'goal_scopes.list', 'goal_scopes.propose', 'goal_scopes.revoke', "
                                                "'grants.create', 'grants.list', 'grants.propose', "
                                                "'grants.revoke', 'projects.archive', 'projects.import', "
                                                "'projects.list', 'projects.reassociate', 'sessions.cancel', "
                                                "'sessions.create', 'sessions.get', 'sessions.list', "
                                                "'sessions.send', 'system.snapshot', 'tasks.cancel', "
                                                "'tasks.create', 'tasks.get', 'tasks.list', "
                                                "'validation_policies.list', 'validation_policies.propose', "
                                                "'validation_policies.revoke', 'validations.list', "
                                                "'validations.run', 'workspaces.create', 'workspaces.list'])\n"
                                                "DIAGNOSTIC_CODES = frozenset(['AUTH_REQUIRED', 'BUDGET_UNKNOWN', "
                                                "'CANCEL_REQUESTED', 'CAPABILITY_UNSUPPORTED', 'CHILD_CANCELLED', "
                                                "'CONFIG_CONFLICT', 'DEADLINE_EXCEEDED', "
                                                "'ENFORCEMENT_UNAVAILABLE', 'INPUT_CHANGED', 'INTERNAL_ERROR', "
                                                "'INVALID_REQUEST', 'NOT_FOUND', 'OUTCOME_UNKNOWN', "
                                                "'PERMISSION_DENIED', 'PROVIDER_DISCONNECTED', 'RESOURCE_LIMIT', "
                                                "'STALE_REVISION', 'TIMEOUT', 'VERSION_UNSUPPORTED', "
                                                "'WORKSPACE_BUSY'])\n"),
                                               (506,
                                                635,
                                                'c288dd38a30ea9a87dc995f71ff185ead0452720c0c7ecc6b4663c6a900caff0',
                                                '        '
                                                "'private_history_published':False,'finite_diagnostics':diagnostics}\n"
                                                '\n'
                                                'def publish(path,value):\n'
                                                "    pending=path.with_name(path.name+'.pending')\n"),
                                               (642,
                                                687,
                                                '78adcb529f57d5dafae780cd26e8fd2a34671b9173a41a74d570a8862ada3d29',
                                                '    '
                                                "p.add_argument('--case',choices=CASES,required=True);p.add_argument('--exit-code',required=True)\n"
                                                '    '
                                                "p.add_argument('--output',type=Path,required=True);args=p.parse_args()\n"
                                                "    exit_code=None if args.exit_code=='unobserved'else "
                                                'int(args.exit_code)\n'
                                                '    require(exit_code is None or -128<=exit_code<=255)\n'
                                                '    '
                                                'output=args.output.absolute();output.mkdir(mode=0o700,parents=True,exist_ok=False)\n'
                                                '    '
                                                "summary={'contract':'agentbaton.child-goal-ci.v1','case':args.case,'status':'failed',\n"
                                                "        'exit_code':exit_code,'command_state':'unobserved'if "
                                                'exit_code is None else '
                                                "'returned','failure_kind':'manifest_invalid','screenshots_published':False,'private_history_published':False,'finite_diagnostics':unobserved_diagnostics()}\n"
                                                "    stage='manifest_invalid'\n"
                                                '    try:\n'
                                                '        '
                                                'candidate,fields=candidate_summary(regular_bytes(args.manifest,4_194_304));summary.update(fields)\n'
                                                '        '
                                                "harness=Path(__file__).resolve().parents[1]/'apps/desktop/tests/factory-child-goal-real-ipc.py'\n"
                                                '        '
                                                "harness_sha=sha(regular_bytes(harness,2_097_152));summary['harness_sha256']=harness_sha\n"
                                                "        stage='private_directory_invalid'\n"
                                                '        '
                                                'info=args.private_dir.lstat();require(stat.S_ISDIR(info.st_mode)and '
                                                'not stat.S_ISLNK(info.st_mode)and '
                                                'stat.S_IMODE(info.st_mode)==0o700)\n'
                                                "        stage='report_missing'\n"
                                                "        report_path=args.private_dir/'report.json'\n"
                                                '        if not report_path.exists():\n'
                                                '            '
                                                "summary['failure_kind']='preflight_refused'if(args.private_dir/'preflight-refusal.json').is_file()else "
                                                "'report_missing'\n"
                                                '        else:\n'
                                                "            stage='report_invalid'\n"
                                                '            '
                                                'summary=project(decode(regular_bytes(report_path,33_554_432,private=True)),candidate,fields,args.case,exit_code,harness_sha)\n'
                                                '    except '
                                                '(OSError,ValueError,TypeError,KeyError,OverflowError,RecursionError):\n'
                                                '        '
                                                "summary['status']='failed';summary['failure_kind']=stage\n"
                                                "    publish(output/'summary.json',summary)\n")]),
 'tests/infrastructure/test_child_goal_ci_evidence.py': ('e1578f63960af2cf5ad296b1c5300e763595b484223a088fee8ae78169e6f914',
                                                         'd610cee32598ede2fb646f76e43efdf6d9da29df8b98b94d06d91443ac1f644d',
                                                         [(14,
                                                           22,
                                                           '6f97db7e2b84c348eaef686e254ef9a4831c74dfedd286889a66fabcda5c81c6',
                                                           'from unittest import mock\n'
                                                           '\n'
                                                           'ROOT=Path(__file__).resolve().parents[2]\n'
                                                           "P=types.ModuleType('child_goal_projection');P.__file__=str(ROOT/'scripts/record-child-goal-ci-evidence.py')\n"
                                                           "exec(compile(Path(P.__file__).read_bytes(),P.__file__,'exec'),P.__dict__)\n"
                                                           "SECRET='PRIVATE_SENTINEL_TOKEN_GRANT_RPC_HTML'\n"
                                                           '\n')])}


DRIVER = 'apps/desktop/tests/factory-child-goal-real-ipc.py'
PRIOR_DRIVER_LAYER = LAYERS[DRIVER]
RENDERED_ROOT_LAYER = ('06d78e2c4df8af6beb816ae85d9cec37256ea7315263e8fb3038be2439bb6855',
 '33bc3a72c30e509ae64cfce573743f58aa4406e4632ac42e3354d125f7ac242c',
 [(32,
   37,
   '0374c1701c2444b168e2745da5c88fdeb282f6cf5f1f09e261a330a4d9af1f1f',
   '\n'
   "PLANNING_PATH = Path(__file__).with_name('factory-child-planning-real-ipc.py')\n"
   "PLANNING_SHA256 = '9c2e0a94b80634650c0f3b5d90d447fe2122f787a8f5b0aca8c37e9f84e484a7'\n"
   'PLANNING_SOURCE = PLANNING_PATH.read_bytes()\n'
   "_spec = importlib.util.spec_from_file_location('_child_goal_planning_support', PLANNING_PATH)\n"),
  (1692,
   1744,
   '5887ce63c9a963f19265b21e0ef9089fb368aebd7362c9c201c7c856002fbe33',
   '\n'
   '\n'
   'def current_child_goal_preflight(args, output):\n'
   '    require(base.sha(PLANNING_SOURCE) == base.sha(PLANNING_PATH.read_bytes()) == PLANNING_SHA256,\n'),
  (2100,
   2138,
   'ac272aa001ef21982131bd8710e30fcadf00dc8b88a28e8cafd91141928de46b',
   "        goal_details=plans.get_by_test_id('factory-child-goals');expand(goal_details)\n"
   "        goal_panel=goal_details.get_by_test_id('factory-child-goal-panel')\n"
   "        goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()\n"
   "        expect(goal_panel.get_by_role('button',name='Execute this exact saved child "
   "plan',exact=True)).to_be_enabled()\n"
   "        shot('02-exact-saved-raw-reference-before-start',goal_panel)\n"
   "        goal_panel.get_by_role('button',name='Execute this exact saved child plan',exact=True).click()\n"
   "        started=wait(lambda:[r for r in report['http']if r['method']=='factory_child_goals.start'and not "
   "r.get('response',{}).get('error')and r.get('response')],lambda v:len(v)==1)[0]\n"
   '        '
   "initial=started['response']['result'];goal_id=initial['goal']['run']['id'];bounds.goal_runs[goal_id]=child['task_id']\n"),
  (2181,
   2203,
   '64054208b160ea991b53adbb254c1563ea261352b9167d3749721c84ac738d36',
   "            finally:phase_stamp('before_owner_capture','end')\n"
   "            require(before_owner['details']['task_accepted']is False and "
   "before_owner['details']['task_acceptance']is None)\n"
   "            phase_stamp('owner_UI_refresh_click','start')\n"
   '            finite.owner_refresh_begin(bound_child_goal_query(query,bounds),time.monotonic_ns())\n'
   "            sample_owner_ui(finite,goal_panel,query['goal_run_id'],'begin')\n"
   "            try:goal_panel.get_by_role('button',name='Refresh child Goal execution',exact=True).click()\n"
   '            finally:\n'
   '                finite.owner_refresh_click_finished(time.monotonic_ns())\n'
   "                phase_stamp('owner_UI_refresh_click','end')\n"
   "            owner=goal_panel.get_by_role('button',name='Review exact Factory-origin candidate in native "
   "dialog',exact=True)\n"
   "            phase_stamp('owner_enabled_expect','start')\n"
   '            try:expect(owner).to_be_enabled()\n'
   '            finally:\n'
   "                sample_owner_ui(finite,goal_panel,query['goal_run_id'],'end')\n")])
LAYERS[DRIVER] = (RENDERED_ROOT_LAYER[0], PRIOR_DRIVER_LAYER[1], RENDERED_ROOT_LAYER[2])


PLANNING_DRIVER = 'apps/desktop/tests/factory-child-planning-real-ipc.py'
SERIAL_PLAN_HISTORY_LAYER = ('64e27e2d1b49750542933ea50d9ad6a85ec93964ec1c21411a82c2a1451a6ea6',
 '9c2e0a94b80634650c0f3b5d90d447fe2122f787a8f5b0aca8c37e9f84e484a7',
 [(1264,
   1272,
   'd404b9f2ad049b6e20ed1cac5708c1238cd74db62e9c4d11d7d28e863d82d91f',
   "        root_region = root_summary.locator('..').get_by_role('region', name='Child task plans', exact=True)\n"
   "        expect(root_region.get_by_text('Current plan head: r2', exact=False)).to_be_visible()\n"
   "        child_summary = panel.locator('summary').filter(has_text='Child task plans · ' + child_id); "
   'child_summary.click()\n'
   "        child_region = child_summary.locator('..').get_by_role('region', name='Child task plans', "
   'exact=True)\n'),
  (1279,
   1286,
   'db7b2e27a134a59b64fa89a9240f5d5850ff3a4d7d633a75c33719af5a8e2fac',
   "        shot('02-real-child-ref-history-original-plan', child_region)\n"
   "        passed('CP04', 'UI list refs and select r1 retrieves exact original GoalPlan bytes and raw SHA, not "
   "fabricated history')\n"
   "        editor = root_region.get_by_test_id('factory-child-plan-editor'); editor.locator(':scope > "
   "summary').click()\n"
   "        editor.get_by_role('button', name='Prepare steps from latest revision', exact=True).click()\n"),
  (1332,
   1377,
   '695201a2a79aa5367984afc22d4cb840e201455ef4a1bce0f96f3311298b0c31',
   "        passed('CP06', 'UI rejects coherent budget/input/output/mandatory-owner-validator drift; actual Core "
   "independently rejects exact payloads with zero DB mutation')\n"
   '        # Stale head uses a new key and is denied; this is not transport replay.\n'
   '        before_stale = fingerprint_db(runtime)\n'
   "        stale = http_call('factory_child_plans_submit', {'scopeId': scope['id'], 'taskId': task['id'], "
   "'planJson': original_draft,\n"
   "            'expectedPlanRevision': 2, 'expectedRevision': scope['revision'], 'idempotencyKey': "
   "'actual-stale-head'})\n"
   "        assert stale['value']['error']['code'] == 'STALE_REVISION'\n"
   "        assert stale['value']['error']['side_effect_state'] == 'not_started' and fingerprint_db(runtime) == "
   'before_stale\n'
   "        passed('CP07', 'Actual stale plan CAS refuses atomically; observed head/floors remain r3')\n"
   "        root_region.get_by_role('button', name='Discard local draft', exact=True).click()\n"
   "        raw4 = next_raw(stored, 'Actual lost-response UI revision four')\n")])

# Reverse only the pinned Start-failure observation layer before existing inverses.
START_FAILURE_LAYERS = {
    'apps/desktop/tests/factory-child-goal-real-ipc.py': (
        '320af1df2802e6e543c68ca3f5ce26d15bcba4a3beba057314fbd561ef44b73d',
        '06d78e2c4df8af6beb816ae85d9cec37256ea7315263e8fb3038be2439bb6855',
        [
            (1789, 1790,
             '0da2dd1350c272a3b7e483a3378cc1e1bc574af8b4b8f0780e1e09e041f39ccf',
             ''),
            (2136, 2152,
             'ab66a49ef732feb48aa3d0de3ba5844aa5baa58ed176ab6141d80b0acf57dfa8',
             "        start_button.click()\n        started=wait(lambda:[r for r in report['http']if r['method']=='factory_child_goals.start'and not r.get('response',{}).get('error')and r.get('response')],lambda v:len(v)==1)[0]\n"),
            (2443, 2444,
             '335975afdc497e1f153508ee66d2f3343efee9c63862a5f0c55ae9beb4b51afc',
             "        report['finite_diagnostics']=finite.snapshot()\n"),
        ],
    ),
    'scripts/record-child-goal-ci-evidence.py': (
        '8d03881dd61db2c398f96a922190c779079d9cfcd9635483ffea185a6acbc741',
        '6d11ba710b957c5cc894960211457ba1771767b25603a1584c7c3b2b71ca02bb',
        [
            (214, 223,
             '06afc0dbd7534e463d6792c1699b1262aadcc565c7ed19c6c62c1067b9142bf7',
             ''),
            (508, 511,
             'fe610a2df2ae3c77ad169bcc943898cc63eef54ef7fc365c761393cd23ddf6e7',
             ''),
            (519, 622,
             '4f48d34720a21e91d7967c9014c5233e512d129b7b05d933505e2991c5b43e08',
             ''),
        ],
    ),
    'tests/infrastructure/test_child_goal_ci_evidence.py': (
        '6622dfa6aa6f4309d9bfd6a5e95c6ad49a9320c4028f2b4ff924e6211e009682',
        'e1578f63960af2cf5ad296b1c5300e763595b484223a088fee8ae78169e6f914',
        [
            (210, 355,
             '311576530834190c0b5fe218755af75706519f71acd2411368f8d9404034ccd6',
             ''),
        ],
    ),
}


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def reverse_pinned_spans(source, layer, error=AssertionError):
    current, historical, spans = layer
    if layer is START_FAILURE_LAYERS[PROJECTOR] and sha(source) == MCP_LATE_WAIT_LAYER[0]:
        source = restore_mcp_late_wait_layer(source, error=error)
    if layer is START_FAILURE_LAYERS[DRIVER]:
        source = restore_reviewed_factory_ui_layer(DRIVER, source, error)
        cancel = CANCEL_PROC_FD_LAYERS[DRIVER]
        if sha(source) == cancel[0]:
            source = reverse_pinned_spans(source, cancel, error)
    if sha(source) != current:
        raise error('Unrecognized Child Goal source bytes')
    lines = source.splitlines(keepends=True)
    for start, end, span_sha, replacement in reversed(spans):
        if not 0 <= start < end <= len(lines) or sha(''.join(lines[start:end])) != span_sha:
            raise error('Child Goal inverse span changed')
        lines[start:end] = replacement.splitlines(keepends=True)
    restored = ''.join(lines)
    if sha(restored) != historical:
        raise error('Child Goal inverse did not restore the exact source')
    return restored


MCP_LATE_WAIT_LAYER = ('88da82f3f97601f7a0c71948bec4a4caf562be87c3343d8751b034cb9124d76d',
 '8d03881dd61db2c398f96a922190c779079d9cfcd9635483ffea185a6acbc741',
 [(207, 215, '1af18b1a958d003fb0bb2e0c6df966fffccc871cd47afbb5520b1d2f6eba8e2d', ''),
  (480, 561, 'b2572c4d8b787bd90371f3762114b1d57e880cfb961245c9f2670726f075733e', ''),
  (566, 570, '1f62170c77eef1f3e4b5c0e7fd0c711cf3d4bdd21f18cee0c86f57eb24fe75e8', ''),
  (573, 574, 'fc23d2e4f90f0597a3874a17d911dc09f006f7657e8d1393cb07fc611901e05d', ''),
  (584, 585, 'a4066376fc9423e51427c11fb0aaeba1041515902c8bed240012ae208cab9c31', ''),
  (586, 588, 'f742f6599a215f1d5030daf56c37abccc270df34213ff27a6a5317294d2a4427', ''),
  (603,
   604,
   'c034942bda716023d50922df058f345e7f52c5396b51b35f95d9ca849aa322fe',
   "    if diagnostics.get('version')==5:\n")])


def restore_mcp_late_wait_layer(source, *, error=AssertionError):
    if type(source) is not str:
        raise error('Unknown MCP late-wait projector source')
    if sha(source) == MCP_LATE_WAIT_LAYER[1]:
        return source
    return reverse_pinned_spans(source, MCP_LATE_WAIT_LAYER, error)


def without_start_failure_layer(path, source, error):
    if path == PROJECTOR and sha(source) == MCP_LATE_WAIT_LAYER[0]:
        source = restore_mcp_late_wait_layer(source, error=error)
    layer = START_FAILURE_LAYERS.get(path)
    digest = sha(source)
    if layer is not None and (digest == layer[0] or path == DRIVER and digest == CANCEL_PROC_FD_LAYERS[DRIVER][0]):
        return reverse_pinned_spans(source, layer, error)
    return source


def restore_child_goal_before_schema19(path, source, *, error=AssertionError):
    if type(path) is not str or path not in LAYERS or type(source) is not str:
        raise error('Unknown Child Goal schema19 inverse arguments')
    source = restore_reviewed_factory_ui_layer(path, source, error)
    source = without_start_failure_layer(path, source, error)
    layer = LAYERS[path]
    digest = sha(source)
    if digest == layer[1]:
        return source
    if path == DRIVER:
        if digest == RENDERED_ROOT_LAYER[0]:
            source = reverse_pinned_spans(source, RENDERED_ROOT_LAYER, error)
        layer = PRIOR_DRIVER_LAYER
    return reverse_pinned_spans(source, layer, error)


CHILD_CONTEXT_GUARD = 'tests/infrastructure/test_child_goal_diagnostics_v4.py'
CHILD_CONTEXT_GUARD_LAYER = (
    'c681f130f35f45dfaac8c57e1090a213c18539e9b7cf004ddfc8a8ca4af6a1ec',
    '2f383e2ed7fcb15dc302254c302a63a08fdba40886bb8b1df39decfb5da6d9d7',
)
CHILD_CONTEXT_GUARD_MANIFEST_SHA256 = '7e5ebe20338c90e51abc7b8b1ea56530b848910ee9629ca36b81db39ab31a92e'


def restore_child_context_guard(source, error):
    root = Path(__file__).resolve().parents[2]
    path = root / 'docs/execution/pr33-child-start-replay/GUARD_INVERSE.json'
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != CHILD_CONTEXT_GUARD_MANIFEST_SHA256:
        raise error('Unknown child Context guard manifest')
    record = json.loads(raw)
    current, prior = CHILD_CONTEXT_GUARD_LAYER
    if (record['path'] != CHILD_CONTEXT_GUARD or record['candidate_sha256'] != current
            or record['base_sha256'] != prior):
        raise error('Unknown child Context guard source targets')
    spans = [(edit['start'], edit['end'], sha(edit['expected']), edit['replacement'])
             for edit in record['edits']]
    return reverse_pinned_spans(source, (current, prior, spans), error)


CANCEL_PROC_FD_LAYERS = {
    DRIVER: (
        '73470592d65790678a57a17bb14b13351f261902fc87411039e6ec5ff31c2e11',
        '320af1df2802e6e543c68ca3f5ce26d15bcba4a3beba057314fbd561ef44b73d',
        [
            (34, 35, '3a766f0e8ebf72bdfc99030057a3f398ed38f49788ac9b47d694a71771fe2093',
             "PLANNING_SHA256 = '64e27e2d1b49750542933ea50d9ad6a85ec93964ec1c21411a82c2a1451a6ea6'\n"),
        ],
    ),
    'apps/desktop/tests/factory-real-ipc.py': (
        '6dfa764259b667f57fe0a579ecbbe242d6a4d2b253e1878f158e68cd161a1d93',
        'e81410e1224a9fb1449a2e264f35eeb6ab5d5d42ad0b41ecd2c8b1fbfc6db464',
        [
            (10, 11, 'b17c41ffb3939b0dc71fa9e725ac38888b83de0c14b844fb813deda3b5410663', ''),
            (527, 538, 'b3ce4d06e07042b7ab3c54ebf8305a8d538029cfae4c1af893e159885533efe1', ''),
        ],
    ),
    PLANNING_DRIVER: (
        'fe607d2c8bfeaf9ee34a0656b823bec9e00c11c9152501ebbd1e273bfbf79f39',
        '64e27e2d1b49750542933ea50d9ad6a85ec93964ec1c21411a82c2a1451a6ea6',
        [
            (36, 37, 'd0dd2b083e107213cd7bf2fac090dd12eadee28168f991ecefa05b0b969ed91e',
             "FACTORY_SUPPORT_SHA256 = 'e81410e1224a9fb1449a2e264f35eeb6ab5d5d42ad0b41ecd2c8b1fbfc6db464'\n"),
        ],
    ),
}


def restore_cancel_proc_fd_layer(path, source, *, error=AssertionError):
    layer = CANCEL_PROC_FD_LAYERS.get(path)
    if layer is None or type(source) is not str:
        raise error('Unknown cancel proc FD source')
    if sha(source) == layer[1]:
        return source
    return reverse_pinned_spans(source, layer, error)


# Exact browser navigation and imported-support pin changes since c87a3e83.
# Keep the schema19, bounded-runtime and stop-evidence inverse layers intact.
FACTORY_UI_SIMPLIFICATION_LAYERS = {'apps/desktop/tests/factory-real-ipc.py': ('54e5d19cc8e19d93bb93debf7ab15c11f64e3ac5e29f84b612fb1b0e1c3dad37',
                                            '6dfa764259b667f57fe0a579ecbbe242d6a4d2b253e1878f158e68cd161a1d93',
                                            [(51,
                                              52,
                                              '238a88a8f74329dae9fb067cecf31f31348d11906b4b3f4c4f17c436242f8d22',
                                              'GRAPH_SUPPORT_SHA256 = '
                                              "'550256ce728cbc26c5873237e09888195cfe01726b697d46d5e2b881e46b4d18'\n"),
                                             (1317,
                                              1325,
                                              'bb303feb72661170c859a2e978b0cb1dd99735251c5ff1b67778f5a0b6e40cb1',
                                              ''),
                                             (1568,
                                              1569,
                                              'c702b476dfcc4fa70cfda5f95b9b6c4b96121df55108ece5a246fd7dff4cb880',
                                              ''),
                                             (1691,
                                              1695,
                                              '257849930b3e29c87d183749492d7c0f705909fffa3b42d82228cd116746d735',
                                              '')]),
 'apps/desktop/tests/factory-child-planning-real-ipc.py': ('2cf813bc87db8e2f8ee5d7006815087117155bddafa56022486b1a2adc14d303',
                                                           'fe607d2c8bfeaf9ee34a0656b823bec9e00c11c9152501ebbd1e273bfbf79f39',
                                                           [(36,
                                                             37,
                                                             '36ee3b12b53745d51ed2222633b052c917d10e306870d399a7378a4c396624d2',
                                                             'FACTORY_SUPPORT_SHA256 = '
                                                             "'6dfa764259b667f57fe0a579ecbbe242d6a4d2b253e1878f158e68cd161a1d93'\n"),
                                                            (1188,
                                                             1189,
                                                             '655fbd25121a3cca3c3e23ab93f70d5582dfcb5d50509542c4d430e11efaafd5',
                                                             ''),
                                                            (1192,
                                                             1194,
                                                             'f0c39b11623a3092119d2b66925f4848ce4f046d7112ce48d227f3f475daea9d',
                                                             "        form = panel.locator('form')\n"),
                                                            (1207,
                                                             1209,
                                                             'b6e5e086689dc86784ac1e1a839548a7bf461f6cae8d3a2fef63748e995ce6d8',
                                                             '        planning = '
                                                             "form.locator('details').filter(has=page.get_by_text('Child-planning "
                                                             "policy for Native review', exact=True))\n"
                                                             "        planning.locator('summary').click(); "
                                                             "planning.get_by_label('Allow bounded child "
                                                             "planning', exact=True).check()\n")]),
 'apps/desktop/tests/factory-child-goal-real-ipc.py': ('4ffcc4d61abd73f8b68eb22755ca9fdee24a522705804f68f7f9895d7b1fc709',
                                                       '73470592d65790678a57a17bb14b13351f261902fc87411039e6ec5ff31c2e11',
                                                       [(34,
                                                         35,
                                                         '1c437773f8ab239110d4460328a3ecdbf65ecf36cc454c08b20dbe10451bd0d6',
                                                         'PLANNING_SHA256 = '
                                                         "'fe607d2c8bfeaf9ee34a0656b823bec9e00c11c9152501ebbd1e273bfbf79f39'\n"),
                                                        (2042,
                                                         2046,
                                                         '8d50106360610dded375d1db796406f3d2378c01f28fd5e47ad4e84eb2f04e70',
                                                         '        '
                                                         "panel=page.get_by_test_id('factory-runtime');expand(panel.locator(':scope "
                                                         "> details'));form=panel.locator('form')\n"),
                                                        (2055,
                                                         2056,
                                                         '717caa61deed9e2e161efd3b49cbcaf40ae9e95815bceceac9b3e8d6b67e03e5',
                                                         '        '
                                                         "policy=form.locator('details').filter(has=page.get_by_text('Child-planning "
                                                         'policy for Native '
                                                         "review',exact=True));expand(policy)\n"),
                                                        (2060,
                                                         2061,
                                                         'b5999bf42ab24601c61c465045ed46f830b379f1c4d07f7de5eae4cdf135d561',
                                                         '        '
                                                         "execution=form.locator('details').filter(has=page.get_by_text('Child "
                                                         'Goal execution '
                                                         "policy',exact=True));expand(execution)\n"),
                                                        (2099,
                                                         2100,
                                                         '94fb9c72f2b4088959fbf219837b187e6a724af2ed2283aa24240c8df313939d',
                                                         "        plan_details=task_item.locator(':scope > "
                                                         "details');expand(plan_details)\n")])}


def restore_factory_before_ui_simplification(path, source, *, error=AssertionError):
    if type(path) is not str or path not in FACTORY_UI_SIMPLIFICATION_LAYERS or type(source) is not str:
        raise error('Unknown Factory UI inverse arguments')
    layer = FACTORY_UI_SIMPLIFICATION_LAYERS[path]
    if sha(source) == layer[1]:
        return source
    return reverse_pinned_spans(source, layer, error)


def restore_reviewed_factory_ui_layer(path, source, error):
    layer = FACTORY_UI_SIMPLIFICATION_LAYERS.get(path)
    if layer is not None and sha(source) == layer[0]:
        return restore_factory_before_ui_simplification(path, source, error=error)
    return source


def normalize_child_goal_outer_layer(path, source, *, error=AssertionError):
    source = restore_reviewed_factory_ui_layer(path, source, error)
    cancel_layer = CANCEL_PROC_FD_LAYERS.get(path)
    if cancel_layer is not None and sha(source) == cancel_layer[0]:
        source = restore_cancel_proc_fd_layer(path, source, error=error)
    if path == CHILD_CONTEXT_GUARD and sha(source) == CHILD_CONTEXT_GUARD_LAYER[0]:
        return restore_child_context_guard(source, error)
    source = without_start_failure_layer(path, source, error)
    layer = LAYERS.get(path)
    digest = sha(source)
    if path == PLANNING_DRIVER and digest == SERIAL_PLAN_HISTORY_LAYER[0]:
        return reverse_pinned_spans(source, SERIAL_PLAN_HISTORY_LAYER, error)
    if layer is not None and (digest == layer[0] or path == DRIVER and digest == PRIOR_DRIVER_LAYER[0]):
        return restore_child_goal_before_schema19(path, source, error=error)
    return source


def load_historical_projector(root):
    path = root / PROJECTOR
    original = restore_child_goal_before_schema19(PROJECTOR, path.read_text())
    module = ModuleType('historical_child_goal_projection')
    module.__file__ = str(path)
    exec(compile(original, str(path), 'exec'), module.__dict__)
    return module
