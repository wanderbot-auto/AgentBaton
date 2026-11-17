use agentbaton_remote_host::{
    protocol::{digest, identifier, Result},
    registration::*,
    transport::SshConfig,
};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};
use std::path::PathBuf;

fn fixture() -> HostRegistration {
    HostRegistration {
        schema_version: REGISTRATION_VERSION,
        name: "Synthetic remote Host".into(),
        declared_platform: DeclaredPlatform::Linux,
        endpoint: SshEndpointMetadata {
            expected_host_identity: "test_host".into(),
            address: "never-resolve.example.invalid".into(),
            user: "baton".into(),
            port: 22,
            host_key_alias: "approved_alias".into(),
        },
        known_hosts: KnownHostsReference {
            reference: "known-hosts:synthetic-v1".into(),
            file_sha256: "a".repeat(64),
        },
        transport_identity_ref: "unknown:synthetic-transport-identity".into(),
        configuration: ConfigurationReference {
            reference: "companion-config:synthetic-v1".into(),
            declared_ownership: DeclaredConfigurationOwnership::ExternalReadOnly,
        },
        directory_mappings: vec![DeclaredDirectoryMapping {
            id: "source".into(),
            remote_root: "/synthetic/remote/source".into(),
        }],
    }
}
fn code<T>(result: Result<T>, expected: &str) {
    match result {
        Err(e) => assert_eq!(e.code, expected),
        Ok(_) => panic!("expected {expected}"),
    }
}
fn legacy_endpoint(expected: &str, host: &str, user: &str, port: u16, alias: &str) -> Result<()> {
    use agentbaton_remote_host::protocol::require;
    require(identifier(expected), "ssh_host_identity")?;
    require(
        !host.is_empty()
            && host.len() <= 253
            && !host.starts_with('-')
            && host
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c)),
        "ssh_host",
    )?;
    require(
        identifier(user) && identifier(alias) && port > 0,
        "ssh_identity",
    )
}
fn legacy_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 512
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
        && !path.split('/').any(|p| p == ".." || p == ".")
}
#[test]
fn valid_metadata_roundtrip_has_a_fixed_synthetic_input_and_no_authority_projection() {
    let f = fixture();
    let bytes = serde_json::to_vec(&f).unwrap();
    println!(
        "registration_fixture_json={}",
        String::from_utf8(bytes.clone()).unwrap()
    );
    println!("registration_fixture_sha256={}", digest(&bytes));
    assert_eq!(decode_registration(&bytes).unwrap(), f);
    let value = serde_json::to_value(&f).unwrap();
    for key in [
        "connected",
        "verified",
        "execution_enabled",
        "grant",
        "permissions",
        "budget",
        "credential",
        "capabilities",
    ] {
        assert!(value.get(key).is_none());
    }
    for platform in [DeclaredPlatform::MacOs, DeclaredPlatform::Linux] {
        for ownership in [
            DeclaredConfigurationOwnership::BatonManaged,
            DeclaredConfigurationOwnership::ExternalReadOnly,
        ] {
            let mut other = f.clone();
            other.declared_platform = platform.clone();
            other.configuration.declared_ownership = ownership;
            other.directory_mappings.clear();
            other.validate().unwrap();
            assert_eq!(
                decode_registration(&serde_json::to_vec(&other).unwrap()).unwrap(),
                other
            );
        }
    }
}
#[test]
fn pure_endpoint_and_path_helpers_preserve_the_legacy_predicates_and_error_order() {
    let ids = ["good", "local", "LOCAL", "", "bad:id", "a-b_c"];
    let hosts = [
        "host.example",
        "127.0.0.1",
        "...",
        "h-",
        "-option",
        "h;cmd",
        "h@other",
        "h\ncmd",
        "",
    ];
    for expected in ids {
        for host in hosts {
            for user in ids {
                for alias in ids {
                    for port in [0, 1, 22, 65535] {
                        assert_eq!(
                            validate_ssh_endpoint_syntax(expected, host, user, port, alias),
                            legacy_endpoint(expected, host, user, port, alias)
                        );
                    }
                }
            }
        }
    }
    for path in [
        "",
        "/",
        "//",
        "/a/",
        "/a//b",
        "/a/./b",
        "/a/../b",
        "/a/.../b",
        "relative",
        "/a;cmd",
        "/space name",
        "/a\0b",
        "/中文",
    ] {
        assert_eq!(ssh_path_syntax(path), legacy_path(path));
    }
    for byte in 0u8..=127 {
        let path = format!("/prefix{}suffix", char::from(byte));
        assert_eq!(ssh_path_syntax(&path), legacy_path(&path));
    }
    for length in [1, 511, 512, 513] {
        let path = format!("/{}", "x".repeat(length - 1));
        assert_eq!(ssh_path_syntax(&path), legacy_path(&path));
    }
    for length in [253, 254] {
        let host = "x".repeat(length);
        assert_eq!(
            validate_ssh_endpoint_syntax("h", &host, "u", 22, "a"),
            legacy_endpoint("h", &host, "u", 22, "a")
        );
    }
}
#[test]
fn syntax_is_not_complete_registration_and_opaque_refs_are_not_attempt_ids() {
    for reserved in ["local", "LOCAL", "Local"] {
        let mut f = fixture();
        f.endpoint.expected_host_identity = reserved.into();
        f.endpoint.validate_syntax().unwrap();
        code(f.validate(), "registration_reserved_host");
    }
    let mut f = fixture();
    f.name.clear();
    f.endpoint.validate_syntax().unwrap();
    code(f.validate(), "registration_name");
    let f = fixture();
    assert!(!identifier(&f.transport_identity_ref));
    f.validate().unwrap();
    let result = decode_registration(&serde_json::to_vec(&f).unwrap()).unwrap();
    assert_eq!(
        result.transport_identity_ref,
        "unknown:synthetic-transport-identity"
    );
    assert_eq!(result.configuration, f.configuration);
}
#[test]
fn version_unknown_authority_fields_and_malformed_types_fail_closed() {
    for version in [0, 2, u32::MAX] {
        let mut f = fixture();
        f.schema_version = version;
        code(f.validate(), "registration_version");
    }
    for key in [
        "connected",
        "verified",
        "execution_enabled",
        "grant",
        "permission",
        "budget",
        "token",
        "private_key",
    ] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value[key] = json!(true);
        code(
            decode_registration(&serde_json::to_vec(&value).unwrap()),
            "protocol",
        );
    }
    for (object, key) in [
        ("endpoint", "command"),
        ("known_hosts", "fingerprint"),
        ("configuration", "synchronize"),
    ] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value[object][key] = json!("unapproved");
        code(
            decode_registration(&serde_json::to_vec(&value).unwrap()),
            "protocol",
        );
    }
    for patch in [
        json!({"declared_platform":"windows"}),
        json!({"declared_platform":"unknown"}),
        json!({"schema_version":-1}),
    ] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        for (key, v) in patch.as_object().unwrap() {
            value[key] = v.clone();
        }
        assert!(decode_registration(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for port in [-1, 65536] {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value["endpoint"]["port"] = json!(port);
        code(
            decode_registration(&serde_json::to_vec(&value).unwrap()),
            "protocol",
        );
    }
    let value = serde_json::to_string(&fixture()).unwrap();
    let duplicate = value.replacen(
        "\"schema_version\":1",
        "\"schema_version\":1,\"schema_version\":1",
        1,
    );
    code(decode_registration(duplicate.as_bytes()), "protocol");
}
#[test]
fn field_and_payload_byte_limits_are_checked_without_normalizing_input() {
    let mut f = fixture();
    f.name = "中".repeat(85) + "a";
    assert_eq!(f.name.len(), 256);
    f.validate().unwrap();
    f.name.push('b');
    code(f.validate(), "registration_name");
    for name in ["", " leading", "trailing ", "line\nbreak", "nul\0byte"] {
        let mut f = fixture();
        f.name = name.into();
        code(f.validate(), "registration_name");
    }
    let mut f = fixture();
    f.transport_identity_ref = "r".repeat(MAX_REFERENCE_BYTES);
    f.validate().unwrap();
    f.transport_identity_ref.push('r');
    code(f.validate(), "registration_reference");
    for reference in [
        "",
        ".",
        "..",
        "/private/path",
        "relative/path",
        "two words",
        "ref\nvalue",
        "ref\0value",
    ] {
        for field in 0..3 {
            let mut f = fixture();
            match field {
                0 => f.transport_identity_ref = reference.into(),
                1 => f.configuration.reference = reference.into(),
                _ => f.known_hosts.reference = reference.into(),
            };
            code(f.validate(), "registration_reference");
        }
    }
    for digest in [
        "A".repeat(64),
        "g".repeat(64),
        "a".repeat(63),
        format!("SHA256:{}", "a".repeat(64)),
    ] {
        let mut f = fixture();
        f.known_hosts.file_sha256 = digest;
        code(f.validate(), "registration_digest");
    }
    let mut bytes = serde_json::to_vec(&fixture()).unwrap();
    bytes.resize(MAX_REGISTRATION_BYTES, b' ');
    decode_registration(&bytes).unwrap();
    bytes.push(b' ');
    code(decode_registration(&bytes), "registration_size");
    let mut value: Value = serde_json::to_value(fixture()).unwrap();
    value["name"] = json!("x".repeat(MAX_REGISTRATION_BYTES));
    code(
        decode_registration(&serde_json::to_vec(&value).unwrap()),
        "registration_size",
    );
}
#[test]
fn declared_remote_mappings_are_bounded_unique_and_do_not_resolve_local_paths() {
    let mut f = fixture();
    f.directory_mappings = (0..MAX_DIRECTORY_MAPPINGS)
        .map(|i| DeclaredDirectoryMapping {
            id: format!("map{i}"),
            remote_root: format!("/nonexistent/remote/{i}"),
        })
        .collect();
    f.validate().unwrap();
    f.directory_mappings.push(DeclaredDirectoryMapping {
        id: "extra".into(),
        remote_root: "/remote/path".into(),
    });
    code(f.validate(), "registration_mappings");
    let mut f = fixture();
    f.directory_mappings.push(f.directory_mappings[0].clone());
    code(f.validate(), "registration_mapping_id");
    for id in ["", "bad:id", "a/b"] {
        let mut f = fixture();
        f.directory_mappings[0].id = id.into();
        code(f.validate(), "registration_mapping_id");
    }
    for root in [
        "relative",
        "/",
        "//remote/path",
        "/remote//path",
        "/remote/path/",
        "/remote/./path",
        "/remote/../path",
        "/remote/path;cmd",
        "/remote/path\0suffix",
    ] {
        let mut f = fixture();
        f.directory_mappings[0].remote_root = root.into();
        code(f.validate(), "registration_remote_root");
    }
    let mut f = fixture();
    f.directory_mappings[0].remote_root = format!("/{}", "r".repeat(511));
    f.validate().unwrap();
    f.directory_mappings[0].remote_root.push('r');
    code(f.validate(), "registration_remote_root");
    // Declared namespace stays byte-for-byte, regardless of claimed OS or controller layout.
    let mut f = fixture();
    f.directory_mappings[0].remote_root = "/Users/synthetic/remote-root".into();
    let before = f.clone();
    for _ in 0..10 {
        f.validate().unwrap();
    }
    assert_eq!(f, before);
}
#[test]
fn legacy_argv_keeps_exact_flags_filesystem_pin_checks_and_does_not_adopt_registration_policy() {
    let tmp = tempfile::tempdir().unwrap();
    let parent = fs::canonicalize(tmp.path()).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let known = parent.join("known_hosts");
    let key = parent.join("synthetic_identity");
    fs::write(&known, "synthetic known_hosts bytes").unwrap();
    fs::write(&key, "synthetic identity stat fixture").unwrap();
    for path in [&known, &key] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    // Syntax-only values intentionally rejected by full registration semantics.
    let mut ssh = SshConfig {
        expected_host_identity: "local".into(),
        host: "...".into(),
        user: "baton".into(),
        port: 22,
        host_key_alias: "alias".into(),
        known_hosts: known.clone(),
        known_hosts_digest: digest(&fs::read(&known).unwrap()),
        identity_file: key.clone(),
        remote_config: "/".into(),
    };
    let argv = ssh.argv().unwrap();
    let mut expected: Vec<String> = [
        "-F",
        "none",
        "-T",
        "-a",
        "-x",
        "-oBatchMode=yes",
        "-oStrictHostKeyChecking=yes",
        "-oUpdateHostKeys=no",
        "-oGlobalKnownHostsFile=/dev/null",
        "-oIdentitiesOnly=yes",
        "-oIdentityAgent=none",
        "-oCertificateFile=none",
        "-oAddKeysToAgent=no",
        "-oPasswordAuthentication=no",
        "-oKbdInteractiveAuthentication=no",
        "-oPreferredAuthentications=publickey",
        "-oClearAllForwardings=yes",
        "-oPermitLocalCommand=no",
        "-oControlMaster=no",
        "-oControlPath=none",
        "-oConnectTimeout=5",
        "-oConnectionAttempts=1",
        "-oServerAliveInterval=2",
        "-oServerAliveCountMax=2",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    expected.extend([
        format!("-oUserKnownHostsFile={}", known.display()),
        "-oHostKeyAlias=alias".into(),
        "-i".into(),
        key.to_str().unwrap().into(),
        "-p".into(),
        "22".into(),
        "-l".into(),
        "baton".into(),
        "--".into(),
        "...".into(),
        "exec /usr/local/libexec/baton-host-companion --serve /".into(),
    ]);
    assert_eq!(argv, expected);
    ssh.expected_host_identity = "bad:id".into();
    ssh.host = "bad;host".into();
    code(ssh.argv(), "ssh_host_identity");
    ssh.expected_host_identity = "valid".into();
    code(ssh.argv(), "ssh_host");
    ssh.host = "host".into();
    ssh.user = "bad:user".into();
    ssh.remote_config = "bad".into();
    code(ssh.argv(), "ssh_identity");
    ssh.user = "user".into();
    code(ssh.argv(), "ssh_remote_path");
    ssh.remote_config = "/valid".into();
    ssh.known_hosts = PathBuf::from(std::ffi::OsString::from_vec(vec![b'/', 0xff]));
    code(ssh.argv(), "ssh_path");
    ssh.known_hosts = known.clone();
    fs::set_permissions(&key, fs::Permissions::from_mode(0o644)).unwrap();
    code(ssh.argv(), "ssh_file");
    fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&known, "changed synthetic known_hosts").unwrap();
    code(ssh.argv(), "host_key_drift");
    assert_eq!(
        fs::read_to_string(&key).unwrap(),
        "synthetic identity stat fixture"
    );
}
#[test]
fn registration_module_has_no_io_clock_transport_or_authority_calls() {
    let source = include_str!("../src/registration.rs");
    for forbidden in [
        "HashSet",
        "std::fs",
        "std::net",
        "std::process",
        "std::time",
        "libc::",
        "File::",
        "Command::",
        "canonicalize(",
        "metadata(",
        "to_socket_addrs(",
        "Client::",
        "Store::",
        "Binding {",
        "Grant {",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected operation token: {forbidden}"
        );
    }
    // Nonexistent endpoint/roots/references are valid metadata, not existence proofs.
    let f = fixture();
    f.validate().unwrap();
    assert_eq!(
        decode_registration(&serde_json::to_vec(&f).unwrap()).unwrap(),
        f
    );
}
