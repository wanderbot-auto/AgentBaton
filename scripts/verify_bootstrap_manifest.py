#!/usr/bin/env python3
"""Validate the delivered preparation tree; hashes provide integrity, not identity."""
from pathlib import Path
import argparse, hashlib, json, sys
ROOT=Path(__file__).resolve().parents[1]
def main():
    p=argparse.ArgumentParser();p.add_argument('--paths0',action='store_true');a=p.parse_args()
    items=json.loads((ROOT/'BOOTSTRAP_MANIFEST.json').read_text())['files']
    for name,expected in items.items():
        path=ROOT/name
        if path.is_symlink() or not path.resolve().is_relative_to(ROOT) or not path.is_file():
            raise SystemExit('Unsafe/missing bootstrap path: '+name)
        if hashlib.sha256(path.read_bytes()).hexdigest()!=expected:
            raise SystemExit('Bootstrap file changed; review before staging: '+name)
    if a.paths0:
        sys.stdout.buffer.write(b'\0'.join(x.encode() for x in [*sorted(items),'BOOTSTRAP_MANIFEST.json'])+b'\0')
    else: print(json.dumps({'status':'passed','files':len(items),'scope':'bootstrap_file_integrity'}))
if __name__=='__main__':main()
