//! Fixed MCP stdio bridge. It never reads or forwards the daemon owner token.
use crate::{CoreError, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::MetadataExt, net::UnixStream},
    path::{Component, Path},
    time::Duration,
};
const MAX_REQUEST: usize = 65536;
const MAX_CHILD_PLAN_REQUEST: usize = 1024 * 1024;
const MAX_RESPONSE: u64 = 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopedCredential {
    schema_version: u32,
    handle_id: String,
    socket_path: std::path::PathBuf,
    scoped_token: String,
    /// Server-written private handle metadata, not a tools/call parameter.
    #[serde(default)]
    child_planning: bool,
}
fn denied() -> CoreError {
    CoreError::new(
        "AUTH_REQUIRED",
        "The private Factory scoped credential is unavailable or invalid",
    )
}
fn credential(path: &Path) -> Result<ScopedCredential> {
    use rustix::fs::{open, openat, Mode, OFlags};
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(denied());
    }
    let mut fd: File = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| denied())?
    .into();
    let parts: Vec<_> = path.components().collect();
    for (i, part) in parts.iter().enumerate() {
        if matches!(part, Component::RootDir) {
            continue;
        }
        let Component::Normal(name) = part else {
            return Err(denied());
        };
        let leaf = i + 1 == parts.len();
        if leaf {
            let parent = fd.metadata().map_err(|_| denied())?;
            if parent.uid() != rustix::process::geteuid().as_raw()
                || parent.mode() & 0o7777 != 0o700
            {
                return Err(denied());
            }
        }
        let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        if !leaf {
            flags |= OFlags::DIRECTORY;
        }
        fd = openat(&fd, *name, flags, Mode::empty())
            .map_err(|_| denied())?
            .into();
    }
    let before = fd.metadata().map_err(|_| denied())?;
    if !before.is_file()
        || before.nlink() != 1
        || before.uid() != rustix::process::geteuid().as_raw()
        || before.mode() & 0o7777 != 0o600
        || before.len() > 8192
    {
        return Err(denied());
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut fd)
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| denied())?;
    let after = fd.metadata().map_err(|_| denied())?;
    if bytes.len() as u64 != before.len()
        || (
            after.dev(),
            after.ino(),
            after.len(),
            after.mode(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        ) != (
            before.dev(),
            before.ino(),
            before.len(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        )
    {
        return Err(denied());
    }
    let value: ScopedCredential = serde_json::from_slice(&bytes).map_err(|_| denied())?;
    if value.schema_version != 1
        || value.handle_id.is_empty()
        || value.handle_id.len() > 128
        || value.scoped_token.len() != 64
        || !value.scoped_token.bytes().all(|b| b.is_ascii_hexdigit())
        || !value.socket_path.is_absolute()
        || value
            .socket_path
            .file_name()
            .is_none_or(|p| p != "batond.sock")
    {
        return Err(denied());
    }
    Ok(value)
}
fn call(
    credential: &ScopedCredential,
    name: &str,
    args: Value,
    key: Option<String>,
    revision: Option<u64>,
) -> Result<Value> {
    if !matches!(
        name,
        "delegate_task"
            | "get_task_status"
            | "get_task_result"
            | "cancel_task"
            | "submit_child_plan"
    ) {
        return Err(denied());
    }
    let request = json!({"api_version":1,"request_id":uuid::Uuid::new_v4().to_string(),"handle_id":credential.handle_id,"scoped_token":credential.scoped_token,"method":name,"params":args,"idempotency_key":key,"expected_revision":revision});
    let bytes = serde_json::to_vec(&request).map_err(|_| denied())?;
    if bytes.len()
        > if name == "submit_child_plan" {
            MAX_CHILD_PLAN_REQUEST
        } else {
            MAX_REQUEST
        }
    {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Factory tool request exceeds its complete frame bound",
        ));
    }
    let mut socket =
        UnixStream::connect(&credential.socket_path).map_err(|_| CoreError::transport())?;
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|_| CoreError::transport())?;
    socket
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| CoreError::transport())?;
    socket
        .write_all(&bytes)
        .and_then(|()| socket.write_all(b"\n"))
        .map_err(|_| CoreError::transport())?;
    let mut response = Vec::new();
    BufReader::new(socket)
        .take(MAX_RESPONSE + 1)
        .read_until(b'\n', &mut response)
        .map_err(|_| CoreError::transport())?;
    if response.len() as u64 > MAX_RESPONSE || response.last() != Some(&b'\n') {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Factory tool response was incomplete or over its bound",
        ));
    }
    serde_json::from_slice(&response).map_err(|_| CoreError::transport())
}
fn tools(child_planning: bool) -> Value {
    let target = json!({"type":"object","properties":{"task_id":{"type":"string","minLength":1,"maxLength":128}},"required":["task_id"],"additionalProperties":false});
    let mut tools = json!([
    {"name":"delegate_task","description":"Request a bounded child under this exact session's independent Factory approval. Returns a handle immediately; it does not wait for or accept a result.","inputSchema":{"type":"object","properties":{"goal":{"type":"string","minLength":1,"maxLength":4096},"connection_id":{"type":"string"},"workspace_id":{"type":"string"},"input_artifact_ids":{"type":"array","items":{"type":"string"},"maxItems":16},"output_artifact_ids":{"type":"array","items":{"type":"string"},"maxItems":16},"budget":{"type":"object"},"wait_for_child":{"type":"boolean"},"idempotency_key":{"type":"string","minLength":1,"maxLength":128}},"required":["goal","connection_id","workspace_id","input_artifact_ids","output_artifact_ids","budget","wait_for_child","idempotency_key"],"additionalProperties":false}},
    {"name":"get_task_status","description":"Read this caller's own task or a same-Factory descendant; never expands caller scope.","inputSchema":target},
    {"name":"get_task_result","description":"Read bounded actual artifact and validation references. Candidate results are not Task acceptance.","inputSchema":target},
    {"name":"cancel_task","description":"Request cancellation of this caller's own task or descendant. A request does not prove process stop.","inputSchema":{"type":"object","properties":{"task_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"idempotency_key":{"type":"string","minLength":1,"maxLength":128}},"required":["task_id","expected_revision","idempotency_key"],"additionalProperties":false}}
     ]);
    if child_planning {
        tools.as_array_mut().expect("fixed tools array").push(json!({"name":"submit_child_plan","description":"Store a complete bounded versioned GoalPlan for this caller or its same-Factory descendant under an explicitly approved child planning policy. Returns immutable plan metadata, never execution or acceptance.","inputSchema":{"type":"object","properties":{"task_id":{"type":"string","minLength":1,"maxLength":128},"plan_json":{"type":"string","minLength":1,"maxLength":262144},"expected_plan_revision":{"type":"integer","minimum":0,"maximum":255},"idempotency_key":{"type":"string","minLength":1,"maxLength":128}},"required":["task_id","plan_json","expected_plan_revision","idempotency_key"],"additionalProperties":false}}));
    }
    tools
}
/// The trusted host passes only a server-created0600 handle file. No owner IPC
/// command, generic shell, arbitrary MCP server or global credential is exposed.
pub fn run(path: &Path) -> Result<()> {
    let credential = credential(path)?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut initialized = false;
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take((MAX_CHILD_PLAN_REQUEST + 1) as u64)
            .read_until(b'\n', &mut line)
            .map_err(|_| CoreError::transport())?;
        if n == 0 {
            return Ok(());
        }
        if line.len() > MAX_CHILD_PLAN_REQUEST || line.last() != Some(&b'\n') {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "MCP request exceeds its bounded frame",
            ));
        }
        let request: Value = serde_json::from_slice(&line)
            .map_err(|_| CoreError::new("INVALID_REQUEST", "Invalid bounded MCP envelope"))?;
        let child_plan_call =
            request["method"] == "tools/call" && request["params"]["name"] == "submit_child_plan";
        if line.len() > MAX_REQUEST && !child_plan_call {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Original Factory MCP methods keep their65536-byte frame bound",
            ));
        }
        let id = request.get("id").cloned();
        let method = request["method"].as_str().unwrap_or("");
        if id.is_none() {
            if method == "notifications/initialized" {
                continue;
            }
            continue;
        }
        if child_plan_call
            && !id.as_ref().is_some_and(|v| {
                v.as_str().is_some_and(|s| s.len() <= 128)
                    || v.as_u64().is_some_and(|n| n <= 9_007_199_254_740_991)
            })
        {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Child-plan MCP id must be a bounded string or safe unsigned integer",
            ));
        }
        let id = id.unwrap();
        let result = if request["jsonrpc"] != "2.0" {
            Err(CoreError::new(
                "INVALID_REQUEST",
                "Expected MCP JSON-RPC2.0",
            ))
        } else {
            match method {
                "initialize" if !initialized => {
                    initialized = true;
                    Ok(
                        json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"agentbaton-factory","version":"0.1.0"}}),
                    )
                }
                "ping" if initialized => Ok(json!({})),
                "tools/list" if initialized => {
                    Ok(json!({"tools":tools(credential.child_planning)}))
                }
                "tools/call" if initialized => {
                    let params = &request["params"];
                    let name = params["name"].as_str().unwrap_or("");
                    let mut args = params["arguments"].clone();
                    let key = args
                        .as_object_mut()
                        .and_then(|o| o.remove("idempotency_key"))
                        .and_then(|v| v.as_str().map(str::to_owned));
                    let revision = args
                        .as_object_mut()
                        .and_then(|o| o.remove("expected_revision"))
                        .and_then(|v| v.as_u64());
                    call(&credential,name,args,key,revision).map(|response|json!({"content":[{"type":"text","text":serde_json::to_string(&response).unwrap_or_else(|_|"{\"error\":\"serialization\"}".into())}],"isError":response.get("error").is_some()}))
                }
                _ => Err(CoreError::new(
                    "CAPABILITY_UNSUPPORTED",
                    "Only the fixed initialized Factory MCP tools are available",
                )),
            }
        };
        let response = match result {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err(error) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":error.message}})
            }
        };
        let bytes = serde_json::to_vec(&response).map_err(|_| CoreError::transport())?;
        if bytes.len() as u64 > MAX_RESPONSE {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Complete MCP response exceeds its bound",
            ));
        }
        writer
            .write_all(&bytes)
            .and_then(|()| writer.write_all(b"\n"))
            .and_then(|()| writer.flush())
            .map_err(|_| CoreError::transport())?;
    }
}

