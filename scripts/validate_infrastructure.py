#!/usr/bin/env python3
"""Static validation of preparation files, NOT proof of deployed infrastructure."""
from pathlib import Path
import json,re,yaml
ROOT=Path(__file__).resolve().parents[1]
def check(x,m):
    if not x:raise AssertionError(m)
def main():
    gate=json.loads((ROOT/'infra/execution-gate.json').read_text())
    check(gate['product_development_authorized'] is False,'Preparation gate must stay closed')
    check(gate['production_release_authorized'] is False,'No production approval')
    pins=json.loads((ROOT/'infra/action-pins.json').read_text())['actions']
    action_count=0
    files=list((ROOT/'.github/workflows').glob('*.yml'))
    check(len(files)==1,'Only the explicit environment preflight is enabled')
    for f in files:
        # BaseLoader avoids YAML 1.1 treating the Actions key "on" as a boolean.
        d=yaml.load(f.read_text(),Loader=yaml.BaseLoader)
        check(set(d['on'])=={'workflow_dispatch','push'},'Only explicit preparation triggers allowed')
        check(d['on']['push']=={'branches':['main'],'paths':['infra/ci-request.json']},'Automatic runs require the preparation request file on main')
        check(d['permissions']=={'contents':'read'},'Preflight permissions must be read only')
        check(d['jobs']['preflight']['strategy']['matrix']['os']==['ubuntu-24.04','macos-15','macos-15-intel'],'Dual-architecture hosts missing')
        for job in d['jobs'].values():
            check(int(job['timeout-minutes'])<=30,'Unbounded job')
            for step in job['steps']:
                if 'uses' in step:
                    action,sha=step['uses'].split('@')
                    check(re.fullmatch('[0-9a-f]{40}',sha) is not None,'Unpinned action')
                    check(pins[action]['commit']==sha,'Action pin mismatch')
                    action_count+=1
        check('secrets.' not in f.read_text(),'Preflight must not use secrets')
        check('pull_request_target' not in f.read_text(),'Privileged PR triggers forbidden')
    check((ROOT/'package-lock.json').is_file(),'Missing Node infrastructure lockfile')
    check((ROOT/'tools/environment-smoke/Cargo.lock').is_file(),'Missing Rust smoke lockfile')
    check(not (ROOT/'apps/desktop').exists(),'No product implementation is included in this preparation')
    print(json.dumps({'status':'passed','scope':'static_infrastructure_only','workflow_files':len(files),'pinned_action_references':action_count,'remote_ci_executed':False,'product_development_authorized':False},indent=2))
if __name__=='__main__':main()
