//! Dependency-free infrastructure fixture. Not an Agent provider or daemon.
fn main() {
    println!(
        "AgentBaton infrastructure smoke: {} / {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn executable_is_available() {
        let executable = std::env::current_exe().expect("test executable path");
        assert!(executable.is_file());
    }

    #[test]
    fn utf8_workspace_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "agentbaton-infra-rust-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root).expect("unique temporary workspace");
        let path = root.join("测试 文件.txt");
        std::fs::write(&path, "environment-only").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "environment-only");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
