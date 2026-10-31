"""Pure FIFO/selector models and source controls; no process, pipe or SQL opened."""
import ast
import copy
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import types
import unittest
from unittest import mock

ROOT=Path(__file__).resolve().parents[2]
DRIVER=ROOT/'apps/desktop/tests/factory-child-goal-real-ipc.py'
IDENTITY={'pid':2001,'ppid':2000,'start_ticks':'4567','uid':1000,'pgid':2001}
NONCE='a'*32
PIN=[1,2,1000,0o700]


def sha(raw):return hashlib.sha256(raw).hexdigest()


# Exact outer composition only. Both targets are immutable reviewed Git blobs:
# Root fd242 (strong binding/ACK/schema17), incoming b55 (v3 finite diagnostics).
# This inverse is source evidence; no restored historical collector is executed.
_COMPOSED_DRIVER_INVERSES = {'ours': {'path': 'apps/desktop/tests/factory-child-goal-real-ipc.py',
          'target_stage': 'ours',
          'merged_sha256': '9acadb5d494e7cd055b871530867fc273bff9588384beeb9415ffbdaa68c2610',
          'restored_sha256': '4b0332152c2dff9e4d4d7df94dede8ce23b0735c1b76adbef14398c142811b88',
          'edits': [{'start': 1070,
                     'end': 1071,
                     'expected': 'def bind_graph_observation(fixed, expected_argv, old, context, output, '
                                 'finite=None, cancel_durable=None):\n',
                     'replacement': 'def bind_graph_observation(fixed, expected_argv, old, context, '
                                    'output):\n'},
                    {'start': 1082,
                     'end': 1083,
                     'expected': '    graph_finite_observation(finite, known, result, observation, '
                                 'cancel_durable)\n',
                     'replacement': ''},
                    {'start': 1094,
                     'end': 1197,
                     'expected': 'def graph_finite_observation(finite, known, result, observation, '
                                 'cancel_durable):\n'
                                 '    """Project the existing two samples; never re-read a PID or change its '
                                 'guard."""\n'
                                 '    if finite is None:return\n'
                                 "    kind=observation['argv']['kind']\n"
                                 "    if kind=='empty':\n"
                                 '        durable=cancel_durable()if cancel_durable is not None else None\n'
                                 '        '
                                 "finite.graph_identity(graph_empty_identity_diagnostic(known,observation['before'],observation['after'],durable))\n"
                                 '    '
                                 "reasons={'empty':'empty','oversize':'oversized','missing_nul':'missing_nul','mismatch':'argv_mismatch'}\n"
                                 '    # A previously strong-bound terminal/absent observation remains only '
                                 'that\n'
                                 '    # observation. Finite argv failure describes an original guard '
                                 'refusal.\n'
                                 "    if result['kind']=='not_verified'and kind in reasons:\n"
                                 "        finite.graph_argv({'reason':reasons[kind],'previously_bound':known "
                                 'is not None})\n'
                                 '\n'
                                 '\n'
                                 'def graph_identity_snapshot(pid,current=None):\n'
                                 '    """One readonly sample; never signal, wait, retry or infer owned '
                                 'stop."""\n'
                                 '    try:\n'
                                 '        value=support.proc_identity(pid)if current is None else current\n'
                                 '        return '
                                 "{'observation':'observed','pid':value['pid'],'ppid':value['parent_pid'],\n"
                                 '            '
                                 "'start_ticks':value['start_ticks'],'uid':Path('/proc',str(pid)).stat().st_uid,\n"
                                 "            'pgid':os.getpgid(pid)}\n"
                                 '    except (FileNotFoundError,ProcessLookupError):return '
                                 "{'observation':'absent'}\n"
                                 '    except (OSError,ValueError,TypeError,KeyError,AssertionError):return '
                                 "{'observation':'unobserved'}\n"
                                 '\n'
                                 '\n'
                                 'def graph_identity_initial(expected,before,after,uid):\n'
                                 "    fields={'observation','pid','ppid','start_ticks','uid','pgid'}\n"
                                 '    if not(type(before)is dict and type(after)is dict and '
                                 'set(before)==set(after)==fields):return None\n'
                                 "    if before!=after or before['observation']!='observed'or type(uid)is "
                                 'not int:return None\n'
                                 '    if not(all(type(before[k])is int for k in '
                                 "('pid','ppid','uid','pgid'))and before['ppid']>0):return None\n"
                                 "    if type(before['start_ticks'])is not str or "
                                 "re.fullmatch('[0-9]+',before['start_ticks'])is None:return None\n"
                                 "    if not(before['uid']==uid and all(before[k]==expected[k]for k in "
                                 "('pid','pgid','start_ticks'))):return None\n"
                                 '    return copy.deepcopy(before)\n'
                                 '\n'
                                 '\n'
                                 'def graph_empty_identity_diagnostic(known,before,after,cancel_durable):\n'
                                 "    value={'initial_binding':'verified'if known is not None else "
                                 "'unobserved',\n"
                                 '        '
                                 "'empty_read_observed':True,'known_same_identity':None,'uid_matches':None,\n"
                                 "        'pgid_matches':None,'ppid_matches':None,'start_ticks_match':None,\n"
                                 "        'after_read':'unobserved','exact_cancel_durable':cancel_durable}\n"
                                 "    if after.get('observation')=='absent':value['after_read']='absent'\n"
                                 '    if known is not None and '
                                 "before.get('observation')==after.get('observation')=='observed':\n"
                                 '        value.update({name:before[key]==after[key]==known[key]for name,key '
                                 'in\n'
                                 '            '
                                 "[('uid_matches','uid'),('pgid_matches','pgid'),('ppid_matches','ppid'),('start_ticks_match','start_ticks')]})\n"
                                 '        same=all(value[k]is True for k in '
                                 "('uid_matches','pgid_matches','ppid_matches','start_ticks_match'))and "
                                 "before['pid']==after['pid']==known['pid']\n"
                                 "        value.update(known_same_identity=same,after_read='same'if same "
                                 "else 'changed')\n"
                                 '    return finite_projection.graph_identity_diagnostic(value)\n'
                                 '\n'
                                 '\n'
                                 'def remember_graph_cancel(context,call):\n'
                                 "    if call.method!='factory_child_goals.cancel':return\n"
                                 '    '
                                 "snapshot={'params':copy.deepcopy(call.params),'revision':call.revision,'key':call.key}\n"
                                 "    previous=context.get('_graph_cancel_request')\n"
                                 "    if previous is None:context['_graph_cancel_request']=snapshot\n"
                                 '    elif not '
                                 "support.exact_json_value(previous,snapshot):context['_graph_cancel_request_conflict']=True\n"
                                 '\n'
                                 '\n'
                                 'def '
                                 'exact_graph_cancel_rows(scope,factory_run_id,fixed,call,origin_rows,run_rows,idem_rows):\n'
                                 '    """Pure tuple/document binding predicate; committed intent proves no '
                                 'stop."""\n'
                                 '    try:\n'
                                 '        '
                                 "p=call['params'];run_id=fixed['run_id'];task_id=fixed['request']['task_binding']['task_id']\n"
                                 '        if not(type(p)is dict and '
                                 "set(p)=={'scope_id','task_id','goal_run_id','expected_goal_run_revision'}):return "
                                 'False\n'
                                 "        if not(p['scope_id']==scope['id']and p['task_id']==task_id and "
                                 "p['goal_run_id']==run_id):return False\n"
                                 "        if not(type(call['revision'])is int and call['revision']>0 and "
                                 "type(p['expected_goal_run_revision'])is int and "
                                 "p['expected_goal_run_revision']>0):return False\n"
                                 "        if type(call['key'])is not str or "
                                 "planning.KEY.fullmatch(call['key'])is None:return False\n"
                                 '        if not(len(origin_rows)==len(run_rows)==len(idem_rows)==1):return '
                                 'False\n'
                                 '        goal,goal_scope,fscope,frun,task,root,raw=origin_rows[0]\n'
                                 '        if not all(type(value)is str and value for value in '
                                 '(goal,goal_scope,fscope,frun,task,root)):return False\n'
                                 '        record=json.loads(raw,object_pairs_hook=planning.unique_object)\n'
                                 '        if '
                                 "not((goal,fscope,frun,task,root)==(run_id,scope['id'],factory_run_id,task_id,scope['root_task_id'])):return "
                                 'False\n'
                                 "        origin=record['origin'];initial=record['initial_run']\n"
                                 "        if not(record['run_id']==run_id and record['scope_id']==goal_scope "
                                 "and initial['id']==run_id and initial['scope_id']==goal_scope and "
                                 "initial['task_id']==task_id):return False\n"
                                 "        if not(origin['factory_scope_id']==fscope and "
                                 "origin['factory_run_id']==frun and origin['root_task_id']==root):return "
                                 'False\n'
                                 '        '
                                 'rid,rscope,rtask,revision,state,raw=run_rows[0];current=json.loads(raw,object_pairs_hook=planning.unique_object)\n'
                                 '        if not((rid,rscope,rtask)==(run_id,goal_scope,task_id)and '
                                 "type(revision)is int and type(current['revision'])is int and "
                                 "revision==current['revision']and state==current['state']):return False\n"
                                 "        if not(current['id']==rid and current['scope_id']==rscope and "
                                 "current['task_id']==rtask and current['cancel_requested']is True and "
                                 "current['task_accepted']is False):return False\n"
                                 '        if state not in '
                                 "{'cancel_requested','cancelled','outcome_unknown'}:return False\n"
                                 '        '
                                 'payload,raw=idem_rows[0];payload=json.loads(payload,object_pairs_hook=planning.unique_object);receipt=json.loads(raw,object_pairs_hook=planning.unique_object)\n'
                                 '        if not '
                                 "support.exact_json_value(payload,[call['revision'],p]):return False\n"
                                 "        if not(receipt['factory_scope_id']==fscope and "
                                 "receipt['factory_run_id']==frun and receipt['factory_root_task_id']==root "
                                 "and receipt['task_id']==task_id):return False\n"
                                 "        result=receipt['goal']['run']\n"
                                 "        return (result['id']==run_id and result['scope_id']==goal_scope "
                                 "and result['task_id']==task_id\n"
                                 "            and receipt['goal']['task_accepted']is False and "
                                 "result['cancel_requested']is True and result['task_accepted']is False\n"
                                 "            and result['state']in "
                                 "{'cancel_requested','cancelled','outcome_unknown'}\n"
                                 "            and type(result['revision'])is int and "
                                 "p['expected_goal_run_revision']<result['revision']<=revision)\n"
                                 '    except (ValueError,TypeError,KeyError,IndexError,Refusal):return '
                                 'False\n'
                                 '\n'
                                 '\n'
                                 'def observe_exact_graph_cancel(db,scope,context,fixed):\n'
                                 "    call=context.get('_graph_cancel_request')\n"
                                 '    if call is None:return None\n'
                                 "    if context.get('_graph_cancel_request_conflict'):return False\n"
                                 '    try:\n'
                                 '        '
                                 "run_id=fixed['run_id'];task_id=fixed['request']['task_binding']['task_id']\n"
                                 '        origin=db.execute("SELECT '
                                 'goal_run_id,goal_scope_id,factory_scope_id,factory_run_id,task_id,root_task_id,CASE '
                                 'WHEN length(CAST(document AS BLOB))<=1048576 THEN document END FROM '
                                 'factory_child_goal_runs WHERE goal_run_id=? LIMIT 2",[run_id]).fetchall()\n'
                                 '        current=db.execute("SELECT id,scope_id,task_id,revision,state,CASE '
                                 'WHEN length(CAST(document AS BLOB))<=1048576 THEN document END FROM '
                                 'goal_runs WHERE id=? LIMIT 2",[run_id]).fetchall()\n'
                                 '        '
                                 "namespace='factory:'+scope['id']+':owner:'+task_id+':child-goal:cancel'\n"
                                 '        idem=db.execute("SELECT CASE WHEN length(CAST(payload AS '
                                 'BLOB))<=1048576 THEN payload END,CASE WHEN length(CAST(result AS '
                                 'BLOB))<=1048576 THEN result END FROM idempotency WHERE scope=? AND key=? '
                                 'LIMIT 2",[namespace,call[\'key\']]).fetchall()\n'
                                 '        return '
                                 "exact_graph_cancel_rows(scope,context['run_id'],fixed,call,origin,current,idem)\n"
                                 '    except (sqlite3.Error,OSError,ValueError,TypeError,KeyError):return '
                                 'None\n'
                                 '\n'
                                 '\n'
                                 'def '
                                 'collect_graph_records(runtime,workspace,scope,context,verified,output,finite=None):\n',
                     'replacement': 'def '
                                    'collect_graph_records(runtime,workspace,scope,context,verified,output):\n'},
                    {'start': 1229,
                     'end': 1231,
                     'expected': '            '
                                 "bind_graph_observation(fixed,row['provider_argv'],old,context,output,finite,\n"
                                 '                '
                                 'lambda:observe_exact_graph_cancel(db,scope,context,fixed))\n',
                     'replacement': '            '
                                    "bind_graph_observation(fixed,row['provider_argv'],old,context,output)\n"},
                    {'start': 1674,
                     'end': 1675,
                     'expected': '        '
                                 'collect_graph_records(runtime,workspace,scope,record_context,graph_verified,output,finite)\n',
                     'replacement': '        '
                                    'collect_graph_records(runtime,workspace,scope,record_context,graph_verified,output)\n'},
                    {'start': 1818,
                     'end': 1822,
                     'expected': "                    stage='core_call';read_started=time.monotonic_ns()\n"
                                 '                    remember_graph_cancel(record_context,call)\n'
                                 '                    '
                                 'value,reply=host.wire(call);read_finished=time.monotonic_ns();require(len(reply)<=planning.MAX_WIRE)\n'
                                 '                    '
                                 'finite.owner_refresh_reply(call.method,call.params,value,read_started,read_finished)\n',
                     'replacement': '                    '
                                    "stage='core_call';value,reply=host.wire(call);require(len(reply)<=planning.MAX_WIRE)\n"},
                    {'start': 2006,
                     'end': 2007,
                     'expected': '            '
                                 'finite.owner_refresh_begin(bound_child_goal_query(query,bounds),time.monotonic_ns())\n',
                     'replacement': ''},
                    {'start': 2008,
                     'end': 2011,
                     'expected': '            finally:\n'
                                 '                finite.owner_refresh_click_finished(time.monotonic_ns())\n'
                                 "                phase_stamp('owner_UI_refresh_click','end')\n",
                     'replacement': "            finally:phase_stamp('owner_UI_refresh_click','end')\n"},
                    {'start': 2014,
                     'end': 2017,
                     'expected': '            finally:\n'
                                 '                finite.owner_refresh_end(time.monotonic_ns())\n'
                                 "                phase_stamp('owner_enabled_expect','end')\n",
                     'replacement': "            finally:phase_stamp('owner_enabled_expect','end')\n"}]},
 'theirs': {'path': 'apps/desktop/tests/factory-child-goal-real-ipc.py',
            'target_stage': 'theirs',
            'merged_sha256': '9acadb5d494e7cd055b871530867fc273bff9588384beeb9415ffbdaa68c2610',
            'restored_sha256': '7dc4732f18be492fddcc49bb989f233cd4fc409bb11de33a7951f7d5cf36e338',
            'edits': [{'start': 1,
                       'end': 2,
                       'expected': '"""Schema17 real child-Goal browser/Unix IPC harness, not '
                                   'native/Provider proof.\n',
                       'replacement': '"""Schema16 real child-Goal browser/Unix IPC harness, not '
                                      'native/Provider proof.\n'},
                      {'start': 6,
                       'end': 7,
                       'expected': 'all successful business receipts then come from the supplied real '
                                   'Core17.\n',
                       'replacement': 'all successful business receipts then come from the supplied real '
                                      'Core16.\n'},
                      {'start': 234,
                       'end': 235,
                       'expected': '    manifest = '
                                   "{'schema':17,'binaries':{'batond':base.sha(binary.read_bytes())},'source_head':'a'*40}\n",
                       'replacement': '    manifest = '
                                      "{'schema':16,'binaries':{'batond':base.sha(binary.read_bytes())},'source_head':'a'*40}\n"},
                      {'start': 238,
                       'end': 240,
                       'expected': "    checks.append('integer17-manifest-positive-pure-bytes-only')\n"
                                   '    for bad in '
                                   "[12,13,14,15,16,18,True,15.0,16.0,17.0,'15','16','17',None]:\n",
                       'replacement': "    checks.append('integer16-manifest-positive-pure-bytes-only')\n"
                                      "    for bad in [12,13,14,15,17,True,15.0,16.0,'15','16',None]:\n"},
                      {'start': 248,
                       'end': 250,
                       'expected': '        def __init__(self,schema=17,values=None): '
                                   'self.schema,self.values=schema,values or {}\n'
                                   '        def execute(self,query,params=()):\n',
                       'replacement': '        def __init__(self,schema=16,values=None): '
                                      'self.schema,self.values=schema,values or {}\n'
                                      '        def execute(self,query):\n'},
                      {'start': 251,
                       'end': 253,
                       'expected': "            elif query.startswith('SELECT type,CASE'):\n"
                                   "                value=('table','CREATE TABLE '+params[0]+'(id "
                                   "INTEGER)')\n",
                       'replacement': ''},
                      {'start': 260,
                       'end': 261,
                       'expected': '                def fetchone(self): return value if type(value)is tuple '
                                   'else (value,)\n',
                       'replacement': '                def fetchone(self): return (value,)\n'},
                      {'start': 274,
                       'end': 281,
                       'expected': '    for table in support.INTEGRATION_RECORD_TABLES.values():\n'
                                   "        for bad in [1,None,True,0.0,-1,RuntimeError('missing table or "
                                   "query failure')]:\n"
                                   '            try: '
                                   'support.current_factory_counts(FakeDB(values={table:bad}))\n'
                                   '            except (AssertionError,RuntimeError): pass\n'
                                   '            else: raise AssertionError((table,bad))\n'
                                   '            '
                                   "checks.append('integration-zero-reject:'+table+':'+repr(bad))\n"
                                   '    for schema in '
                                   "[12,13,14,15,16,18,15.0,16.0,17.0,True,'15','16','17',None]:\n",
                       'replacement': "    for schema in [12,13,14,15,17,15.0,16.0,True,'15','16',None]:\n"},
                      {'start': 311,
                       'end': 312,
                       'expected': "    snapshot={'schema':17,'factory_counts':{key:2 if key=='scopes'else 0 "
                                   'for key in support.FACTORY_RECORD_TABLES},\n',
                       'replacement': "    snapshot={'schema':16,'factory_counts':{key:2 if "
                                      "key=='scopes'else 0 for key in support.FACTORY_RECORD_TABLES},\n"},
                      {'start': 431,
                       'end': 432,
                       'expected': '    recovery_line="            '
                                   "recover=goal_panel.get_by_role('button',name='Recover the exact child "
                                   "Goal request',exact=True)\\n            "
                                   'wait_graph_recovery_enabled(recover,lambda '
                                   'timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)\\n            '
                                   'recover.click()\\n"\n',
                       'replacement': '    recovery_line="            '
                                      "recover=goal_panel.get_by_role('button',name='Recover the exact child "
                                      'Goal '
                                      'request\',exact=True);expect(recover).to_be_enabled(timeout=15000);recover.click()\\n"\n'},
                      {'start': 433,
                       'end': 443,
                       'expected': '    observation_call="wait_graph_recovery_enabled(recover,lambda '
                                   'timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)"\n'
                                   '    mutations={\n'
                                   '        '
                                   "'implicit-default5s':source.replace(recovery_line,recovery_line.replace(observation_call,'expect(recover).to_be_enabled()')),\n"
                                   "        'expanded16s':source.replace('deadline=time.monotonic()+15\\n    "
                                   "def read_enabled','deadline=time.monotonic()+16\\n    def "
                                   "read_enabled'),\n"
                                   '        '
                                   "'zero-observation':source.replace('deadline=time.monotonic()+15\\n    "
                                   "def read_enabled','deadline=time.monotonic()+0\\n    def "
                                   "read_enabled'),\n"
                                   '        '
                                   "'inexact-recovery-locator':source.replace(recovery_line,recovery_line.replace('exact=True','exact=False')),\n"
                                   '        '
                                   "'click-before-enabled':source.replace(recovery_line,recovery_line.replace('            "
                                   "'+observation_call+'\\n            recover.click()','            "
                                   "recover.click()\\n            '+observation_call)),\n"
                                   '    }\n'
                                   '    for name,mutated in mutations.items():\n'
                                   '        assert mutated!=source\n',
                       'replacement': '    for name,replacement in {\n'
                                      '        '
                                      "'implicit-default5s':recovery_line.replace('(timeout=15000)','()'),\n"
                                      '        '
                                      "'expanded16s':recovery_line.replace('timeout=15000','timeout=16000'),\n"
                                      '        '
                                      "'zero-observation':recovery_line.replace('timeout=15000','timeout=0'),\n"
                                      '        '
                                      "'inexact-recovery-locator':recovery_line.replace('exact=True','exact=False'),\n"
                                      '        '
                                      "'click-before-enabled':recovery_line.replace('expect(recover).to_be_enabled(timeout=15000);recover.click()','recover.click();expect(recover).to_be_enabled(timeout=15000)'),\n"
                                      '    }.items():\n'
                                      '        mutated=source.replace(recovery_line,replacement);assert '
                                      'mutated!=source\n'},
                      {'start': 493,
                       'end': 494,
                       'expected': "        'wait_graph_recovery_enabled(recover,lambda "
                                   "timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)\\nrecover.click()\\n').body\n",
                       'replacement': '        '
                                      "'expect(recover).to_be_enabled(timeout=15000)\\nrecover.click()\\n').body\n"},
                      {'start': 495,
                       'end': 502,
                       'expected': '    helper=next(n for n in ast.parse(source).body if '
                                   "isinstance(n,ast.FunctionDef)and n.name=='wait_graph_recovery_enabled')\n"
                                   '    end=helper.body[1]\n'
                                   '    assert isinstance(end,ast.Assign)and '
                                   "ast.unparse(end.targets[0])=='deadline'and "
                                   "ast.unparse(end.value)=='time.monotonic() + 15'\n"
                                   '    calls=[n for n in ast.walk(helper)if isinstance(n,ast.Call)and '
                                   "isinstance(n.func,ast.Name)and n.func.id=='wait']\n"
                                   '    assert len(calls)==1 and [(k.arg,ast.unparse(k.value))for k in '
                                   "calls[0].keywords]==[('seconds','15')]\n"
                                   '    probes=[n for n in ast.walk(helper)if isinstance(n,ast.Call)and '
                                   "isinstance(n.func,ast.Attribute)and n.func.attr=='is_enabled']\n"
                                   '    assert len(probes)==1 and [(k.arg,ast.unparse(k.value))for k in '
                                   "probes[0].keywords]==[('timeout','min(20, remaining)')]\n",
                       'replacement': ''},
                      {'start': 676,
                       'end': 830,
                       'expected': "GRAPH_OBSERVATION_ACK_CODE = r'''\n"
                                   'import hashlib,selectors,stat\n'
                                   '\n'
                                   'def graph_ack_identity(value):\n'
                                   " fields={'pid','ppid','start_ticks','uid','pgid'}\n"
                                   ' assert type(value)is dict and set(value)==fields\n'
                                   " assert all(type(value[k])is int for k in ('pid','ppid','uid','pgid'))\n"
                                   " assert value['pid']>1 and value['ppid']>0 and value['uid']>=0 and "
                                   "value['pgid']==value['pid']\n"
                                   " assert type(value['start_ticks'])is str and "
                                   "0<len(value['start_ticks'])<=32 and value['start_ticks'].isascii() and "
                                   "value['start_ticks'].isdigit()\n"
                                   ' return value\n'
                                   '\n'
                                   'def graph_ack_bytes(nonce,identity):\n'
                                   ' graph_ack_identity(identity)\n'
                                   ' assert type(nonce)is str and len(nonce)==32 and all(c in '
                                   "'0123456789abcdef'for c in nonce)\n"
                                   ' '
                                   "metadata=hashlib.sha256(json.dumps(identity,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()\n"
                                   ' '
                                   "value={'contract':'graph-observation-ack.v1','nonce':nonce,'pid':identity['pid'],\n"
                                   "  'start_ticks':identity['start_ticks'],'metadata_sha256':metadata}\n"
                                   ' '
                                   "raw=(json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False)+'\\n').encode()\n"
                                   ' assert 0<len(raw)<=4096\n'
                                   ' return raw\n'
                                   '\n'
                                   'def graph_ack_name(nonce,identity):\n'
                                   ' graph_ack_bytes(nonce,identity)\n'
                                   ' return '
                                   "'graph-'+nonce+'-'+str(identity['pid'])+'-'+identity['start_ticks']+'.fifo'\n"
                                   '\n'
                                   'def graph_ack_noinherit(fd):\n'
                                   ' try:\n'
                                   '  os.set_inheritable(fd,False)\n'
                                   '  assert os.get_inheritable(fd)is False\n'
                                   '  return fd\n'
                                   ' except BaseException:os.close(fd);raise\n'
                                   '\n'
                                   'def graph_ack_directory(path,pin=None):\n'
                                   " path=Path(path);assert path.is_absolute() and '..'not in path.parts\n"
                                   ' flags=os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW|os.O_CLOEXEC\n'
                                   " fd=graph_ack_noinherit(os.open('/',flags))\n"
                                   ' try:\n'
                                   '  for part in path.parts[1:]:\n'
                                   '   '
                                   'nxt=graph_ack_noinherit(os.open(part,flags,dir_fd=fd));os.close(fd);fd=nxt\n'
                                   '  info=os.fstat(fd)\n'
                                   '  assert stat.S_ISDIR(info.st_mode) and info.st_uid==os.geteuid() and '
                                   'stat.S_IMODE(info.st_mode)==0o700\n'
                                   '  '
                                   'actual=[info.st_dev,info.st_ino,info.st_uid,stat.S_IMODE(info.st_mode)]\n'
                                   '  if pin is not None:\n'
                                   '   assert type(pin)is list and len(pin)==4 and all(type(v)is int for v '
                                   'in pin) and actual==pin\n'
                                   '  return fd,actual\n'
                                   ' except BaseException:os.close(fd);raise\n'
                                   '\n'
                                   'def graph_ack_fifo_pin(info):\n'
                                   ' assert stat.S_ISFIFO(info.st_mode) and '
                                   'stat.S_IMODE(info.st_mode)==0o600\n'
                                   ' assert info.st_uid==os.geteuid() and info.st_nlink==1\n'
                                   ' return '
                                   '(info.st_dev,info.st_ino,info.st_mode,info.st_uid,info.st_nlink)\n'
                                   '\n'
                                   'def graph_ack_self_identity():\n'
                                   " with open('/proc/self/stat','rb')as stream:raw=stream.read(4097)\n"
                                   ' assert len(raw)<=4096\n'
                                   " tail=raw.rsplit(b')',1)[1].split();assert len(tail)>=20\n"
                                   ' return '
                                   "graph_ack_identity({'pid':os.getpid(),'ppid':int(tail[1]),'start_ticks':tail[19].decode('ascii'),\n"
                                   "  'uid':os.geteuid(),'pgid':os.getpgid(0)})\n"
                                   '\n'
                                   'def graph_ack_prepare(settings,request,started_ns):\n'
                                   ' # This is part of the original producing attempt, not a fresh '
                                   'observation budget.\n'
                                   " reservation=request['budget']['reservation'];assert "
                                   "reservation==request['node']['reservation']\n"
                                   " runtime=reservation['runtime_ms'];assert type(runtime)is int\n"
                                   " assert settings['case']in ('normal','lost-start','cancel') and "
                                   "runtime==(10000 if settings['case']=='cancel'else 4000)\n"
                                   ' assert type(started_ns)is int and started_ns>0\n'
                                   ' deadline=started_ns+runtime*1_000_000\n'
                                   " assert time.monotonic_ns()<deadline,'Graph observation ACK original "
                                   "attempt deadline expired'\n"
                                   ' '
                                   "identity=graph_ack_self_identity();nonce=settings['nonce'];name=graph_ack_name(nonce,identity)\n"
                                   ' '
                                   "parent,pin=graph_ack_directory(settings['graph_observation_directory'],settings['graph_observation_directory_pin'])\n"
                                   ' fd=None\n'
                                   ' try:\n'
                                   '  os.mkfifo(name,0o600,dir_fd=parent)\n'
                                   '  '
                                   'before=graph_ack_fifo_pin(os.stat(name,dir_fd=parent,follow_symlinks=False))\n'
                                   '  '
                                   'fd=graph_ack_noinherit(os.open(name,os.O_RDWR|os.O_NONBLOCK|os.O_NOFOLLOW|os.O_CLOEXEC,dir_fd=parent))\n'
                                   '  assert '
                                   'before==graph_ack_fifo_pin(os.fstat(fd))==graph_ack_fifo_pin(os.stat(name,dir_fd=parent,follow_symlinks=False))\n'
                                   '  return '
                                   "{'fd':fd,'parent':parent,'name':name,'pin':before,'deadline_ns':deadline,\n"
                                   "   'nonce':nonce,'identity':identity,'directory_pin':pin}\n"
                                   ' except BaseException:\n'
                                   '  if fd is not None:os.close(fd)\n'
                                   '  os.close(parent);raise\n'
                                   '\n'
                                   'def graph_ack_wait(channel):\n'
                                   " expected=graph_ack_bytes(channel['nonce'],channel['identity']);raw=b''\n"
                                   " assert os.get_inheritable(channel['fd'])is False\n"
                                   ' with selectors.DefaultSelector()as selector:\n'
                                   "  selector.register(channel['fd'],selectors.EVENT_READ)\n"
                                   '  while True:\n'
                                   "   remaining=channel['deadline_ns']-time.monotonic_ns()\n"
                                   "   assert remaining>0,'Graph observation ACK original attempt deadline "
                                   "expired'\n"
                                   '   events=selector.select(remaining/1_000_000_000)\n'
                                   "   assert time.monotonic_ns()<channel['deadline_ns'],'Graph observation "
                                   "ACK arrived too late'\n"
                                   '   if not events:continue\n'
                                   "   assert len(events)==1 and events[0][0].fd==channel['fd']\n"
                                   "   block=os.read(channel['fd'],4097-len(raw));assert block,'Graph "
                                   "observation ACK pipe closed'\n"
                                   '   raw+=block;assert len(raw)<=4096\n'
                                   '   assert '
                                   "channel['pin']==graph_ack_fifo_pin(os.fstat(channel['fd']))==graph_ack_fifo_pin(os.stat(channel['name'],dir_fd=channel['parent'],follow_symlinks=False))\n"
                                   "   if b'\\n'in raw:\n"
                                   "    assert raw==expected,'Graph observation ACK binding refused'\n"
                                   "    assert graph_ack_self_identity()==channel['identity'],'Graph "
                                   "observation identity changed'\n"
                                   "    assert time.monotonic_ns()<channel['deadline_ns'],'Graph observation "
                                   "ACK verification arrived too late'\n"
                                   '    return\n'
                                   '\n'
                                   'def graph_ack_close(channel):\n'
                                   " try:os.close(channel['fd'])\n"
                                   " finally:os.close(channel['parent'])\n"
                                   "'''\n"
                                   'exec(GRAPH_OBSERVATION_ACK_CODE)\n'
                                   '\n'
                                   '\n'
                                   'def publish_graph_binding_ack(fixed,context):\n'
                                   '    """Root host only, after actual strong initial binding; repeat never '
                                   'calls."""\n'
                                   "    identity=context['_graph_provider_identities'].get(fixed['path'])\n"
                                   '    require(graph_valid_identity(identity) and '
                                   "identity['uid']==os.geteuid()\n"
                                   "        and all(identity[k]==fixed['owned_provider'][k]for k in "
                                   "('pid','pgid','start_ticks')))\n"
                                   "    sent=context.setdefault('_graph_observation_ack_sent',set())\n"
                                   "    require(fixed['path']not in sent,'Graph observation ACK already "
                                   "sent')\n"
                                   '    '
                                   "raw=graph_ack_bytes(context['nonce'],identity);name=graph_ack_name(context['nonce'],identity)\n"
                                   '    '
                                   "parent,pin=graph_ack_directory(context['graph_observation_directory'],context['graph_observation_directory_pin'])\n"
                                   '    fd=None\n'
                                   '    try:\n'
                                   '        '
                                   'before=graph_ack_fifo_pin(os.stat(name,dir_fd=parent,follow_symlinks=False))\n'
                                   '        '
                                   'fd=graph_ack_noinherit(os.open(name,os.O_WRONLY|os.O_NONBLOCK|os.O_NOFOLLOW|os.O_CLOEXEC,dir_fd=parent))\n'
                                   '        require(before==graph_ack_fifo_pin(os.fstat(fd)))\n'
                                   "        require(len(raw)<=os.fpathconf(fd,'PC_PIPE_BUF'))\n"
                                   "        receipt=name[:-5]+'.ack.json'\n"
                                   '        '
                                   'output=graph_ack_noinherit(os.open(receipt,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW|os.O_CLOEXEC,0o600,dir_fd=parent))\n'
                                   "        with os.fdopen(output,'wb')as stream:\n"
                                   '            stream.write(raw);stream.flush();os.fsync(stream.fileno())\n'
                                   "        require(os.write(fd,raw)==len(raw),'Graph observation ACK pipe "
                                   "write incomplete')\n"
                                   '        '
                                   'require(before==graph_ack_fifo_pin(os.stat(name,dir_fd=parent,follow_symlinks=False)))\n'
                                   '        '
                                   "check,actual=graph_ack_directory(context['graph_observation_directory'],pin);os.close(check)\n"
                                   "        require(actual==pin);sent.add(fixed['path'])\n"
                                   '    finally:\n'
                                   '        if fd is not None:os.close(fd)\n'
                                   '        os.close(parent)\n'
                                   '\n'
                                   '\n'
                                   'def '
                                   'wait_graph_recovery_enabled(locator,assert_enabled,timeout_error,*,wait):\n'
                                   '    """Keep capture active during the original15s readonly enabled '
                                   'observation."""\n'
                                   '    deadline=time.monotonic()+15\n'
                                   '    def read_enabled():\n'
                                   '        remaining=int((deadline-time.monotonic())*1000)\n'
                                   "        require(remaining>0,'Original recovery enabled observation "
                                   "expired')\n"
                                   '        try:enabled=locator.is_enabled(timeout=min(20,remaining))\n'
                                   '        except timeout_error:return False\n'
                                   '        require(type(enabled)is bool and time.monotonic()<deadline)\n'
                                   '        if enabled:\n'
                                   '            remaining=int((deadline-time.monotonic())*1000)\n'
                                   '            require(remaining>0);assert_enabled(remaining)\n'
                                   "            require(time.monotonic()<deadline,'Recovery enabled "
                                   "assertion arrived too late')\n"
                                   '        return enabled\n'
                                   '    return wait(read_enabled,lambda value:value is True,seconds=15)\n'
                                   '\n'
                                   '\n',
                       'replacement': ''},
                      {'start': 850,
                       'end': 862,
                       'expected': '    graph_channel=graph_ack_prepare(settings,c,FIXTURE_STARTED_NS)\n'
                                   '    try:\n'
                                   '     '
                                   "proof={'request_text':raw,'request_sha256':hashlib.sha256(raw.encode()).hexdigest(),\n"
                                   '      '
                                   "'owned_provider':identity(os.getpid()),'fixture_nonce':settings['nonce'],\n"
                                   '      '
                                   "'native_session_id':native_session_id,'program_sha256':file_hash(__file__),\n"
                                   '      '
                                   "'settings_sha256':file_hash(settings_path),'provider_argv':sys.argv,'mcp_servers':[]}\n"
                                   "     record='reports/graph-request-'+str(os.getpid())+'.json'\n"
                                   '     '
                                   "fd=os.open(record+'.tmp',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)\n"
                                   "     with os.fdopen(fd,'w')as f:json.dump(proof,f)\n"
                                   "     os.replace(record+'.tmp',record)\n"
                                   '     graph_ack_wait(graph_channel)\n'
                                   '    finally:graph_ack_close(graph_channel)\n',
                       'replacement': '    '
                                      "proof={'request_text':raw,'request_sha256':hashlib.sha256(raw.encode()).hexdigest(),\n"
                                      '     '
                                      "'owned_provider':identity(os.getpid()),'fixture_nonce':settings['nonce'],\n"
                                      '     '
                                      "'native_session_id':native_session_id,'program_sha256':file_hash(__file__),\n"
                                      '     '
                                      "'settings_sha256':file_hash(settings_path),'provider_argv':sys.argv,'mcp_servers':[]}\n"
                                      "    record='reports/graph-request-'+str(os.getpid())+'.json'\n"
                                      '    '
                                      "fd=os.open(record+'.tmp',os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)\n"
                                      "    with os.fdopen(fd,'w')as f:json.dump(proof,f)\n"
                                      "    os.replace(record+'.tmp',record)\n"},
                      {'start': 902,
                       'end': 903,
                       'expected': "        ('mcp=None;mid=0', EXEC_PLAN_HELPER + '\\n' + "
                                   "GRAPH_OBSERVATION_ACK_CODE + '\\nmcp=None;mid=0'),\n",
                       'replacement': "        ('mcp=None;mid=0', EXEC_PLAN_HELPER + "
                                      "'\\nmcp=None;mid=0'),\n"},
                      {'start': 948,
                       'end': 949,
                       'expected': '        '
                                   "tables=set(support.FACTORY_RECORD_TABLES.values())|set(support.INTEGRATION_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|{'events','attempts','sessions','dispatch_intents','workspace_leases','budgets','execution_grants'}\n",
                       'replacement': '        '
                                      "tables=set(support.FACTORY_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|{'events','attempts','sessions','dispatch_intents','workspace_leases','budgets','execution_grants'}\n"},
                      {'start': 957,
                       'end': 1106,
                       'expected': '    return '
                                   "{'schema':17,'factory_counts':counts,'tables':result,'sequence':sequence,'active_workspace_leases':active_leases,'active_dispatch_intents':active_intents}\n"
                                   '\n'
                                   '\n'
                                   "GRAPH_IDENTITY_FIELDS = ('pid', 'ppid', 'start_ticks', 'uid', 'pgid')\n"
                                   "GRAPH_LIVE_STATES = frozenset(('R', 'S', 'D', 'T', 't', 'I', 'P', 'W'))\n"
                                   "GRAPH_TERMINAL_STATES = frozenset(('Z', 'X', 'x'))\n"
                                   '\n'
                                   '\n'
                                   'def graph_identity_sample(pid):\n'
                                   '    """Bounded readonly metadata only; absent and unobserved stay '
                                   'distinct."""\n'
                                   '    try:\n'
                                   '        value = support.proc_identity(pid)\n'
                                   "        return {'observation': 'observed', 'pid': value['pid'], 'ppid': "
                                   "value['parent_pid'],\n"
                                   "            'start_ticks': value['start_ticks'], 'uid': Path('/proc', "
                                   'str(pid)).stat().st_uid,\n'
                                   "            'pgid': os.getpgid(pid), 'state': value['state']}\n"
                                   '    except (FileNotFoundError, ProcessLookupError):\n'
                                   "        return {'observation': 'absent'}\n"
                                   '    except (OSError, ValueError, TypeError, KeyError, AssertionError):\n'
                                   "        return {'observation': 'unobserved'}\n"
                                   '\n'
                                   '\n'
                                   'def graph_valid_identity(value):\n'
                                   '    return (type(value) is dict and set(value) == '
                                   'set(GRAPH_IDENTITY_FIELDS)\n'
                                   "        and all(type(value[k]) is int for k in ('pid', 'ppid', 'uid', "
                                   "'pgid'))\n"
                                   "        and value['pid'] > 1 and value['ppid'] > 0 and value['uid'] >= 0 "
                                   "and value['pgid'] == value['pid']\n"
                                   "        and type(value['start_ticks']) is str and "
                                   "re.fullmatch('[0-9]{1,32}', value['start_ticks']) is not None)\n"
                                   '\n'
                                   '\n'
                                   'def graph_sample_identity(value):\n'
                                   "    if not (type(value) is dict and set(value) == {'observation', "
                                   "'state'} | set(GRAPH_IDENTITY_FIELDS)\n"
                                   "            and value['observation'] == 'observed' and "
                                   "type(value['state']) is str\n"
                                   "            and value['state'] in GRAPH_LIVE_STATES | "
                                   'GRAPH_TERMINAL_STATES):\n'
                                   '        return None\n'
                                   '    identity = {key: value[key] for key in GRAPH_IDENTITY_FIELDS}\n'
                                   '    return identity if graph_valid_identity(identity) else None\n'
                                   '\n'
                                   '\n'
                                   'def graph_argv_observation(raw, expected):\n'
                                   '    """Finite grammar observation; no raw arguments/messages leave this '
                                   'helper."""\n'
                                   '    if type(raw) is not bytes:\n'
                                   "        return {'kind': 'unobserved', 'bytes': None}\n"
                                   '    size = len(raw)\n'
                                   "    if size > 8192: return {'kind': 'oversize', 'bytes': 8193}\n"
                                   "    if size == 0: return {'kind': 'empty', 'bytes': 0}\n"
                                   "    if not raw.endswith(b'\\0'): return {'kind': 'missing_nul', 'bytes': "
                                   'size}\n'
                                   "    try: argv = [value.decode('utf-8') for value in "
                                   "raw[:-1].split(b'\\0')]\n"
                                   "    except UnicodeDecodeError: return {'kind': 'invalid_utf8', 'bytes': "
                                   'size}\n'
                                   '    valid = (type(expected) is list and len(expected) == 3\n'
                                   '        and all(type(value) is str and value for value in expected) and '
                                   'argv[-3:] == expected)\n'
                                   "    return {'kind': 'valid' if valid else 'mismatch', 'bytes': size}\n"
                                   '\n'
                                   '\n'
                                   'def graph_binding_decision(expected, known, before, argv, after, uid):\n'
                                   '    """Retain an initial binding only; no branch proves business stop or '
                                   'join."""\n'
                                   "    refused = {'kind': 'not_verified', 'binding': None, 'stop_proved': "
                                   'False}\n'
                                   "    if not (type(expected) is dict and set(expected) == {'pid', 'pgid', "
                                   "'start_ticks'}\n"
                                   '            and type(uid) is int and uid >= 0): return refused\n'
                                   '    sample_before, sample_after = graph_sample_identity(before), '
                                   'graph_sample_identity(after)\n'
                                   '    def matches(value):\n'
                                   "        return (value is not None and value['uid'] == uid\n"
                                   '            and all(type(value[k]) is type(expected[k]) and value[k] == '
                                   'expected[k] for k in expected))\n'
                                   "    if not (type(argv) is dict and set(argv) == {'kind', 'bytes'}): "
                                   'return refused\n'
                                   "    kind, size = argv['kind'], argv['bytes']\n"
                                   "    if kind in ('not_read', 'unobserved'):\n"
                                   '        if size is not None: return refused\n'
                                   "    elif not (kind in ('valid', 'empty', 'oversize', 'missing_nul', "
                                   "'invalid_utf8', 'mismatch')\n"
                                   '            and type(size) is int and 0 <= size <= 8193): return '
                                   'refused\n'
                                   "    if kind == 'valid' and not 0 < size <= 8192: return refused\n"
                                   "    if kind == 'empty' and size != 0: return refused\n"
                                   "    if kind in ('oversize', 'missing_nul', 'invalid_utf8', 'mismatch'): "
                                   'return refused\n'
                                   '    if known is None:\n'
                                   '        if (matches(sample_before) and sample_before == sample_after and '
                                   "kind == 'valid'\n"
                                   "                and before['state'] in GRAPH_LIVE_STATES and "
                                   "after['state'] in GRAPH_LIVE_STATES):\n"
                                   "            return {'kind': 'initial_live', 'binding': "
                                   "copy.deepcopy(sample_before), 'stop_proved': False}\n"
                                   '        return refused\n'
                                   '    if not (graph_valid_identity(known) and matches(known)): return '
                                   'refused\n'
                                   "    before_absent = type(before) is dict and before == {'observation': "
                                   "'absent'}\n"
                                   "    after_absent = type(after) is dict and after == {'observation': "
                                   "'absent'}\n"
                                   '    if sample_before is None and not before_absent: return refused\n'
                                   '    if sample_after is None and not after_absent: return refused\n'
                                   '    if after_absent:\n'
                                   "        result = 'repeat_absent'\n"
                                   "    elif sample_after['pid'] == known['pid'] and "
                                   "sample_after['start_ticks'] != known['start_ticks']:\n"
                                   "        result = 'repeat_replaced'\n"
                                   '    elif sample_before == sample_after == known:\n'
                                   "        if after['state'] in GRAPH_TERMINAL_STATES:\n"
                                   "            if kind not in ('valid', 'empty', 'not_read'): return "
                                   'refused\n'
                                   "            result = 'repeat_terminal'\n"
                                   "        elif before['state'] in GRAPH_LIVE_STATES and kind == 'valid':\n"
                                   "            result = 'repeat_live'\n"
                                   '        else: return refused\n'
                                   '    else: return refused\n'
                                   "    return {'kind': result, 'binding': None, 'stop_proved': False}\n"
                                   '\n'
                                   '\n'
                                   'def observe_graph_identity(expected, expected_argv, known):\n'
                                   '    """Two metadata samples surround a bounded read of only a bound live '
                                   'PID."""\n'
                                   "    before = graph_identity_sample(expected['pid'])\n"
                                   '    identity = graph_sample_identity(before)\n'
                                   "    argv = {'kind': 'not_read', 'bytes': None}\n"
                                   "    if (identity is not None and before['state'] in GRAPH_LIVE_STATES "
                                   "and identity['uid'] == os.geteuid()\n"
                                   '            and all(identity[k] == expected[k] for k in expected) and '
                                   '(known is None or identity == known)):\n'
                                   '        try:\n'
                                   "            with Path('/proc', str(expected['pid']), "
                                   "'cmdline').open('rb') as stream:\n"
                                   '                raw = stream.read(8193)\n'
                                   '        except OSError: raw = None\n'
                                   '        argv = graph_argv_observation(raw, expected_argv)\n'
                                   "    after = graph_identity_sample(expected['pid'])\n"
                                   '    result = graph_binding_decision(expected, known, before, argv, '
                                   'after, os.geteuid())\n'
                                   "    return result, {'kind': result['kind'], 'before': before, 'argv': "
                                   "argv, 'after': after,\n"
                                   "        'initial_binding_verified': known is not None, 'stop_proved': "
                                   'False}\n'
                                   '\n'
                                   '\n'
                                   'def bind_graph_observation(fixed, expected_argv, old, context, output, '
                                   'finite=None, cancel_durable=None):\n'
                                   '    """First observation must prove live identity; repeat never creates '
                                   'one."""\n'
                                   "    history = context.setdefault('_graph_provider_identities', {})\n"
                                   "    known = history.get(fixed['path'])\n"
                                   "    require((old is None) == (known is None), 'Graph initial binding "
                                   "coverage not verified')\n"
                                   '    result, observation = '
                                   "observe_graph_identity(fixed['owned_provider'], expected_argv, known)\n"
                                   "    seen = context.setdefault('_graph_provider_observation_kinds', "
                                   'set())\n'
                                   "    key = (fixed['path'], result['kind'])\n"
                                   '    if key not in seen:\n'
                                   "        require(len(seen) < 12, 'Bounded Graph observation kinds "
                                   "exceeded')\n"
                                   "        base.write_private_json(output / ('observed-graph-' + "
                                   "str(fixed['owned_provider']['pid']) + '-' + result['kind'] + '.json'), "
                                   'observation)\n'
                                   '        seen.add(key)\n'
                                   '    graph_finite_observation(finite, known, result, observation, '
                                   'cancel_durable)\n'
                                   "    require(result['kind'] != 'not_verified', 'Graph process observation "
                                   "not verified')\n"
                                   '    if old is None:\n'
                                   "        require(result['kind'] == 'initial_live' and "
                                   "graph_valid_identity(result['binding']) and len(history) < 2)\n"
                                   "        base.write_private_json(output / ('verified-graph-identity-' + "
                                   "str(fixed['owned_provider']['pid']) + '.json'), result['binding'])\n"
                                   "        history[fixed['path']] = result['binding']\n"
                                   '        publish_graph_binding_ack(fixed,context)\n'
                                   '    else:\n'
                                   "        require(result['kind'] in ('repeat_live', 'repeat_absent', "
                                   "'repeat_replaced', 'repeat_terminal')\n"
                                   "            and result['binding'] is None)\n"
                                   '\n'
                                   '\n'
                                   'def graph_finite_observation(finite, known, result, observation, '
                                   'cancel_durable):\n'
                                   '    """Project the existing two samples; never re-read a PID or change '
                                   'its guard."""\n'
                                   '    if finite is None:return\n'
                                   "    kind=observation['argv']['kind']\n"
                                   "    if kind=='empty':\n"
                                   '        durable=cancel_durable()if cancel_durable is not None else None\n'
                                   '        '
                                   "finite.graph_identity(graph_empty_identity_diagnostic(known,observation['before'],observation['after'],durable))\n"
                                   '    '
                                   "reasons={'empty':'empty','oversize':'oversized','missing_nul':'missing_nul','mismatch':'argv_mismatch'}\n"
                                   '    # A previously strong-bound terminal/absent observation remains only '
                                   'that\n'
                                   '    # observation. Finite argv failure describes an original guard '
                                   'refusal.\n'
                                   "    if result['kind']=='not_verified'and kind in reasons:\n"
                                   '        '
                                   "finite.graph_argv({'reason':reasons[kind],'previously_bound':known is "
                                   'not None})\n',
                       'replacement': '    return '
                                      "{'schema':16,'factory_counts':counts,'tables':result,'sequence':sequence,'active_workspace_leases':active_leases,'active_dispatch_intents':active_intents}\n"},
                      {'start': 1229,
                       'end': 1231,
                       'expected': '            '
                                   "bind_graph_observation(fixed,row['provider_argv'],old,context,output,finite,\n"
                                   '                '
                                   'lambda:observe_exact_graph_cancel(db,scope,context,fixed))\n',
                       'replacement': "            try:current=support.proc_identity(identity['pid'])\n"
                                      '            except FileNotFoundError:current=None\n'
                                      '            if current is not None and '
                                      "current['start_ticks']==identity['start_ticks']:\n"
                                      '                '
                                      "identity_before=graph_identity_snapshot(identity['pid'],current)\n"
                                      '                '
                                      "argv=Path('/proc',str(identity['pid']),'cmdline').read_bytes()\n"
                                      '                '
                                      "identity_after=graph_identity_snapshot(identity['pid'])\n"
                                      '                '
                                      "history=context.setdefault('_graph_provider_identities',{})\n"
                                      "                if argv==b''and finite is not None:\n"
                                      '                    '
                                      "finite.graph_identity(graph_empty_identity_diagnostic(history.get(fixed['path']),identity_before,identity_after,\n"
                                      '                        '
                                      'observe_exact_graph_cancel(db,scope,context,fixed)))\n'
                                      '                '
                                      "finite_projection.check_graph_provider_argv(argv,row['provider_argv'],old "
                                      'is not None)\n'
                                      "                assert os.getpgid(identity['pid'])==identity['pgid']\n"
                                      '                '
                                      'initial=graph_identity_initial(identity,identity_before,identity_after,os.geteuid())\n'
                                      "                if fixed['path']not in history and initial is not "
                                      'None and len(history)<2:\n'
                                      "                    history[fixed['path']]=initial\n"
                                      '                    '
                                      "base.write_private_json(output/('verified-graph-identity-'+str(identity['pid'])+'.json'),initial)\n"},
                      {'start': 1238,
                       'end': 1239,
                       'expected': 'PREAPPROVAL_ZERO_TABLES = '
                                   '(frozenset(support.FACTORY_RECORD_TABLES.values())|frozenset(support.INTEGRATION_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|\n',
                       'replacement': 'PREAPPROVAL_ZERO_TABLES = '
                                      '(frozenset(support.FACTORY_RECORD_TABLES.values())|support.LEGACY_GOAL_TABLES|\n'},
                      {'start': 1246,
                       'end': 1247,
                       'expected': "    require(type(snapshot['schema'])is int and snapshot['schema']==17)\n",
                       'replacement': "    require(type(snapshot['schema'])is int and "
                                      "snapshot['schema']==16)\n"},
                      {'start': 1631,
                       'end': 1632,
                       'expected': "    require(type(manifest['schema'])is int and manifest['schema']==17)\n",
                       'replacement': "    require(type(manifest['schema'])is int and "
                                      "manifest['schema']==16)\n"},
                      {'start': 1741,
                       'end': 1748,
                       'expected': "        graph_observation_directory=root/'graph-observation'\n"
                                   '        graph_observation_directory.mkdir(mode=0o700)\n'
                                   '        require(not '
                                   'graph_observation_directory.is_relative_to(wroot.resolve(strict=True))\n'
                                   '            and not '
                                   'graph_observation_directory.is_relative_to(source_root.resolve(strict=True)))\n'
                                   '        '
                                   'graph_directory_fd,graph_directory_pin=graph_ack_directory(graph_observation_directory)\n'
                                   '        os.close(graph_directory_fd)\n'
                                   '        '
                                   "settings_value={'graph_observation_directory':str(graph_observation_directory),'graph_observation_directory_pin':graph_directory_pin,'case':args.case,'task_id':task['id'],'connection_id':cid,'workspace_id':workspace['id'],'companion':str(companion),'nonce':uuid.uuid4().hex}\n",
                       'replacement': '        '
                                      "settings_value={'case':args.case,'task_id':task['id'],'connection_id':cid,'workspace_id':workspace['id'],'companion':str(companion),'nonce':uuid.uuid4().hex}\n"},
                      {'start': 1749,
                       'end': 1750,
                       'expected': '        '
                                   "record_context.update(graph_observation_directory=str(graph_observation_directory),graph_observation_directory_pin=graph_directory_pin,nonce=settings_value['nonce'],program_sha256=base.sha(provider.read_bytes()),settings_sha256=base.sha(settings.read_bytes()),connection_id=cid,provider=str(provider),settings=str(settings),companion=str(companion),companion_sha256=base.sha(companion.read_bytes()))\n",
                       'replacement': '        '
                                      "record_context.update(nonce=settings_value['nonce'],program_sha256=base.sha(provider.read_bytes()),settings_sha256=base.sha(settings.read_bytes()),connection_id=cid,provider=str(provider),settings=str(settings),companion=str(companion),companion_sha256=base.sha(companion.read_bytes()))\n"},
                      {'start': 1969,
                       'end': 1972,
                       'expected': "            recover=goal_panel.get_by_role('button',name='Recover the "
                                   "exact child Goal request',exact=True)\n"
                                   '            wait_graph_recovery_enabled(recover,lambda '
                                   'timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)\n'
                                   '            recover.click()\n',
                       'replacement': "            recover=goal_panel.get_by_role('button',name='Recover the "
                                      'exact child Goal '
                                      "request',exact=True);expect(recover).to_be_enabled(timeout=15000);recover.click()\n"},
                      {'start': 2102,
                       'end': 2103,
                       'expected': "                require(info['complete']is True and "
                                   "info['schema']==17)\n",
                       'replacement': "                require(info['complete']is True and "
                                      "info['schema']==16)\n"},
                      {'start': 2247,
                       'end': 2249,
                       'expected': "            'Core17 exact lifecycle success/failure/stop-chain fixtures "
                                   "and independent review',\n"
                                   "            'Fresh immutable Core17 tools/source manifest and composed "
                                   "frontend assets',\n",
                       'replacement': "            'Core16 exact lifecycle success/failure/stop-chain "
                                      "fixtures and independent review',\n"
                                      "            'Fresh immutable Core16 tools/source manifest and "
                                      "composed frontend assets',\n"},
                      {'start': 2251,
                       'end': 2252,
                       'expected': '    '
                                   'value.update(schema_required=17,harness_sha256=base.sha(Path(__file__).read_bytes()),\n',
                       'replacement': '    '
                                      'value.update(schema_required=16,harness_sha256=base.sha(Path(__file__).read_bytes()),\n'}]}}


