//! Feature-only OS fixture witness. Opens existing SQLite strictly read-only;
//! never migrates, creates authority or returns raw proposals/paths/documents.
use super::{digest, integration_consumed_proposal, is_integration};
use serde_json::Value;
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const SQL: &str = r#"
import hashlib,json,sqlite3,sys
from pathlib import Path
def pairs(items):
    out={}
    for k,v in items:
        if k in out: raise ValueError('duplicate')
        out[k]=v
    return out
def bad_float(s): raise ValueError('noninteger')
def decode(raw): return json.loads(raw,object_pairs_hook=pairs,parse_float=bad_float,parse_constant=bad_float)
def sha(v): return hashlib.sha256(json.dumps(v,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode('utf-8')).hexdigest()
path,proposal_id,phase,proposal_sha,snapshot_sha,policy_sha,task_id,spec_revision,scope_id,baseline,original_revision=sys.argv[1:]
baseline=int(baseline); original_revision=int(original_revision)
db=sqlite3.connect(Path(path).resolve().as_uri()+'?mode=ro',uri=True,timeout=1,isolation_level=None)
db.execute('PRAGMA query_only=ON');db.execute('PRAGMA trusted_schema=OFF');db.execute('BEGIN')
assert db.execute('PRAGMA query_only').fetchone()[0]==1
schema=db.execute('PRAGMA user_version').fetchone()[0];assert schema==17
tables=('integration_policies','integrations','integration_attempts','integration_ref_fences')
for table in tables:
    row=db.execute('SELECT type,sql FROM sqlite_master WHERE name=?',(table,)).fetchone()
    assert row is not None and row[0]=='table' and row[1].upper().startswith('CREATE TABLE ')
counts=[db.execute('SELECT count(*) FROM '+t).fetchone()[0] for t in tables]
sequence=db.execute('SELECT COALESCE(max(sequence),0) FROM events').fetchone()[0]
row=db.execute("SELECT document,result FROM approval_proposals WHERE id=? AND typeof(document)='text' AND length(CAST(document AS BLOB))<=1048576 AND (result IS NULL OR (typeof(result)='text' AND length(CAST(result AS BLOB))<=1048576))",(proposal_id,)).fetchone();assert row is not None
proposal=decode(row[0]);assert proposal['kind']=='integration' and proposal['id']==proposal_id
assert sha(proposal)==proposal_sha and sha(proposal['snapshot'])==snapshot_sha
assert type(proposal['revision']) is int and type(proposal['consumed']) is bool
approved=phase=='approve';assert phase in ('before','approve','cancel')
assert proposal['consumed'] is approved and proposal['revision']==original_revision
assert counts==([1,0,0,0] if approved else [0,0,0,0])
policy=db.execute('SELECT task_id,spec_revision,goal_scope_id,approval_id,revision,document FROM integration_policies WHERE id=? AND typeof(document)="text" AND length(CAST(document AS BLOB))<=1048576',(proposal_id,)).fetchone()
owner_sequence=None;policy_sequence=None
if approved:
    assert row[1] is not None and policy is not None
    result=decode(row[1]);document=decode(policy[5]);assert sha(result)==policy_sha and sha(document)==policy_sha
    assert policy[:5]==(task_id,int(spec_revision),scope_id,proposal_id,1)
    assert result['id']==proposal_id and result['approval_id']==proposal_id and result['revision']==1 and result['revoked'] is False
    assert result['selection']==proposal['snapshot']['intent'] and result['bindings']==proposal['snapshot']['bindings'] and result['snapshot_digest']==snapshot_sha
else:
    assert row[1] is None and policy is None and sequence==baseline
# Event predicates below are pinned to the Core integration producer contract.
# Both the consumed Owner proposal and policy publication must be in this view.
owner=db.execute("SELECT sequence,document FROM events WHERE sequence>? AND json_extract(document,'$.aggregate_id')=? AND json_extract(document,'$.kind')='approval.confirmed' AND length(CAST(document AS BLOB))<=1048576 ORDER BY sequence LIMIT 2",(baseline,proposal_id)).fetchall()
publication=db.execute("SELECT sequence,document FROM events WHERE sequence>? AND json_extract(document,'$.aggregate_id')=? AND json_extract(document,'$.kind')='integration.policy_approved' AND length(CAST(document AS BLOB))<=1048576 ORDER BY sequence LIMIT 2",(baseline,proposal_id)).fetchall()
if approved:
    assert len(owner)==1 and len(publication)==1
    owner_sequence,owner_raw=owner[0];policy_sequence,policy_raw=publication[0]
    owner_event=decode(owner_raw);policy_event=decode(policy_raw)
    assert type(owner_event['sequence']) is int and owner_event['sequence']==0 and type(policy_event['sequence']) is int and policy_event['sequence']==0
    assert owner_event['source']=='local_owner_confirmation' and type(owner_event['revision']) is int and owner_event['revision']==original_revision
    assert sha(owner_event['payload'])==sha(dict(kind='integration',proposal_id=proposal_id,consumed=True))
    assert type(policy_event['revision']) is int and policy_event['revision']==1 and policy_event['payload']['snapshot_digest']==snapshot_sha and policy_event['source']=='integration_core'
    assert sha(policy_event['payload'])==sha(dict(snapshot_digest=snapshot_sha,owner_sequence=owner_sequence,scope_digest=proposal['snapshot']['bindings']['goal_scope_digest'],binding_digest=proposal['snapshot']['bindings']['binding_digest'],automatic_watch_armed=True))
    assert baseline<owner_sequence<policy_sequence<=sequence
else: assert not owner and not publication
out=dict(schema_version=schema,sequence=sequence,**dict(zip(('policies','runs','attempts','ref_fences'),counts)))
print(json.dumps(dict(counts=out,proposal_matches=True,proposal_result_matches=True,policy_matches=True,owner_event_matches=True,policy_event_matches=True,owner_event_sequence=owner_sequence,policy_event_sequence=policy_sequence),separators=(',',':')))
db.close()
"#;

struct OwnedQuery(Child);
impl Drop for OwnedQuery {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn bounded_query(mut command: Command) -> Result<Value, String> {
    let mut child = OwnedQuery(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Cannot start independent readonly integration query")?,
    );
    let stdout = child
        .0
        .stdout
        .take()
        .ok_or("Missing readonly integration output")?;
    let reader = std::thread::Builder::new()
        .name("native-integration-readonly".into())
        .spawn(move || {
            let mut bytes = Vec::new();
            stdout.take(4097).read_to_end(&mut bytes).map(|_| bytes)
        })
        .map_err(|_| "Cannot observe readonly integration query")?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let status: Result<std::process::ExitStatus, String> = (|| loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|_| "Cannot join readonly integration query")?
        {
            break Ok(status);
        }
        if Instant::now() >= deadline {
            break Err("Readonly integration query exceeded original bound".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    })();
    if status.is_err() {
        let _ = child.0.kill();
        let _ = child.0.wait();
    }
    let bytes = reader
        .join()
        .map_err(|_| "Readonly integration reader join failed")?
        .map_err(|_| "Readonly integration output read failed")?;
    if !status?.success() || bytes.len() > 4096 {
        return Err("Independent readonly integration witness failed".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "Invalid bounded integration witness".into())
}
pub(crate) fn checkpoint(
    runtime: &Path,
    expected: &Value,
    response: Option<&Value>,
) -> Result<Value, String> {
    if !is_integration(expected) {
        return Err("Wrong independent integration witness kind".into());
    }
    let phase = if response.is_none() {
        "before"
    } else {
        expected["phase"].as_str().ok_or("No integration phase")?
    };
    let proposal = integration_consumed_proposal(&expected["proposal"], phase)?;
    let revision = proposal["revision"]
        .as_u64()
        .ok_or("No exact proposal revision")?;
    let mut command = Command::new("python3");
    command
        .args(["-I", "-S", "-c", SQL])
        .arg(runtime.join("baton.sqlite3"))
        .arg(
            expected["proposal"]["id"]
                .as_str()
                .ok_or("No proposal ID")?,
        )
        .arg(phase)
        .arg(digest(&proposal)?)
        .arg(digest(&expected["proposal"]["snapshot"])?)
        .arg(if phase == "approve" {
            digest(&response.ok_or("No actual integration reply")?["result"])?
        } else {
            "-".into()
        })
        .arg(
            expected["invoke"]["selection"]["task_id"]
                .as_str()
                .ok_or("No integration Task")?,
        )
        .arg(
            expected["invoke"]["selection"]["expected_spec_revision"]
                .as_u64()
                .ok_or("No integration spec")?
                .to_string(),
        )
        .arg(
            expected["invoke"]["selection"]["goal_scope_id"]
                .as_str()
                .ok_or("No integration Scope")?,
        )
        .arg(
            expected["baseline_sequence"]
                .as_u64()
                .ok_or("No baseline sequence")?
                .to_string(),
        )
        .arg(revision.to_string());
    bounded_query(command)
}
