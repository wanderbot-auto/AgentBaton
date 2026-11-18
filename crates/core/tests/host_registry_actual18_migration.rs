//! Required-feature genuine18 -> normal Store19 and actual frozen18 backup recovery.
//! No missing-input skip; synthetic/inverse inputs cannot satisfy the fixed seal.
use agentbaton_core::{host_registry_migration, storage::Store};
use rusqlite::{types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
const FIXTURE_SHA: &str = "613e624582d6437576bbe53fd773be5c880c5d7e2aa424c4fd6931817e77c1f1";
const SOURCE: &str = "a4e03303b54bd5a5544131e95ad39662f8310e29";
const TREE: &str = "ddd5bddbeeb8011fa070d53e67f7978deae87214";
fn sha(raw: &[u8]) -> String {
    agentbaton_workspaces::validation::sha256(raw)
}
fn required(name: &str) -> PathBuf {
    let p = PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("required genuine18 input missing: {name}")),
    );
    assert!(p.is_absolute(), "{name} must be absolute");
    assert_eq!(fs::canonicalize(&p).unwrap(), p);
    p
}
fn private_read(path: &Path, max: u64) -> Vec<u8> {
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .unwrap();
    let a = f.metadata().unwrap();
    assert!(
        a.is_file()
            && a.nlink() == 1
            && a.uid() == rustix::process::geteuid().as_raw()
            && a.mode() & 0o7777 == 0o600
            && a.len() > 0
            && a.len() <= max
    );
    let mut raw = vec![];
    Read::by_ref(&mut f)
        .take(max + 1)
        .read_to_end(&mut raw)
        .unwrap();
    let b = f.metadata().unwrap();
    let c = fs::symlink_metadata(path).unwrap();
    let pin = |x: &fs::Metadata| {
        (
            x.dev(),
            x.ino(),
            x.len(),
            x.mtime(),
            x.mtime_nsec(),
            x.ctime(),
            x.ctime_nsec(),
            x.mode(),
            x.uid(),
            x.nlink(),
        )
    };
    assert_eq!(pin(&a), pin(&b));
    assert_eq!(pin(&b), pin(&c));
    assert_eq!(raw.len() as u64, a.len());
    raw
}
fn write_new(path: &Path, raw: &[u8]) {
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    f.write_all(raw).unwrap();
    f.sync_all().unwrap();
}
type Schema = Vec<(String, String, String, Option<String>)>;
#[derive(Debug, PartialEq)]
struct Image {
    schema: Schema,
    rows: BTreeMap<String, Vec<Vec<SqlValue>>>,
    version: i64,
    cookie: i64,
    application: i64,
}
fn image(path: &Path) -> Image {
    let db = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
    )
    .unwrap();
    db.pragma_update(None, "query_only", true).unwrap();
    let schema: Schema = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let rows = schema
        .iter()
        .filter(|x| x.0 == "table")
        .map(|x| {
            let mut s = db
                .prepare(&format!(
                    "SELECT rowid,* FROM \"{}\" ORDER BY rowid",
                    x.1.replace('"', "\"\"")
                ))
                .unwrap();
            let width = s.column_count();
            let data = s
                .query_map([], |r| {
                    (0..width)
                        .map(|i| r.get(i))
                        .collect::<rusqlite::Result<Vec<SqlValue>>>()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (x.1.clone(), data)
        })
        .collect();
    assert!(!db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    Image {
        schema,
        rows,
        version: db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        cookie: db
            .pragma_query_value(None, "schema_version", |r| r.get(0))
            .unwrap(),
        application: db
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap(),
    }
}
const PROBE: &str = r#"
import importlib.util,json,sys
from pathlib import Path
repo,fixture,frozen,manifest=map(Path,sys.argv[1:])
spec=importlib.util.spec_from_file_location('bound18',repo/'scripts/generate-schema18-migration-fixture.py');G=importlib.util.module_from_spec(spec);spec.loader.exec_module(G)
m=G.verify_fixture(fixture,frozen,manifest)
print(json.dumps({'status':m['status'],'schema':m['schema'],'source':m['source_commit'],'tree':m['source_tree'],'sha256':m['database_sha256'],'bytes':m['database_bytes'],'build_sha256':m['tool_provenance_sha256'],'generator_sha256':m['generator_sha256'],'helper_sha256':m['helper_sha256'],'binaries':m['binaries']}))
"#;
const RESTORE: &str = r#"
import importlib.util,json,os,socket,subprocess,sys,time,uuid
from pathlib import Path
repo,fixture,frozen,manifest,runtime=map(Path,sys.argv[1:])
spec=importlib.util.spec_from_file_location('bound18restore',repo/'scripts/generate-schema18-migration-fixture.py');G=importlib.util.module_from_spec(spec);spec.loader.exec_module(G);H=G.H
G.verify_fixture(fixture,frozen,manifest);before=G.probe(runtime/'baton.sqlite3');tools=H.tool_records(frozen)
child=None;state={'owned_identity':None,'sigterm_sent':False,'forced_kill':False};error=None;result={'status':'failed'}
try:
 env={k:v for k,v in os.environ.items() if k in ('PATH','LANG','LC_ALL','TMPDIR')};env['BATON_RUNTIME_DIR']=str(runtime)
 with (runtime/'old18-daemon.log').open('xb') as log:
  os.fchmod(log.fileno(),0o600)
  child=subprocess.Popen([str(frozen/'batond')],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  state['owned_identity']=H.COMMON.darwin_owned_identity(child);G.wait_ready(runtime,child)
  token=H.read_regular(runtime/'session.token',1024).decode();request={'api_version':1,'request_id':str(uuid.uuid4()),'token':token,'method':'system.snapshot','params':{}}
  with socket.socket(socket.AF_UNIX) as client:
   client.settimeout(10);client.connect(str(runtime/'batond.sock'));client.sendall(json.dumps(request).encode()+b'\n')
   with client.makefile('rb') as reader:raw=reader.readline(1048577)
  H.require(0<len(raw)<=1048576 and raw.endswith(b'\n'),'restore RPC bound');reply=H.parse_json(raw);H.require(reply.get('request_id')==request['request_id'] and 'result'in reply and not reply.get('error'),'restore RPC failure')
  snapshot=reply['result'];observed=G.snapshot_observation(snapshot,5,3)
  cli=subprocess.run([str(frozen/'baton'),'snapshot'],env=env,capture_output=True,timeout=10);H.require(cli.returncode==0 and len(cli.stdout)<=1048576,'restore CLI failed');cli_snapshot=H.parse_json(cli.stdout)['result'];G.snapshot_observation(cli_snapshot,5,3);H.require(H.strict_equal(cli_snapshot,snapshot),'restore CLI/Core mismatch')
  H.require(sorted(x['state'] for x in snapshot['tasks'])==['cancelled','draft','draft'],'original historical states not restored')
  result.update(observed=observed,cli_exit=cli.returncode,task_states=sorted(x['state'] for x in snapshot['tasks']))
except BaseException as caught:error=caught
finally:
 if child is not None:H.COMMON.stop_darwin_owned(child,H.NATIVE,state)
 result['daemon']=state
 if error is None:
  H.require(state.get('identity_gone_after_join') is True,'restore stop unknown')
  after=G.probe(runtime/'baton.sqlite3');H.require(before['logical_sha256']==after['logical_sha256'],'restore changed old history');H.require(H.strict_equal(tools,H.tool_records(frozen)),'old tool changed');result.update(status='passed',before=before,after=after)
 H.COMMON.publish_private_json(runtime/'old18-restore.json',result)
if error is not None:raise error
print(json.dumps(result))
"#;
fn python(python: &Path, program: &str, args: &[&Path], out: &Path) -> Value {
    let mut command = Command::new(python);
    command
        .arg("-W")
        .arg("error::ResourceWarning")
        .arg("-c")
        .arg(program)
        .env_clear();
    for key in ["PATH", "LANG", "LC_ALL", "TMPDIR"] {
        if let Some(v) = std::env::var_os(key) {
            command.env(key, v);
        }
    }
    for p in args {
        command.arg(p);
    }
    let output = command.output().unwrap();
    let mut raw = output.stdout.clone();
    raw.extend_from_slice(&output.stderr);
    write_new(out, &raw);
    assert!(
        output.status.success(),
        "private genuine18 controller failed; retained {}",
        out.display()
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
#[test]
fn actual18_normal_store19_backup_is_recoverable_by_exact_frozen18_programs() {
    let fixture = required("AGENTBATON_ACTUAL18_FIXTURE_DIR");
    let frozen = required("AGENTBATON_ACTUAL18_FROZEN_DIR");
    let manifest = required("AGENTBATON_ACTUAL18_BUILD_MANIFEST");
    let interpreter = required("AGENTBATON_ACTUAL18_PYTHON");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let root = tempfile::Builder::new()
        .prefix("ab19-a18-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in("/tmp")
        .unwrap()
        .keep();
    let root = fs::canonicalize(root).unwrap();
    let proof = python(
        &interpreter,
        PROBE,
        &[repo, &fixture, &frozen, &manifest],
        &root.join("input-proof.log"),
    );
    assert_eq!(proof["status"], "passed");
    assert_eq!(proof["source"], SOURCE);
    assert_eq!(proof["tree"], TREE);
    assert_eq!(proof["sha256"], FIXTURE_SHA);
    let original = private_read(&fixture.join("actual-schema18.sqlite3"), 16_777_216);
    assert_eq!(sha(&original), FIXTURE_SHA);
    let original_image = image(&fixture.join("actual-schema18.sqlite3"));
    assert_eq!(original_image.version, 18);
    assert_eq!(original_image.cookie, 1);
    let path = root.join("input.sqlite3");
    write_new(&path, &original);
    let before = image(&path);
    assert_eq!(before, original_image);
    assert_eq!(before.rows["tasks"].len(), 3);
    assert_eq!(before.rows["events"].len(), 5);
    assert_eq!(before.rows["idempotency"].len(), 4);
    let store = Store::open(&path).unwrap();
    drop(store);
    let migrated = image(&path);
    assert_eq!(migrated.version, 19);
    assert_eq!(migrated.cookie, before.cookie + 2);
    assert_eq!(migrated.application, before.application);
    for (name, values) in &before.rows {
        assert_eq!(
            &migrated.rows[name],
            values,
            "raw preexisting rows changed {name};{}",
            root.display()
        );
    }
    assert!(migrated.rows["host_registrations"].is_empty());
    let retained: Vec<_> = migrated
        .schema
        .iter()
        .filter(|r| r.1 != "host_registrations" && r.2 != "host_registrations")
        .cloned()
        .collect();
    assert_eq!(retained, before.schema);
    let check = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    host_registry_migration::validate_current19(&check).unwrap();
    drop(check);
    drop(Store::open(&path).unwrap());
    assert_eq!(image(&path), migrated);
    let backups: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("input.pre-v19-")
                && p.extension().is_some_and(|e| e == "sqlite3")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    let backup = &backups[0];
    let sealed = private_read(backup, 16_777_216);
    let backed = image(backup);
    assert_eq!(backed.schema, before.schema);
    assert_eq!(backed.rows, before.rows);
    assert_eq!(backed.version, 18);
    assert_eq!(backed.application, before.application);
    let runtime = root.join("r");
    fs::DirBuilder::new().mode(0o700).create(&runtime).unwrap();
    write_new(&runtime.join("baton.sqlite3"), &sealed);
    let restored = python(
        &interpreter,
        RESTORE,
        &[repo, &fixture, &frozen, &manifest, &runtime],
        &root.join("old18-controller.log"),
    );
    assert_eq!(restored["status"], "passed");
    assert_eq!(restored["observed"]["database_schema_version"], 18);
    assert_eq!(restored["daemon"]["identity_gone_after_join"], true);
    assert_eq!(image(&runtime.join("baton.sqlite3")).rows, before.rows);
    assert_eq!(sha(&private_read(backup, 16_777_216)), sha(&sealed));
    assert_eq!(
        sha(&private_read(
            &fixture.join("actual-schema18.sqlite3"),
            16_777_216
        )),
        FIXTURE_SHA
    );
    let after_restore_proof = python(
        &interpreter,
        PROBE,
        &[repo, &fixture, &frozen, &manifest],
        &root.join("input-proof-after.log"),
    );
    assert_eq!(after_restore_proof, proof);
    let report = json!({"status":"passed","dimension":"real_genuine18_normal_Core19_migration_and_frozen18_backup_recovery","input_sha256":FIXTURE_SHA,"source":SOURCE,"tree":TREE,"input_cookie":before.cookie,"migrated_cookie":migrated.cookie,"backup_cookie":backed.cookie,"original_rows_preserved":true,"backup_rows_preserved":true,"reopen19_exact":true,"old18_program_restore":restored,"input_tools_unchanged":true,"backup_sha256":sha(&sealed),"private_root":root,"not_claimed":["nonempty Conversation/Factory/integration history","P2 readers/native/UI/CI","power-failure durability"]});
    write_new(
        &root.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap().as_slice(),
    );
    println!("{report}");
}