def strip_composed_outer_layer(text, side='ours'):
    if type(text)is not str or type(side)is not str or side not in _COMPOSED_DRIVER_INVERSES:
        raise AssertionError('Exact composed driver inverse arguments refused')
    fixture=_COMPOSED_DRIVER_INVERSES[side]
    digest=sha(text.encode())
    if digest==fixture['restored_sha256']:
        return text  # Only the exact already-restored historical target is valid.
    if digest!=fixture['merged_sha256']:
        raise AssertionError('Unknown composed driver source refused')
    lines=text.splitlines(keepends=True)
    for edit in reversed(fixture['edits']):
        if ''.join(lines[edit['start']:edit['end']])!=edit['expected']:
            raise AssertionError('Exact composed driver inverse anchor refused')
        lines[edit['start']:edit['end']]=edit['replacement'].splitlines(keepends=True)
    restored=''.join(lines)
    if sha(restored.encode())!=fixture['restored_sha256']:
        raise AssertionError('Exact complete historical driver refused')
    return restored


def strip_handshake_layer(text):
    text=strip_composed_outer_layer(text,'ours')
    fixture=json.loads((ROOT/'tests/infrastructure/fixtures/graph_observation_handshake/source_inverse.json').read_text())
    start=text.index('GRAPH_OBSERVATION_ACK_CODE = ');end=text.index('def generated_acp():',start)
    if sha(text[start:end].encode())!=fixture['addition_sha256']:raise AssertionError('Exact handshake addition changed')
    text=text[:start]+text[end:]
    for before,after in reversed(fixture['replacements']):
        if text.count(after)!=1:raise AssertionError('Exact handshake inverse anchor changed')
        text=text.replace(after,before,1)
    return text


