#!/usr/bin/env python3
"""Deny product execution until explicit approval is recorded. Not a sandbox."""
from pathlib import Path
import argparse, json

def assert_product_approval(gate):
    if (gate.get('product_development_authorized') is not True
        or not gate.get('approval_reference')
        or not gate.get('remote_repository')
        or gate.get('repository_owner_confirmed') is not True):
        raise PermissionError('Product development is on hold; explicit user approval and repository binding are missing.')

def main():
    p=argparse.ArgumentParser();p.add_argument('--scope',choices=['preparation','product'],default='product');args=p.parse_args()
    gate=json.loads((Path(__file__).resolve().parents[1]/'infra/execution-gate.json').read_text())
    if args.scope=='preparation':
        print('Preparation scope available. Product work remains separately gated.');return
    try: assert_product_approval(gate)
    except PermissionError as e: print('BLOCKED:',e);raise SystemExit(2)
    print('Product gate approved for',gate['remote_repository'])
if __name__=='__main__': main()
