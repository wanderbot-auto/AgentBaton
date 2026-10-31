#!/usr/bin/env python3
"""Publish finite child-Goal CI results; never copy private browser evidence."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import threading

CASES = ('normal', 'lost-start', 'cancel')
EXPECTED = {'normal': {'CG01','CG02','CG05','CG06'},
            'lost-start': {'CG01','CG02','CG03','CG05','CG06'},
            'cancel': {'CG01','CG02','CG04','CG06'}}

class Refused(ValueError):
    pass

def require(value):
    if not value:
        raise Refused('Private evidence does not satisfy the finite CI projection')

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def regular_bytes(path, maximum, private=False):
    with os.fdopen(os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK),'rb') as stream:
        info=os.fstat(stream.fileno())
        require(stat.S_ISREG(info.st_mode) and info.st_nlink==1 and 0<info.st_size<=maximum)
        if private:require(stat.S_IMODE(info.st_mode)==0o600)
        raw=stream.read(maximum+1);require(len(raw)<=maximum)
        return raw

def unique_object(pairs):
    value={}
    for key,item in pairs:
        require(key not in value);value[key]=item
    return value

def decode(raw):
    def invalid_constant(_):raise Refused('Non-JSON numeric constant')
    return json.loads(raw,object_pairs_hook=unique_object,parse_constant=invalid_constant)

def strict_json_equal(left,right):
    if type(left)is not type(right):return False
    if type(left)is dict:
        return all(type(k)is str for k in left)and all(type(k)is str for k in right)and left.keys()==right.keys()and all(strict_json_equal(left[k],right[k])for k in left)
    if type(left)is list:
        return len(left)==len(right)and all(strict_json_equal(a,b)for a,b in zip(left,right))
    if type(left)is float:return math.isfinite(left)and math.isfinite(right)and left.hex()==right.hex()
    if type(left)in (str,int,bool,type(None)):return left==right
    return False

def candidate_summary(raw):
    m=decode(raw);require(type(m)is dict and type(m.get('schema'))is int and m['schema']==17 and m.get('dirty')is False)
    for key in ('git_head','source_head','compiled_core_source_head'):
        require(type(m.get(key))is str and re.fullmatch('[0-9a-f]{40}',m[key])is not None)
    require(m['git_head']==m['source_head']==m['compiled_core_source_head'])
    tools=m.get('binaries');require(type(tools)is dict and set(tools)=={'batond','baton'})
    for value in tools.values():require(type(value)is str and re.fullmatch('[0-9a-f]{64}',value)is not None)
    assets=m.get('frontend_assets');require(type(assets)is dict and 0<len(assets)<=4096)
    for key,value in assets.items():require(type(key)is str and type(value)is str and re.fullmatch('[0-9a-f]{64}',value)is not None)
    summary={'schema':17,'source_head':m['source_head'],'compiled_core_source_head':m['compiled_core_source_head'],
        'manifest_sha256':sha(raw),'tools_sha256':{'batond':tools['batond'],'baton':tools['baton']},
        'frontend_asset_count':len(assets),'frontend_assets_sha256':sha(json.dumps(assets,sort_keys=True,separators=(',',':')).encode())}
    return m,summary

# Fixed Core15 method/code vocabulary, not user-supplied display strings.
DIAGNOSTIC_METHODS = frozenset(['approvals.decline', 'approvals.finalize', 'command_validation_policies.list', 'command_validation_policies.propose', 'command_validation_policies.revoke', 'command_validations.cancel', 'command_validations.get', 'command_validations.list', 'command_validations.run', 'connections.list', 'connections.propose', 'connections.register', 'data_validation_policies.list', 'data_validation_policies.propose', 'data_validation_policies.revoke', 'data_validations.list', 'data_validations.run', 'data_workspace_previews.cancel', 'data_workspace_previews.get', 'data_workspace_previews.list', 'data_workspace_previews.start', 'data_workspaces.approve', 'data_workspaces.cancel', 'data_workspaces.create', 'data_workspaces.get', 'data_workspaces.list', 'data_workspaces.propose', 'events.subscribe', 'factory_child_goals.advance_plan', 'factory_child_goals.cancel', 'factory_child_goals.get', 'factory_child_goals.list', 'factory_child_goals.start', 'factory_child_plans.context', 'factory_child_plans.get', 'factory_child_plans.list', 'factory_child_plans.submit', 'factory_runs.cancel', 'factory_runs.get', 'factory_runs.start', 'factory_scopes.get', 'factory_scopes.list', 'factory_scopes.propose', 'factory_scopes.revoke', 'goal_graph_runs.advance_plan', 'goal_graph_runs.cancel', 'goal_graph_runs.dispatch', 'goal_graph_runs.get', 'goal_graph_runs.list', 'goal_graph_runs.propose', 'goal_node_approvals.propose', 'goal_plans.accept', 'goal_plans.dispatch', 'goal_plans.get', 'goal_plans.list', 'goal_plans.propose', 'goal_plans.ready_preview', 'goal_plans.reserve', 'goal_plans.revise', 'goal_runs.advance_plan', 'goal_runs.cancel', 'goal_runs.dispatch', 'goal_runs.get', 'goal_runs.list', 'goal_runs.propose', 'goal_scopes.approve', 'goal_scopes.get', 'goal_scopes.list', 'goal_scopes.propose', 'goal_scopes.revoke', 'grants.create', 'grants.list', 'grants.propose', 'grants.revoke', 'projects.archive', 'projects.import', 'projects.list', 'projects.reassociate', 'sessions.cancel', 'sessions.create', 'sessions.get', 'sessions.list', 'sessions.send', 'system.snapshot', 'tasks.cancel', 'tasks.create', 'tasks.get', 'tasks.list', 'validation_policies.list', 'validation_policies.propose', 'validation_policies.revoke', 'validations.list', 'validations.run', 'workspaces.create', 'workspaces.list'])
DIAGNOSTIC_CODES = frozenset(['AUTH_REQUIRED', 'BUDGET_UNKNOWN', 'CANCEL_REQUESTED', 'CAPABILITY_UNSUPPORTED', 'CHILD_CANCELLED', 'CONFIG_CONFLICT', 'DEADLINE_EXCEEDED', 'ENFORCEMENT_UNAVAILABLE', 'INPUT_CHANGED', 'INTERNAL_ERROR', 'INVALID_REQUEST', 'NOT_FOUND', 'OUTCOME_UNKNOWN', 'PERMISSION_DENIED', 'PROVIDER_DISCONNECTED', 'RESOURCE_LIMIT', 'STALE_REVISION', 'TIMEOUT', 'VERSION_UNSUPPORTED', 'WORKSPACE_BUSY'])
DIAGNOSTIC_EFFECTS = frozenset({'not_started','confirmed','unknown'})
ERROR_CLASSES = frozenset({'assertion','refusal','timeout','os_error','type_error','value_error','runtime_error','other'})
MAX_DIAGNOSTIC_REPLIES=2048
MAX_DIAGNOSTIC_BUCKETS=64
GRAPH_ARGV_REASONS=frozenset({'empty','oversized','missing_nul','argv_mismatch'})

def graph_argv_diagnostic(value):
    require(type(value)is dict and set(value)=={'reason','previously_bound'})
    require(type(value['reason'])is str and value['reason']in GRAPH_ARGV_REASONS)
    require(type(value['previously_bound'])is bool)
    return value

class GraphProviderArgvFailure(AssertionError):
    """Fixed scalars only; never retain raw argv or an expected identity."""
    def __init__(self,reason,previously_bound):
        self.diagnostic=graph_argv_diagnostic({'reason':reason,'previously_bound':previously_bound})
        super().__init__('Graph Provider argv guard failed')

def check_graph_provider_argv(raw,expected_argv,previously_bound):
    """Same nonempty/size/NUL/decoded-argv refusal, with a finite first reason."""
    require(type(raw)is bytes and type(expected_argv)is list and all(type(v)is str for v in expected_argv))
    require(len(expected_argv)==3 and type(previously_bound)is bool)
    if len(raw)==0:raise GraphProviderArgvFailure('empty',previously_bound)
    if len(raw)>8192:raise GraphProviderArgvFailure('oversized',previously_bound)
    if not raw.endswith(b'\0'):raise GraphProviderArgvFailure('missing_nul',previously_bound)
    # Preserve the original strict UTF8 decode and its ValueError classification.
    if [v.decode()for v in raw[:-1].split(b'\0')][-3:]!=expected_argv:
        raise GraphProviderArgvFailure('argv_mismatch',previously_bound)

def unobserved_owner_refresh():
    return {'observation':'unobserved','source_epoch':None,'started_ns':None,
        'click_finished_ns':None,'finished_ns':None,'refresh_completed':None,
        'full_fresh':None,'loading':None,'pending_kind':'unobserved',
        'correlation':'temporal_only','click_causality':'not_established',
        'reads':[],'scope_task_only_read_count':0,'truncated':False}

def nullable_bool(value):
    return value is None or type(value)is bool

def diagnostic_error(value):
    require(type(value)is dict and set(value)=={'method','code','retryable','side_effect_state'})
    require(type(value['method'])is str and value['method']in DIAGNOSTIC_METHODS)
    require(type(value['code'])is str and value['code']in DIAGNOSTIC_CODES)
    require(type(value['retryable'])is bool and type(value['side_effect_state'])is str and value['side_effect_state']in DIAGNOSTIC_EFFECTS)
    return value

def owner_refresh_diagnostic(value):
    require(type(value)is dict and set(value)==set(unobserved_owner_refresh()))
    require(type(value['observation'])is str and value['observation']in {'unobserved','observed'})
    # App epoch/freshness/loading/pending have no direct observable DOM source.
    # Harness sequence numbers and a returned click are never substitutes.
    require(all(value[k]is None for k in ('source_epoch','refresh_completed','full_fresh','loading')))
    require(value['pending_kind']=='unobserved' and value['correlation']=='temporal_only' and value['click_causality']=='not_established')
    require(type(value['reads'])is list and len(value['reads'])<=64 and type(value['truncated'])is bool)
    require(type(value['scope_task_only_read_count'])is int and 0<=value['scope_task_only_read_count']<=2048)
    for key in ('started_ns','click_finished_ns','finished_ns'):
        require(value[key]is None or type(value[key])is int and 0<value[key]<2**63)
    start=value['started_ns']
    if value['observation']=='unobserved':
        require(value==unobserved_owner_refresh());return value
    require(start is not None)
    require(all(value[k]is None or value[k]>=start for k in ('click_finished_ns','finished_ns')))
    if value['click_finished_ns']is not None and value['finished_ns']is not None:
        require(value['click_finished_ns']<=value['finished_ns'])
    for row in value['reads']:
        require(type(row)is dict and set(row)=={'method','binding','started_ns','finished_ns','reply_class','error'})
        require(row['method']=='factory_child_goals.get' and row['binding']=='scope_task_run')
        require(type(row['started_ns'])is int and type(row['finished_ns'])is int and start<=row['started_ns']<=row['finished_ns']<2**63)
        require(value['finished_ns']is None or row['finished_ns']<=value['finished_ns'])
        require(type(row['reply_class'])is str and row['reply_class']in {'result','error','unmapped_error','malformed'})
        if row['reply_class']=='error':
            require(diagnostic_error(row['error'])['method']==row['method'])
        else:require(row['error']is None)
    return value

def graph_identity_diagnostic(value):
    fields={'initial_binding','empty_read_observed','known_same_identity','uid_matches',
        'pgid_matches','ppid_matches','start_ticks_match','after_read','exact_cancel_durable'}
    require(type(value)is dict and set(value)==fields)
    require(type(value['initial_binding'])is str and value['initial_binding']in {'unobserved','verified'})
    require(value['empty_read_observed']is True)
    require(all(nullable_bool(value[k])for k in fields-{'initial_binding','empty_read_observed','after_read'}))
    require(type(value['after_read'])is str and value['after_read']in {'unobserved','same','absent','changed'})
    if value['initial_binding']=='unobserved':
        require(all(value[k]is None for k in ('known_same_identity','uid_matches','pgid_matches','ppid_matches','start_ticks_match')))
        require(value['after_read']in {'unobserved','absent'})
    if value['after_read']in {'unobserved','absent'}:
        require(all(value[k]is None for k in ('known_same_identity','uid_matches','pgid_matches','ppid_matches','start_ticks_match')))
    if value['after_read']=='changed':require(value['known_same_identity']is False)
    if value['after_read']=='same' or value['known_same_identity']is True:
        require(value['known_same_identity']is True)
        require(value['initial_binding']=='verified' and value['after_read']=='same')
        require(all(value[k]is True for k in ('uid_matches','pgid_matches','ppid_matches','start_ticks_match')))
    return value

def unobserved_diagnostics():
    return {'version':3,'failure_class':None,'driver_lines':None,'lines_truncated':None,
        'phase':None,'last_completed_check':None,'core_replies':{'observation':'unobserved',
        'captured_count':None,'truncated':None,'unmapped_error_count':None,'errors':None},
        'graph_provider_argv':None,'owner_refresh':unobserved_owner_refresh(),
        'graph_provider_identity':None}

def finite_diagnostics(value):
    if value is None:return unobserved_diagnostics()
    require(type(value)is dict and type(value.get('version'))is int and value['version']in (1,2,3))
    expected=set(unobserved_diagnostics())
    if value['version']<3:expected-= {'owner_refresh','graph_provider_identity'}
    if value['version']==1:expected.remove('graph_provider_argv')
    require(set(value)==expected)
    if value['version']>=2 and value['graph_provider_argv']is not None:
        graph_argv_diagnostic(value['graph_provider_argv'])
        require(value['failure_class']in ({'assertion','refusal'}if value['version']==3 else {'assertion'}))
    if value['version']==3:
        owner_refresh_diagnostic(value['owner_refresh'])
        if value['graph_provider_identity']is not None:graph_identity_diagnostic(value['graph_provider_identity'])
    require(value['failure_class']is None or type(value['failure_class'])is str and value['failure_class']in ERROR_CLASSES)
    require(value['phase']is None or type(value['phase'])is str and value['phase']in {'runtime','cleanup'})
    require(value['last_completed_check']is None or type(value['last_completed_check'])is str and value['last_completed_check']in {'CG01','CG02','CG03','CG04','CG05','CG06'})
    lines=value['driver_lines'];require(lines is None or type(lines)is list and 0<len(lines)<=64 and all(type(n)is int and 1<=n<=1000000 for n in lines))
    require(type(value['lines_truncated'])is bool if value['failure_class']is not None else value['lines_truncated']is None)
    require(value['failure_class']is not None or lines is None and value['phase']is None)
    rows=value['core_replies'];require(type(rows)is dict and set(rows)=={'observation','captured_count','truncated','unmapped_error_count','errors'})
    require(type(rows['observation'])is str and rows['observation']in {'unobserved','observed'})
    if rows['observation']=='unobserved':require(all(rows[k]is None for k in ('captured_count','truncated','unmapped_error_count','errors')))
    else:
        require(type(rows['captured_count'])is int and 1<=rows['captured_count']<=MAX_DIAGNOSTIC_REPLIES and type(rows['truncated'])is bool)
        require(type(rows['unmapped_error_count'])is int and 0<=rows['unmapped_error_count']<=rows['captured_count'])
        require(type(rows['errors'])is list and len(rows['errors'])<=MAX_DIAGNOSTIC_BUCKETS)
        seen=set();total=rows['unmapped_error_count']
        for row in rows['errors']:
            require(type(row)is dict and set(row)=={'method','code','retryable','side_effect_state','count'})
            require(type(row['method'])is str and row['method']in DIAGNOSTIC_METHODS and type(row['code'])is str and row['code']in DIAGNOSTIC_CODES)
            require(type(row['retryable'])is bool and type(row['side_effect_state'])is str and row['side_effect_state']in DIAGNOSTIC_EFFECTS)
            require(type(row['count'])is int and 1<=row['count']<=MAX_DIAGNOSTIC_REPLIES)
            identity=(row['method'],row['code'],row['retryable'],row['side_effect_state']);require(identity not in seen);seen.add(identity);total+=row['count']
        require(total<=rows['captured_count'])
    return value

class FiniteRuntimeDiagnostics:
    """Observe returned Core dictionaries only; no error-text parsing or IO."""
    def __init__(self):
        self._lock=threading.Lock();self._value=unobserved_diagnostics();self._buckets={};self._invalid=False;self._owner_query=None
    def owner_refresh_begin(self,query,started_ns):
        with self._lock:
            require(type(query)is dict and set(query)=={'scope_id','task_id','goal_run_id'} and all(type(v)is str and v for v in query.values()))
            self._owner_query=dict(query);self._value['owner_refresh']=unobserved_owner_refresh()
            self._value['owner_refresh'].update(observation='observed',started_ns=started_ns)
    def owner_refresh_click_finished(self,finished_ns):
        with self._lock:self._value['owner_refresh']['click_finished_ns']=finished_ns
    def owner_refresh_end(self,finished_ns):
        with self._lock:self._value['owner_refresh']['finished_ns']=finished_ns
    def owner_refresh_reply(self,method,params,reply,started_ns,finished_ns):
        with self._lock:
            window=self._value['owner_refresh'];query=self._owner_query
            if query is None or window['finished_ns']is not None or started_ns<window['started_ns']:return
            # list lacks a run argument: count its scope/task match but never
            # associate its error with this exact Goal or infer click causality.
            scope_task={k:query[k]for k in ('scope_id','task_id')}
            if method=='factory_child_goals.list' and (strict_json_equal(params,scope_task)or strict_json_equal(params,{**scope_task,'goal_run_id':None})):
                if window['scope_task_only_read_count']>=2048:window['truncated']=True
                else:window['scope_task_only_read_count']+=1
                return
            if method!='factory_child_goals.get' or not strict_json_equal(params,query):return
            if len(window['reads'])>=64:window['truncated']=True;return
            kind='malformed';error=None
            if type(reply)is dict:
                if 'error'in reply:
                    raw=reply['error']
                    if type(raw)is dict and type(raw.get('code'))is str and raw['code']in DIAGNOSTIC_CODES and type(raw.get('retryable'))is bool and type(raw.get('side_effect_state'))is str and raw['side_effect_state']in DIAGNOSTIC_EFFECTS:
                        error={'method':method,**{k:raw[k]for k in ('code','retryable','side_effect_state')}};kind='error'
                    else:kind='unmapped_error'
                elif 'result'in reply:kind='result'
            window['reads'].append({'method':method,'binding':'scope_task_run','started_ns':started_ns,'finished_ns':finished_ns,'reply_class':kind,'error':error})
    def graph_argv(self,value):
        with self._lock:
            if self._value['graph_provider_argv']is None:
                self._value['graph_provider_argv']=dict(graph_argv_diagnostic(value))
    def graph_identity(self,value):
        with self._lock:
            if self._value['graph_provider_identity']is None:
                self._value['graph_provider_identity']=dict(graph_identity_diagnostic(value))
    def completed(self,check):
        with self._lock:
            if type(check)is not str or check not in {'CG01','CG02','CG03','CG04','CG05','CG06'}:self._invalid=True
            else:self._value['last_completed_check']=check
    def reply(self,method,reply):
        # This short independent metadata lock covers no IPC, file or callback.
        with self._lock:
            rows=self._value['core_replies']
            if rows['observation']=='unobserved':rows.update(observation='observed',captured_count=0,truncated=False,unmapped_error_count=0,errors=[])
            if rows['captured_count']>=MAX_DIAGNOSTIC_REPLIES:rows['truncated']=True;return
            rows['captured_count']+=1
            if type(reply)is not dict:self._invalid=True;return
            if 'error'not in reply:return
            error=reply['error']
            if type(error)is not dict or set(error)!={'code','message','retryable','side_effect_state','details_ref'} or type(error['code'])is not str or type(error['message'])is not str or type(error['retryable'])is not bool or type(error['side_effect_state'])is not str or not(error['details_ref']is None or type(error['details_ref'])is str):
                self._invalid=True;return
            if type(method)is not str or method not in DIAGNOSTIC_METHODS or error['code']not in DIAGNOSTIC_CODES or error['side_effect_state']not in DIAGNOSTIC_EFFECTS:
                rows['unmapped_error_count']+=1;return
            identity=(method,error['code'],error['retryable'],error['side_effect_state'])
            if identity not in self._buckets:
                if len(self._buckets)>=MAX_DIAGNOSTIC_BUCKETS:rows['unmapped_error_count']+=1;rows['truncated']=True;return
                row=dict(zip(('method','code','retryable','side_effect_state'),identity));row['count']=0
                self._buckets[identity]=row;rows['errors'].append(row)
            self._buckets[identity]['count']+=1
    def failure(self,error,phase,filename,refusal_type,timeout_types=()):
        with self._lock:
            if self._value['failure_class']is not None:return
            if isinstance(error,refusal_type):kind='refusal'
            elif isinstance(error,AssertionError):kind='assertion'
            elif isinstance(error,(TimeoutError,)+timeout_types):kind='timeout'
            elif isinstance(error,OSError):kind='os_error'
            elif isinstance(error,TypeError):kind='type_error'
            elif isinstance(error,ValueError):kind='value_error'
            elif isinstance(error,RuntimeError):kind='runtime_error'
            else:kind='other'
            lines=[];truncated=False;tb=error.__traceback__
            for _ in range(4096):
                if tb is None:break
                if tb.tb_frame.f_code.co_filename==filename:
                    if len(lines)<64:lines.append(tb.tb_lineno)
                    else:truncated=True
                tb=tb.tb_next
            if tb is not None:truncated=True
            self._value.update(failure_class=kind,driver_lines=lines or None,lines_truncated=truncated,phase=phase)
            if type(error)is GraphProviderArgvFailure:
                self._value['graph_provider_argv']=dict(graph_argv_diagnostic(error.diagnostic))
    def snapshot(self):
        with self._lock:
            if self._invalid:return {'version':1,'invalid':True}
            # All retained fields have already been reduced to finite scalars.
            return json.loads(json.dumps(self._value,allow_nan=False))

def project(report, candidate, summary, case, exit_code, harness_sha):
    require(case in CASES and (exit_code is None or type(exit_code)is int and -128<=exit_code<=255))
    require(type(report)is dict and report.get('case')==case and report.get('status')in ('passed','failed_or_interrupted'))
    require(strict_json_equal(report.get('candidate'),candidate) and report.get('manifest_sha256')==summary['manifest_sha256'])
    require(report.get('harness_sha256')==harness_sha)
    checks=report.get('checks');require(type(checks)is list and len(checks)<=6)
    safe=[];seen=set()
    for check in checks:
        require(type(check)is dict and type(check.get('id'))is str and check['id']in EXPECTED[case] and check['id']not in seen and check.get('status')=='passed')
        seen.add(check['id']);safe.append({'id':check['id'],'status':'passed'})
    cleanup=report.get('cleanup');require(type(cleanup)is list and len(cleanup)<=256 and type(report.get('cleanup_complete'))is bool)
    diagnostics=finite_diagnostics(report.get('finite_diagnostics'))
    passed=(exit_code==0 and report['status']=='passed' and report['cleanup_complete']is True
        and not report.get('failure') and report.get('http_unexpected_errors')==[] and seen==EXPECTED[case]
        and diagnostics.get('graph_provider_argv')is None)
    return {'contract':'agentbaton.child-goal-ci.v1','case':case,'status':'passed'if passed else 'failed',
        **summary,'harness_sha256':harness_sha,'exit_code':exit_code,'command_state':'unobserved'if exit_code is None else 'returned','checks':safe,
        'cleanup_complete':report['cleanup_complete'],'cleanup_record_count':len(cleanup),
        'failure_kind':'none'if passed else 'runtime_failed','screenshots_published':False,
        'private_history_published':False,'finite_diagnostics':diagnostics}

def publish(path,value):
    pending=path.with_name(path.name+'.pending')
    with os.fdopen(os.open(pending,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600),'wb')as stream:
        os.fchmod(stream.fileno(),0o600);stream.write((json.dumps(value,indent=2)+'\n').encode());stream.flush();os.fsync(stream.fileno())
    os.replace(pending,path)

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--private-dir',type=Path,required=True);p.add_argument('--manifest',type=Path,required=True)
    p.add_argument('--case',choices=CASES,required=True);p.add_argument('--exit-code',required=True)
    p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    exit_code=None if args.exit_code=='unobserved'else int(args.exit_code)
    require(exit_code is None or -128<=exit_code<=255)
    output=args.output.absolute();output.mkdir(mode=0o700,parents=True,exist_ok=False)
    summary={'contract':'agentbaton.child-goal-ci.v1','case':args.case,'status':'failed',
        'exit_code':exit_code,'command_state':'unobserved'if exit_code is None else 'returned','failure_kind':'manifest_invalid','screenshots_published':False,'private_history_published':False,'finite_diagnostics':unobserved_diagnostics()}
    stage='manifest_invalid'
    try:
        candidate,fields=candidate_summary(regular_bytes(args.manifest,4_194_304));summary.update(fields)
        harness=Path(__file__).resolve().parents[1]/'apps/desktop/tests/factory-child-goal-real-ipc.py'
        harness_sha=sha(regular_bytes(harness,2_097_152));summary['harness_sha256']=harness_sha
        stage='private_directory_invalid'
        info=args.private_dir.lstat();require(stat.S_ISDIR(info.st_mode)and not stat.S_ISLNK(info.st_mode)and stat.S_IMODE(info.st_mode)==0o700)
        stage='report_missing'
        report_path=args.private_dir/'report.json'
        if not report_path.exists():
            summary['failure_kind']='preflight_refused'if(args.private_dir/'preflight-refusal.json').is_file()else 'report_missing'
        else:
            stage='report_invalid'
            summary=project(decode(regular_bytes(report_path,33_554_432,private=True)),candidate,fields,args.case,exit_code,harness_sha)
    except (OSError,ValueError,TypeError,KeyError,OverflowError,RecursionError):
        summary['status']='failed';summary['failure_kind']=stage
    publish(output/'summary.json',summary)
    return 0 if summary['status']=='passed'else 1

if __name__=='__main__':raise SystemExit(main())
