#!/usr/bin/env python3
"""Separate UI readiness evidence; never invokes Cargo or a native app."""
from pathlib import Path
import hashlib
import json
import os
import platform
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[4]
UI=Path("docs/execution/remote-host-foundation/ui-readiness")
TEST=Path("apps/desktop/src/GoalPlanDraft.integration.test.tsx")
COMMANDS={
 "install":["npm","--prefix","apps/desktop","ci","--ignore-scripts","--no-audit","--no-fund"],
 "target":["npm","--prefix","apps/desktop","test","--","src/GoalPlanDraft.integration.test.tsx"],
 "typecheck":["npm","--prefix","apps/desktop","run","typecheck"],
 "full":["npm","--prefix","apps/desktop","test"],
}
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def main():
 if not os.environ.get("BATON_RUNTIME_DIR"):raise SystemExit("use scripts/collab.sh run")
 os.chdir(ROOT)
 assert sys.argv[1:] and all(n in COMMANDS for n in sys.argv[1:])
 plan=json.loads((UI/"PLAN.json").read_text())
 remote=json.loads(Path("docs/execution/remote-host-foundation/runs/20261117T153134Z-819857000/evidence.json").read_text())
 original={**plan["unchanged_production_fixture_config_sha256"],**remote["source_sha256"]}
 assert all(sha(Path(k))==v for k,v in original.items())
 run=UI/"runs"/(time.strftime("%Y%m%dT%H%M%SZ",time.gmtime())+"-"+str(time.time_ns()%1000000000));run.mkdir(parents=True)
 evidence=dict(head=subprocess.check_output(["git","rev-parse","HEAD"],text=True).strip(),dirty=subprocess.check_output(["git","status","--porcelain"],text=True).splitlines(),test_path=str(TEST),test_sha256=sha(TEST),harness_sha256=sha(Path(__file__)),platform=platform.platform(),machine=platform.machine(),node=subprocess.check_output(["node","--version"],text=True).strip(),npm=subprocess.check_output(["npm","--version"],text=True).strip(),runtime=os.environ["BATON_RUNTIME_DIR"],unchanged_source_sha256=original,remote_product_commit=remote["head"],remote_source_hash=remote["source_hash"],checks=[],dimension="React/Vitest jsdom with mocked native transport; not Core/native/remote acceptance")
 for name in sys.argv[1:]:
  log=run/(name+".log");start=time.time()
  with log.open("wb") as out:result=subprocess.run(COMMANDS[name],stdout=out,stderr=subprocess.STDOUT)
  evidence["checks"].append(dict(name=name,command=COMMANDS[name],exit_code=result.returncode,started_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ",time.gmtime(start)),elapsed_seconds=round(time.time()-start,3),log=str(log),log_sha256=sha(log)))
  evidence["sources_unchanged"]=sha(TEST)==evidence["test_sha256"] and all(sha(Path(k))==v for k,v in original.items())
  evidence["passed"]=all(c["exit_code"]==0 for c in evidence["checks"]) and evidence["sources_unchanged"]
  (run/"evidence.json").write_text(json.dumps(evidence,indent=2)+"\n")
  print(name,"exit=",result.returncode,"log=",log,flush=True)
  if result.returncode:print(log.read_text(errors="replace")[-6000:]);break
 print("evidence=",run/"evidence.json",flush=True)
 return 0 if evidence["passed"] else 1
if __name__=="__main__":sys.exit(main())