def shared():
    tree=ast.parse(DRIVER.read_text())
    node=next(n for n in tree.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='GRAPH_OBSERVATION_ACK_CODE'for t in n.targets))
    code=ast.literal_eval(node.value)
    ns={'os':os,'re':re,'Path':Path,'time':types.SimpleNamespace(monotonic_ns=lambda:1_000_000_000),'json':json}
    exec(compile(code,'<fixed-generated-ACK-helper>','exec'),ns)
    return ns,code


def function(ns,name):
    node=next(n for n in ast.parse(DRIVER.read_text()).body if isinstance(n,ast.FunctionDef)and n.name==name)
    exec(compile(ast.Module(body=[node],type_ignores=[]),'<fixed-host-function>','exec'),ns)
    return ns[name]


def require(value,*_):
    if not value:raise AssertionError('Fixed handshake refused')


def fifo_info(**changes):
    values={'st_dev':1,'st_ino':3,'st_uid':1000,'st_mode':stat.S_IFIFO|0o600,'st_nlink':1}
    values.update(changes);return types.SimpleNamespace(**values)


class CodecAndMetadata(unittest.TestCase):
    def test_ack_binds_nonce_pid_birth_and_complete_initial_metadata_digest(self):
        ns,_=shared();raw=ns['graph_ack_bytes'](NONCE,IDENTITY);value=json.loads(raw)
        self.assertEqual(set(value),{'contract','nonce','pid','start_ticks','metadata_sha256'})
        self.assertEqual(value['metadata_sha256'],sha(json.dumps(IDENTITY,sort_keys=True,separators=(',',':')).encode()))
        self.assertLessEqual(len(raw),4096)
        for key,bad in [('pid',2002),('start_ticks','4568'),('ppid',1999),('uid',1001)]:
            changed=dict(IDENTITY,**{key:bad})
            if key=='pid':changed['pgid']=bad
            self.assertNotEqual(ns['graph_ack_bytes'](NONCE,changed),raw)
        for nonce in ['',True,'A'*32,'a'*33,'a'*31]:
            with self.assertRaises(AssertionError):ns['graph_ack_bytes'](nonce,IDENTITY)
        for key,bad in [('pid',True),('ppid',0),('uid',False),('pgid',2002),('start_ticks',1)]:
            with self.assertRaises(AssertionError):ns['graph_ack_bytes'](NONCE,dict(IDENTITY,**{key:bad}))

    def test_fifo_type_mode_owner_links_and_noninheritance_are_strict(self):
        ns,_=shared();fake=types.SimpleNamespace(geteuid=lambda:1000,set_inheritable=mock.Mock(),get_inheritable=lambda fd:False,close=mock.Mock())
        ns['os']=fake;self.assertEqual(ns['graph_ack_noinherit'](20),20);fake.set_inheritable.assert_called_once_with(20,False)
        self.assertEqual(ns['graph_ack_fifo_pin'](fifo_info()),(1,3,stat.S_IFIFO|0o600,1000,1))
        for info in [fifo_info(st_mode=stat.S_IFLNK|0o600),fifo_info(st_mode=stat.S_IFREG|0o600),
            fifo_info(st_mode=stat.S_IFIFO|0o644),fifo_info(st_uid=1001),fifo_info(st_nlink=2)]:
            with self.assertRaises(AssertionError):ns['graph_ack_fifo_pin'](info)
        fake.get_inheritable=lambda fd:True
        with self.assertRaises(AssertionError):ns['graph_ack_noinherit'](20)

    def test_self_metadata_uses_bounded_kernel_shape_not_a_generated_record(self):
        ns,_=shared();tail=['S','2000']+['0']*17+['4567']
        raw=('2001 (synthetic) '+' '.join(tail)).encode();reads=[]
        class Stream:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def read(self,n):reads.append(n);return raw
        ns['open']=mock.Mock(return_value=Stream())
        ns['os']=types.SimpleNamespace(getpid=lambda:2001,geteuid=lambda:1000,getpgid=lambda pid:2001)
        self.assertEqual(ns['graph_ack_self_identity'](),IDENTITY)
        ns['open'].assert_called_once_with('/proc/self/stat','rb');self.assertEqual(reads,[4097])
        raw=b'x'*4097
        with self.assertRaises(AssertionError):ns['graph_ack_self_identity']()

    def test_directory_descriptor_walk_refuses_alias_replacement_wrong_mode_and_bool_pin(self):
        ns,_=shared();opened=[];closed=[]
        def open_path(name,flags,**kwargs):opened.append((name,flags,kwargs));return len(opened)+10
        info=types.SimpleNamespace(st_mode=stat.S_IFDIR|0o700,st_uid=1000,st_dev=1,st_ino=2)
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_RDONLY','O_DIRECTORY','O_NOFOLLOW','O_CLOEXEC')},
            open=open_path,close=closed.append,fstat=lambda fd:info,geteuid=lambda:1000,
            set_inheritable=lambda *a:None,get_inheritable=lambda fd:False)
        ns['os']=fake
        fd,pin=ns['graph_ack_directory']('/synthetic/owned',PIN)
        self.assertEqual(pin,PIN);self.assertEqual([n for n,_,_ in opened],['/','synthetic','owned'])
        self.assertTrue(all(flags&os.O_NOFOLLOW and flags&os.O_CLOEXEC for _,flags,_ in opened))
        for bad in [[1,3,1000,0o700],[True,2,1000,0o700]]:
            with self.assertRaises(AssertionError):ns['graph_ack_directory']('/synthetic/owned',bad)
        info.st_mode=stat.S_IFDIR|0o755
        with self.assertRaises(AssertionError):ns['graph_ack_directory']('/synthetic/owned')
        fake.open=mock.Mock(side_effect=OSError('Alias refused by NOFOLLOW'))
        with self.assertRaises(OSError):ns['graph_ack_directory']('/synthetic/alias')


