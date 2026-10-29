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
    m=decode(raw);require(type(m)is dict and type(m.get('schema'))is int and m['schema']==15 and m.get('dirty')is False)
    for key in ('git_head','source_head','compiled_core_source_head'):
        require(type(m.get(key))is str and re.fullmatch('[0-9a-f]{40}',m[key])is not None)
    require(m['git_head']==m['source_head']==m['compiled_core_source_head'])
    tools=m.get('binaries');require(type(tools)is dict and set(tools)=={'batond','baton'})
    for value in tools.values():require(type(value)is str and re.fullmatch('[0-9a-f]{64}',value)is not None)
    assets=m.get('frontend_assets');require(type(assets)is dict and 0<len(assets)<=4096)
    for key,value in assets.items():require(type(key)is str and type(value)is str and re.fullmatch('[0-9a-f]{64}',value)is not None)
    summary={'schema':15,'source_head':m['source_head'],'compiled_core_source_head':m['compiled_core_source_head'],
        'manifest_sha256':sha(raw),'tools_sha256':{'batond':tools['batond'],'baton':tools['baton']},
        'frontend_asset_count':len(assets),'frontend_assets_sha256':sha(json.dumps(assets,sort_keys=True,separators=(',',':')).encode())}
    return m,summary

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
    passed=(exit_code==0 and report['status']=='passed' and report['cleanup_complete']is True
        and not report.get('failure') and report.get('http_unexpected_errors')==[] and seen==EXPECTED[case])
    return {'contract':'agentbaton.child-goal-ci.v1','case':case,'status':'passed'if passed else 'failed',
        **summary,'harness_sha256':harness_sha,'exit_code':exit_code,'command_state':'unobserved'if exit_code is None else 'returned','checks':safe,
        'cleanup_complete':report['cleanup_complete'],'cleanup_record_count':len(cleanup),
        'failure_kind':'none'if passed else 'runtime_failed','screenshots_published':False,
        'private_history_published':False}

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
        'exit_code':exit_code,'command_state':'unobserved'if exit_code is None else 'returned','failure_kind':'manifest_invalid','screenshots_published':False,'private_history_published':False}
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
