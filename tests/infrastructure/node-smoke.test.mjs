import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';

test('Node is new enough for the intended toolchain family', () => {
  const [major,minor] = process.versions.node.split('.').map(Number);
  assert.ok(major > 22 || (major === 22 && minor >= 12));
});
test('JSON and UTF-8 round-trip through child process pipes', () => {
  const payload = {scope:'environment-smoke',text:'测试 中文与空格',productImplemented:false};
  const script = 'let x="";process.stdin.setEncoding("utf8");process.stdin.on("data",d=>x+=d);process.stdin.on("end",()=>process.stdout.write(JSON.stringify(JSON.parse(x))));';
  const p = spawnSync(process.execPath,['-e',script],{input:JSON.stringify(payload),encoding:'utf8',timeout:5000});
  assert.equal(p.status,0);assert.deepEqual(JSON.parse(p.stdout),payload);
});
test('Nonzero child exit is observed rather than treated as success', () => {
  const p = spawnSync(process.execPath,['-e','process.exit(7)'],{timeout:5000});
  assert.equal(p.status,7);
});
test('Temporary UTF-8 workspace IO works without touching a project', async () => {
  const dir = await mkdtemp(join(tmpdir(),'agentbaton-infra-'));
  try {const p=join(dir,'验证 文件.json');await writeFile(p,JSON.stringify({ok:true}));assert.deepEqual(JSON.parse(await readFile(p,'utf8')),{ok:true});}
  finally {await rm(dir,{recursive:true,force:true});}
});