class ProviderEventWait(unittest.TestCase):
    def environment(self,chunks,late=False):
        ns,code=shared();clock=types.SimpleNamespace(value=1_000_000_000);events=[];blocks=list(chunks)
        channel={'fd':20,'parent':10,'name':'fifo','pin':(1,3,stat.S_IFIFO|0o600,1000,1),
            'deadline_ns':5_000_000_000,'nonce':NONCE,'identity':copy.deepcopy(IDENTITY)}
        class Selector:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def register(self,fd,mask):events.append(('registered',fd,mask))
            def select(self,timeout):
                events.append(('select',timeout))
                if not blocks or late:clock.value+=int(timeout*1e9);return []if not blocks else[(types.SimpleNamespace(fd=20),1)]
                clock.value+=1_000_000;return[(types.SimpleNamespace(fd=20),1)]
        ns['selectors']=types.SimpleNamespace(DefaultSelector=Selector,EVENT_READ=1)
        ns['time']=types.SimpleNamespace(monotonic_ns=lambda:clock.value)
        ns['os']=types.SimpleNamespace(get_inheritable=lambda fd:False,geteuid=lambda:1000,
            read=lambda fd,n:(events.append(('read',n)),blocks.pop(0))[1],fstat=lambda fd:fifo_info(),stat=lambda *a,**k:fifo_info())
        ns['graph_ack_self_identity']=lambda:copy.deepcopy(IDENTITY)
        return ns,channel,clock,events,code

    def test_exact_and_fragmented_event_ack_stay_within_same_original_deadline(self):
        proto,_=shared();raw=proto['graph_ack_bytes'](NONCE,IDENTITY)
        for chunks in [[raw],[raw[:7],raw[7:]]]:
            ns,channel,clock,events,code=self.environment(chunks)
            ns['graph_ack_wait'](channel);self.assertLess(clock.value,channel['deadline_ns'])
            self.assertTrue(all(e[1]<=4 for e in events if e[0]=='select'))
            self.assertNotIn('time.sleep',code)

    def test_wrong_nonce_birth_metadata_extra_truncated_or_late_ack_fails(self):
        proto,_=shared();raw=proto['graph_ack_bytes'](NONCE,IDENTITY);value=json.loads(raw)
        bad=[]
        for key,replacement in [('nonce','b'*32),('pid',2002),('start_ticks','4568'),('metadata_sha256','0'*64),('extra','unknown')]:
            changed=dict(value,**{key:replacement});bad.append((json.dumps(changed)+'\n').encode())
        bad.extend([b'x'*4097,b'',raw[:-1]])
        for packet in bad:
            ns,channel,_,_,_=self.environment([packet])
            with self.subTest(size=len(packet)),self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        for chunks,late in [([],False),([raw],True)]:
            ns,channel,_,_,_=self.environment(chunks,late)
            with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        ns,channel,_,_,_=self.environment([raw]);ns['graph_ack_self_identity']=lambda:dict(IDENTITY,ppid=1999)
        with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)
        ns,channel,clock,_,_=self.environment([raw])
        def late_identity():clock.value=channel['deadline_ns'];return copy.deepcopy(IDENTITY)
        ns['graph_ack_self_identity']=late_identity
        with self.assertRaises(AssertionError):ns['graph_ack_wait'](channel)

    def test_prepare_uses_original_started_epoch_and_reservation_with_closed_fds(self):
        ns,_=shared();opened=[];closed=[];mkfifo=mock.Mock()
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_RDWR','O_NONBLOCK','O_NOFOLLOW','O_CLOEXEC')},
            mkfifo=mkfifo,stat=lambda *a,**k:fifo_info(),open=lambda *a,**k:(opened.append((a,k)),20)[1],
            fstat=lambda fd:fifo_info(),geteuid=lambda:1000,close=closed.append,
            set_inheritable=mock.Mock(),get_inheritable=lambda fd:False)
        ns['os']=fake;ns['graph_ack_self_identity']=lambda:copy.deepcopy(IDENTITY)
        ns['graph_ack_directory']=mock.Mock(return_value=(10,PIN))
        for case,runtime in [('normal',4000),('lost-start',4000),('cancel',10000)]:
            settings={'case':case,'nonce':NONCE,'graph_observation_directory':'/synthetic','graph_observation_directory_pin':PIN}
            declared={'runtime_ms':runtime};request={'node':{'reservation':declared},'budget':{'reservation':declared}}
            channel=ns['graph_ack_prepare'](settings,request,500_000_000)
            self.assertEqual(channel['deadline_ns'],500_000_000+runtime*1_000_000)
            self.assertTrue(opened[-1][0][1]&os.O_NONBLOCK and opened[-1][0][1]&os.O_CLOEXEC)
            ns['graph_ack_close'](channel);self.assertEqual(closed[-2:],[20,10])
        mkfifo.side_effect=FileExistsError('Unique FIFO exists')
        with self.assertRaises(FileExistsError):ns['graph_ack_prepare'](settings,request,500_000_000)
        self.assertEqual(closed[-1],10)
        for runtime in (True,0,16000):
            request={'node':{'reservation':{'runtime_ms':runtime}},'budget':{'reservation':{'runtime_ms':runtime}}}
            with self.assertRaises(AssertionError):ns['graph_ack_prepare'](settings,request,500_000_000)


