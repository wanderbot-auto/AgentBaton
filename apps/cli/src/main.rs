use agentbaton_core::{
    ipc::{call, default_runtime_dir},
    CoreError, Result,
};
use serde_json::json;
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command=match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["snapshot"]=>json!({"method":"system.snapshot","params":{}}),
        ["projects","list"]=>json!({"method":"projects.list","params":{}}),
        ["projects","import",path,name,key]=>json!({"method":"projects.import","params":{"path":path,"name":name},"idempotency_key":key}),
        ["projects","archive",id,revision,key]=>json!({"method":"projects.archive","params":{"project_id":id},"expected_revision":parse_revision(revision)?,"idempotency_key":key}),
        ["projects","reassociate",id,path,revision,key]=>json!({"method":"projects.reassociate","params":{"project_id":id,"path":path},"expected_revision":parse_revision(revision)?,"idempotency_key":key}),
        ["workspaces","list",id]=>json!({"method":"workspaces.list","params":{"project_id":id}}),
        ["workspaces","create",id,head,revision,key]=>json!({"method":"workspaces.create","params":{"project_id":id,"expected_head":head},"expected_revision":parse_revision(revision)?,"idempotency_key":key}),
        ["tasks","create",project,goal,key]=>json!({"method":"tasks.create","params":{"project_id":project,"goal":goal},"idempotency_key":key}),
        ["tasks","list"]=>json!({"method":"tasks.list","params":{}}),
        ["tasks","get",id]=>json!({"method":"tasks.get","params":{"task_id":id}}),
        ["tasks","create",goal,key]=>json!({"method":"tasks.create","params":{"goal":goal},"idempotency_key":key}),
        ["tasks","cancel",id,revision]=>json!({"method":"tasks.cancel","params":{"task_id":id},"expected_revision":revision.parse::<u64>().map_err(|_|CoreError::new("INVALID_REQUEST","Revision must be an integer"))?}),
        ["events",cursor]=>json!({"method":"events.subscribe","params":{"cursor":cursor.parse::<u64>().map_err(|_|CoreError::new("INVALID_REQUEST","Cursor must be an integer"))?,"limit":100}}),
        _=>return Err(CoreError::new("INVALID_REQUEST","Usage: baton snapshot | tasks list | tasks get ID | tasks create GOAL IDEMPOTENCY_KEY | tasks cancel ID REVISION | events CURSOR | projects list/import PATH NAME KEY/archive ID REV KEY/reassociate ID PATH REV KEY | workspaces list ID/create ID HEAD PROJECT_REV KEY")),
    };
    let response = call(&default_runtime_dir()?, command)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&response)
            .map_err(|_| CoreError::new("INTERNAL_ERROR", "Cannot print response"))?
    );
    if response.get("error").is_some() {
        return Err(
            serde_json::from_value::<CoreError>(response["error"].clone()).unwrap_or_else(|_| {
                CoreError::new("INTERNAL_ERROR", "Daemon returned an invalid error")
            }),
        );
    }
    Ok(())
}

fn parse_revision(value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| CoreError::new("INVALID_REQUEST", "Revision must be an integer"))
}
