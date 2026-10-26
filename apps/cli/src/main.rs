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
        ["tasks","list"]=>json!({"method":"tasks.list","params":{}}),
        ["tasks","get",id]=>json!({"method":"tasks.get","params":{"task_id":id}}),
        ["tasks","create",goal,key]=>json!({"method":"tasks.create","params":{"goal":goal},"idempotency_key":key}),
        ["tasks","cancel",id,revision]=>json!({"method":"tasks.cancel","params":{"task_id":id},"expected_revision":revision.parse::<u64>().map_err(|_|CoreError::new("INVALID_REQUEST","Revision must be an integer"))?}),
        ["events",cursor]=>json!({"method":"events.subscribe","params":{"cursor":cursor.parse::<u64>().map_err(|_|CoreError::new("INVALID_REQUEST","Cursor must be an integer"))?,"limit":100}}),
        _=>return Err(CoreError::new("INVALID_REQUEST","Usage: baton snapshot | tasks list | tasks get ID | tasks create GOAL IDEMPOTENCY_KEY | tasks cancel ID REVISION | events CURSOR")),
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