class HostAndReadonlyObservation(unittest.TestCase):
    def test_only_sealed_initial_binding_can_publish_one_atomic_noninherited_ack(self):
        ns,_=shared();writes=[];opened=[];closed=[]
        class Writer:
            def __enter__(self):return self
            def __exit__(self,*_):return False
            def write(self,raw):writes.append(('receipt',raw))
            def flush(self):pass
            def fileno(self):return 30
        fake=types.SimpleNamespace(**{k:getattr(os,k)for k in ('O_WRONLY','O_NONBLOCK','O_NOFOLLOW','O_CLOEXEC','O_CREAT','O_EXCL')},
            geteuid=lambda:1000,stat=lambda *a,**k:fifo_info(),fstat=lambda fd:fifo_info(),
            open=lambda *a,**k:(opened.append((a,k)),20 if len(opened)==1 else 30)[1],
            fpathconf=lambda *a:4096,fdopen=lambda *a:Writer(),fsync=lambda fd:None,
            write=lambda fd,raw:(writes.append(('pipe',raw)),len(raw))[1],close=closed.append,
            set_inheritable=mock.Mock(),get_inheritable=lambda fd:False)
        ns.update(os=fake,require=require,GRAPH_IDENTITY_FIELDS=('pid','ppid','start_ticks','uid','pgid'))
        function(ns,'graph_valid_identity');ns['graph_ack_directory']=mock.Mock(return_value=(10,PIN))
        publish=function(ns,'publish_graph_binding_ack');fixed={'path':'fixed','owned_provider':{k:IDENTITY[k]for k in ('pid','pgid','start_ticks')}}
        context={'nonce':NONCE,'_graph_provider_identities':{'fixed':copy.deepcopy(IDENTITY)},
            'graph_observation_directory':'/synthetic','graph_observation_directory_pin':PIN}
        for bad in [{},dict(context,_graph_provider_identities={}),dict(context,_graph_provider_identities={'fixed':dict(IDENTITY,uid=1001)})]:
            with self.assertRaises((AssertionError,KeyError)):publish(fixed,bad)
        self.assertEqual(writes,[])
        publish(fixed,context)
        self.assertEqual([x[0]for x in writes],['receipt','pipe']);self.assertEqual(writes[0][1],writes[1][1])
        self.assertTrue(opened[1][0][1]&os.O_EXCL and opened[1][0][1]&os.O_NOFOLLOW)
        with self.assertRaises(AssertionError):publish(fixed,context)
        self.assertEqual(len(writes),2)

    def test_ui_enabled_wait_keeps_existing_pump_capture_active_without_new_writes_or_deadline(self):
        ns={'time':types.SimpleNamespace(monotonic=lambda:clock[0]),'require':require};fn=function(ns,'wait_graph_recovery_enabled')
        clock=[0.0];captured=[];probes=[];assertions=[]
        class Pending(Exception):pass
        class Locator:
            def is_enabled(self,timeout):
                probes.append(timeout)
                if len(captured)==1:raise Pending()
                return len(captured)>=3
        def wait(read,predicate,seconds):
            self.assertEqual(seconds,15);end=clock[0]+seconds
            while True:
                captured.append(clock[0]);clock[0]+=.01
                value=read()
                if predicate(value):return value
                require(clock[0]<end);clock[0]+=.02
        self.assertTrue(fn(Locator(),assertions.append,Pending,wait=wait))
        self.assertEqual(len(captured),3);self.assertEqual(probes,[20]*3)
        self.assertEqual(len(assertions),1);self.assertLessEqual(assertions[0],15000);self.assertLess(clock[0],15)
        def late_wait(read,predicate,seconds):clock[0]+=15;return read()
        with self.assertRaises(AssertionError):fn(Locator(),assertions.append,Pending,wait=late_wait)

    def test_ui_late_enabled_reply_or_enabled_assertion_never_renews_window(self):
        for where in ('probe','assertion'):
            clock=[0.0];ns={'time':types.SimpleNamespace(monotonic=lambda:clock[0]),'require':require};fn=function(ns,'wait_graph_recovery_enabled')
            class Locator:
                def is_enabled(self,timeout):
                    if where=='probe':clock[0]+=15
                    return True
            def assertion(timeout):
                if where=='assertion':clock[0]+=15
            def wait(read,predicate,seconds):return read()
            with self.subTest(where=where),self.assertRaises(AssertionError):fn(Locator(),assertion,TimeoutError,wait=wait)


