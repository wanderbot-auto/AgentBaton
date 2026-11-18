use rusqlite::{Connection, Error};

fn main() {
    let storage = std::fs::read_to_string("crates/core/src/storage.rs").unwrap();
    for (name, table) in [
        ("migrate_runtime_snapshots", "session_runtime_snapshots"),
        ("migrate_validations", "validation_policies"),
    ] {
        let body = storage.split_once(&format!("fn {name}(")).unwrap().1;
        let batch = body
            .split_once("tx.execute_batch(\"")
            .unwrap()
            .1
            .split_once("\").map_err")
            .unwrap()
            .0;
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(&format!("CREATE TABLE {table}(sentinel TEXT)"))
            .unwrap();
        let error = db.execute_batch(batch).unwrap_err();
        let Error::SqlInputError { error, msg, offset, .. } = error else {
            panic!("{name} did not produce a preparation error");
        };
        assert_eq!(error.extended_code, 1);
        assert_eq!(msg, format!("table {table} already exists"));
        println!("{name}: SqlInputError extended_code=1, offset={offset}, {msg}");
    }
}