#[cfg(test)]
mod child_planning_contract_tests {
    use super::*;
    #[test]
    fn old_disabled_tools_are_identical_and_new_policy_adds_only_fifth_tool() {
        let old = tools(false);
        let new = tools(true);
        let old = old.as_array().unwrap();
        let new = new.as_array().unwrap();
        let historical: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/factory_original_four_tools.json"
        ))
        .unwrap();
        assert_eq!(
            old,
            historical.as_array().unwrap(),
            "Preserve the exact original four schema bodies, not only their names"
        );
        assert_eq!(old.len(), 4);
        assert_eq!(&new[..4], old);
        assert_eq!(new.len(), 5);
        assert_eq!(new[4]["name"], "submit_child_plan");
        assert_eq!(
            old.iter()
                .map(|v| v["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                "delegate_task",
                "get_task_status",
                "get_task_result",
                "cancel_task"
            ]
        );
    }
    #[test]
    fn original_private_credential_defaults_no_child_planning_and_never_accepts_renderer_capability_fields(
    ) {
        let raw = json!({"schema_version":1,"handle_id":"fixed","socket_path":"/tmp/owned/batond.sock","scoped_token":"a".repeat(64)});
        let old: ScopedCredential = serde_json::from_value(raw.clone()).unwrap();
        assert!(!old.child_planning);
        let mut new = raw.clone();
        new["child_planning"] = json!(true);
        assert!(
            serde_json::from_value::<ScopedCredential>(new)
                .unwrap()
                .child_planning
        );
        let mut bad = raw;
        bad["global_owner_token"] = json!("unavailable");
        assert!(serde_json::from_value::<ScopedCredential>(bad).is_err());
    }
}
