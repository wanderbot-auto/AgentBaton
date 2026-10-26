#!/usr/bin/env python3
"""Read-only tool availability evidence. Never reads tokens or login files."""
from pathlib import Path
import argparse, datetime, json, platform, shutil, subprocess
TOOLS={'git':['--version'],'node':['--version'],'npm':['--version'],'python3':['--version'],'rustc':['--version'],'cargo':['--version'],'rustup':['--version'],'gh':['--version']}
def probe(name,args):
    exe=shutil.which(name)
    if not exe: return {'status':'missing','path':None}
    try:
        p=subprocess.run([exe,*args],text=True,capture_output=True,timeout=15)
        return {'status':'available' if p.returncode==0 else 'failed','path':exe,'version':(p.stdout or p.stderr).strip().splitlines()[:2],'exit_code':p.returncode}
    except (OSError,subprocess.TimeoutExpired) as e: return {'status':'failed','error':type(e).__name__}
def main():
    p=argparse.ArgumentParser();p.add_argument('--output',default='artifacts/doctor.json');p.add_argument('--strict',action='store_true');a=p.parse_args()
    d={'recorded_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'os':platform.system(),'arch':platform.machine(),'tools':{k:probe(k,v) for k,v in TOOLS.items()},'product_test_status':'not_run'}
    if platform.system()=='Darwin':
        d['xcode']=probe('xcodebuild',['-version']);d['codesign']=probe('codesign',['--version'])
    out=Path(a.output);out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(d,indent=2)+'\n');print(json.dumps(d,indent=2))
    required=['git','node','npm','python3','rustc','cargo','rustup']
    if a.strict and any(d['tools'][k]['status']!='available' for k in required): raise SystemExit(2)
if __name__=='__main__':main()