class SourceControls(unittest.TestCase):
    def test_exact_handshake_layer_inverse_preserves_all_old_harness_business_budget_and_cleanup(self):
        text=DRIVER.read_text();restored=strip_handshake_layer(text)
        self.assertEqual(sha(strip_composed_outer_layer(text,'ours').encode()),'4b0332152c2dff9e4d4d7df94dede8ce23b0735c1b76adbef14398c142811b88')
        self.assertEqual(sha(strip_composed_outer_layer(text,'theirs').encode()),'7dc4732f18be492fddcc49bb989f233cd4fc409bb11de33a7951f7d5cf36e338')
        for side in ('ours','theirs'):
            with self.assertRaises(AssertionError):strip_composed_outer_layer(text+'\n# unknown region\n',side)
        self.assertEqual(sha(restored.encode()),'b7cac9f5eda5d8933de6b739da65d2246063fea3ce29272cf33963ca0dc0239a')
        self.assertIn("publish_graph_binding_ack(fixed,context)",text)
        self.assertIn("graph_observation_directory=root/'graph-observation'",text)
        self.assertIn('not graph_observation_directory.is_relative_to(wroot.resolve(strict=True))',text)
        self.assertIn('not graph_observation_directory.is_relative_to(source_root.resolve(strict=True))',text)
        tree=ast.parse(text);fn=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='generated_acp')
        prompt=ast.literal_eval(next(n.value for n in fn.body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='graph_prompt'for t in n.targets)))
        self.assertLess(prompt.index('graph_ack_prepare'),prompt.index("os.replace(record+'.tmp',record)"))
        self.assertLess(prompt.index("os.replace(record+'.tmp',record)"),prompt.index('graph_ack_wait'))
        self.assertLess(prompt.index('graph_ack_close'),prompt.index("output=c['outputs']"))
        self.assertIn('graph_ack_prepare(settings,c,FIXTURE_STARTED_NS)',prompt)
        self.assertNotIn('publish_graph_binding_ack',prompt)
        bind=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='bind_graph_observation')
        branches=[n for n in ast.walk(bind)if isinstance(n,ast.If)and ast.unparse(n.test)=='old is None']
        calls=[n for n in ast.walk(branches[0])if isinstance(n,ast.Call)and isinstance(n.func,ast.Name)and n.func.id=='publish_graph_binding_ack']
        self.assertEqual(len(calls),1)

    def test_updated_lost_start_ast_guard_keeps_five_original_negative_classes_and_poll_slice(self):
        ns={};check=function(ns,'lost_start_observation_self_check');text=DRIVER.read_text();self.assertTrue(check(text))
        for old,new in [('deadline=time.monotonic()+15\n    def read_enabled','deadline=time.monotonic()+16\n    def read_enabled'),
            ('deadline=time.monotonic()+15\n    def read_enabled','deadline=time.monotonic()+0\n    def read_enabled'),
            ('timeout=min(20,remaining)','timeout=15000'),('seconds=15)\n\n\ndef generated_acp','seconds=16)\n\n\ndef generated_acp')]:
            self.assertIn(old,text)
            with self.subTest(old=old),self.assertRaises(AssertionError):check(text.replace(old,new,1))
        fixture=json.loads((ROOT/'tests/infrastructure/fixtures/graph_observation_handshake/source_inverse.json').read_text())
        runtime=fixture['replacements'][-1][1]
        call="wait_graph_recovery_enabled(recover,lambda timeout:expect(recover).to_be_enabled(timeout=timeout),PlaywrightTimeoutError,wait=wait)"
        for changed in [runtime.replace(call,'expect(recover).to_be_enabled()'),runtime.replace('exact=True','exact=False'),runtime.replace('            wait_graph_recovery_enabled', '            recover.click()\n            wait_graph_recovery_enabled')]:
            with self.assertRaises(AssertionError):check(text.replace(runtime,changed,1))


if __name__=='__main__':unittest.main()
