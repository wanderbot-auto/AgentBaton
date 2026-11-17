#!/usr/bin/env python3
"""Run the accepted verifier with ONLY its evidence output prefix relocated.
No test, command, limit, failure behavior, input or original file is modified.
"""
from pathlib import Path
import difflib
import hashlib
import json
import os

root=Path(__file__).resolve().parents[4]
original=root/"scripts/verify-remote-host.py"
source=original.read_text()
old='Path("docs/execution/remote-host-foundation/runs")'
new='Path("docs/execution/remote-host-core-integration/registration/runs")'
assert source.count(old)==1
executed=source.replace(old,new)
record=root/"docs/execution/remote-host-core-integration/registration/VERIFIER_RELOCATION.json"
record.write_text(json.dumps(dict(original_path=str(original),original_sha256=hashlib.sha256(source.encode()).hexdigest(),executed_sha256=hashlib.sha256(executed.encode()).hexdigest(),only_change=list(difflib.unified_diff(source.splitlines(),executed.splitlines(),fromfile="original",tofile="evidence-prefix-only",n=1)),scope_reason="Legacy verifier writes prior-task docs outside current claim; current output must remain in the allocated RHOST002 directory."),indent=2)+"\n")
os.chdir(root)
exec(compile(executed,str(original),"exec"),{"__file__":str(original),"__name__":"__main__"})
