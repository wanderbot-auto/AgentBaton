"""Pure strict source layers. No historical source, Native process or SQL executed."""
from pathlib import Path
import hashlib
import json
import re
import tomllib
import unittest

from tests.infrastructure.host19_native_smoke_source_fixture import (
    NATIVE_SMOKE_PATH,
    restore_native_smoke_before_schema19,
)
from tests.infrastructure.host_native_bridge_source_fixture import restore_host_native_bridge_v1
from tests.infrastructure.host_native_schema19_source_fixture import restore_host_native_schema18

ROOT = Path(__file__).resolve().parents[2]
DESKTOP_DEV_PROFILE_LAYER = '''# This is a separate Cargo workspace. Match the root's runtime-sensitive dev
# overrides so the helper can reuse dependencies in one checkout-local target.
[profile.dev.package.libsqlite3-sys]
opt-level = 3

[profile.dev.package.agentbaton-core]
opt-level = 2
debug = 1

[profile.dev.package.agentbaton-workspaces]
opt-level = 2
debug = 1

[profile.dev.package.serde_json]
opt-level = 3
debug = 1

[profile.dev.package.serde_core]
opt-level = 3
debug = 1

[profile.dev.package.rusqlite]
opt-level = 2
debug = 1

'''


def strip_desktop_dev_profile_layer(manifest):
    """Peel only the reviewed dev profiles; retain the historical parent pin."""
    if manifest.count(DESKTOP_DEV_PROFILE_LAYER) != 1:
        raise ValueError('Expected exactly the reviewed desktop dev profiles')
    return manifest.replace(DESKTOP_DEV_PROFILE_LAYER, '', 1)



# Versioned Host registration dependency layer. These are complete immutable
# inputs, not a discovered baseline or permission to regenerate historical pins.
HOST_DESKTOP_LOCK_V1_SHA256 = '439c10289c6d0b2b130046b732cb46c8988ed3414636864eb0fda171261231b5'
PRE_HOST_DESKTOP_LOCK_SHA256 = 'f1ad092cfc6b1317d259831e197e160d100de8c6ef292b075104531276667515'
HOST_CORE_EDGE_V1 = (
    'name = "agentbaton-core"\nversion = "0.1.0"\ndependencies = [\n'
    ' "agentbaton-config",\n "agentbaton-providers",\n'
    ' "agentbaton-remote-host",\n "agentbaton-workspaces",\n'
)
# Exact reviewed node includes its additional adjacent newline in439 bytes.
HOST_PACKAGE_V1 = (
    '[[package]]\nname = "agentbaton-remote-host"\nversion = "0.1.0"\n'
    'dependencies = [\n "libc",\n "rusqlite",\n "serde",\n'
    ' "serde_json",\n "sha2",\n]\n\n\n'
)


def strip_host_registration_lock_v1(raw):
    """Exact439 -> exactf1ad, then the caller retains its original libc inverse."""
    if not isinstance(raw, bytes):
        raise ValueError('Host lock inverse requires exact bytes')
    pin = hashlib.sha256(raw).hexdigest()
    if pin == PRE_HOST_DESKTOP_LOCK_SHA256:
        return raw
    if pin != HOST_DESKTOP_LOCK_V1_SHA256:
        raise ValueError('Unknown complete desktop lock input for Host layer v1')
    source = raw.decode('utf-8')
    if source.count(HOST_CORE_EDGE_V1) != 1 or source.count(HOST_PACKAGE_V1) != 1:
        raise ValueError('Host v1 dependency edge/node must each occur exactly once')
    parent = source.replace(
        HOST_CORE_EDGE_V1,
        HOST_CORE_EDGE_V1.replace(' "agentbaton-remote-host",\n', '', 1), 1,
    ).replace(HOST_PACKAGE_V1, '', 1).encode('utf-8')
    if hashlib.sha256(parent).hexdigest() != PRE_HOST_DESKTOP_LOCK_SHA256:
        raise ValueError('Host layer did not restore the complete historical parent')
    return parent


BG_STEP_NAME = 'Actual Native background registration IPC in generated private paths'
# Exact workflow at 4ce910b and the same bytes with the new Native step.
BASE_WORKFLOW_SHA256 = 'e9e70c5340ca711528e6df6f9c2434104e021068054bf74cb5d3f9efc90984e5'
COMPOSED_WORKFLOW_SHA256 = '97500089880e6810080169f62779b0446c3aee85eada81ba20541b516c4c0357'
CURRENT_MAC_MATRIX = '        os: [macos-15]\n'
PREVIOUS_MAC_MATRIX = '        os: [macos-15, macos-15-intel]\n'
BG_STEP_INDEX = 46
BG_STEP = (
    '      - name: Actual Native background registration IPC in generated private paths\n'
    '        run: |\n'
    '          set -euo pipefail\n'
    '          cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --lib background_service -- --test-threads=1\n'
    '          cargo build --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-background-service-smoke\n'
    '          cp target/debug/batond apps/desktop/src-tauri/target/debug/batond\n'
    '          python3 scripts/native-background-service-smoke.py --binary apps/desktop/src-tauri/target/debug/agentbaton-native-background-service-smoke --daemon target/debug/batond --output artifacts/native-smoke/background-registration\n'
)


def restore_previous_mac_matrix(text):
    """Reverse only the exact Apple Silicon matrix change for historical pins."""
    if text.count(CURRENT_MAC_MATRIX) != 1 or PREVIOUS_MAC_MATRIX in text:
        raise ValueError('Expected one Apple Silicon matrix')
    restored = text.replace(CURRENT_MAC_MATRIX, PREVIOUS_MAC_MATRIX, 1)
    if hashlib.sha256(restored.encode()).hexdigest() != COMPOSED_WORKFLOW_SHA256:
        raise ValueError('Apple Silicon matrix change did not preserve the exact parent workflow')
    return restored


def strip_background_native_smoke_layer(text):
    if CURRENT_MAC_MATRIX in text:
        text = restore_previous_mac_matrix(text)
    # Exact current whole input, exact adjacent block, exact old whole output.
    # No git-object dependency, old-code execution or arbitrary future hash list.
    if hashlib.sha256(text.encode()).hexdigest() != COMPOSED_WORKFLOW_SHA256:
        raise ValueError('Unknown or incomplete background Native workflow composition')
    headers = list(re.finditer(r'^      - .*\n', text, re.M))
    title = '      - name: ' + BG_STEP_NAME + '\n'
    indices = [i for i,h in enumerate(headers) if h.group(0)==title]
    if indices != [BG_STEP_INDEX] or BG_STEP_INDEX+1 >= len(headers):
        raise ValueError('Background Native step count/position changed')
    start,end = headers[BG_STEP_INDEX].start(),headers[BG_STEP_INDEX+1].start()
    if (text[start:end] != BG_STEP
            or headers[BG_STEP_INDEX-1].group(0) != '      - name: Native library tests with smoke features\n'
            or headers[BG_STEP_INDEX+1].group(0) != '      - name: Actual Native integration approval and early cancellation\n'):
        raise ValueError('Background Native exact block/neighbor changed')
    restored = text[:start]+text[end:]
    if hashlib.sha256(restored.encode()).hexdigest() != BASE_WORKFLOW_SHA256:
        raise ValueError('Background Native composition changed previous workflow')
    return restored


def lex(source):
    """Small audited Rust lexical scanner, not a Rust compiler."""
    i=0
    while i<len(source):
        if source[i].isspace():i+=1;continue
        if source.startswith('//',i):
            end=source.find('\n',i);i=len(source) if end<0 else end+1;continue
        if source.startswith('/*',i):
            depth=1;j=i+2
            while depth:
                if source.startswith('/*',j):depth+=1;j+=2
                elif source.startswith('*/',j):depth-=1;j+=2
                else:j+=1
                if j>len(source):raise ValueError('Unclosed comment')
            i=j;continue
        raw=re.match(r'(?:br|r)(\#*)"',source[i:])
        if raw:
            end=source.find('"'+raw[1],i+len(raw[0]))
            if end<0:raise ValueError('Unclosed raw string')
            j=end+1+len(raw[1]);yield source[i:j],i,j;i=j;continue
        char=re.match(r"'(?:\\.|[^'\\])'",source[i:])
        if char:j=i+len(char[0]);yield source[i:j],i,j;i=j;continue
        if source[i]=='"':
            j=i+1
            while j<len(source):
                if source[j]=='\\':j+=2;continue
                if source[j]=='"':j+=1;break
                j+=1
            yield source[i:j],i,j;i=j;continue
        word=re.match(r'[A-Za-z_][A-Za-z_0-9]*|[0-9]+',source[i:])
        if word:j=i+len(word[0]);yield source[i:j],i,j;i=j
        else:yield source[i],i,i+1;i+=1


def strip_fixture_cfg(source):
    pattern=re.compile(r'#\[cfg\(all\(feature = "native-smoke", debug_assertions, target_os = "macos"\)\)\]')
    while (match:=pattern.search(source)):
        tail=source[match.end():]
        header=tail.lstrip()
        field=header.startswith('background_mode:')
        statement=header.startswith('let ')
        p=q=b=0;seen=False;end=None
        for token,a,z in lex(tail):
            if token=='(':p+=1
            elif token==')':p-=1
            elif token=='[':q+=1
            elif token==']':q-=1
            elif token=='{':b+=1;seen=True
            elif token=='}':
                b-=1
                if seen and p==q==b==0 and not statement and not field:end=match.end()+z;break
            if token==';' and p==q==b==0:end=match.end()+z;break
            if field and token==',' and p==q==b==0:end=match.end()+z;break
        if end is None:raise ValueError('Unknown cfg item boundary')
        source=source[:match.start()]+source[end:]
    return source


def tokens(source):return [token for token,_,_ in lex(source)]
def token_sha(source):return hashlib.sha256(json_tokens(source)).hexdigest()
def json_tokens(source):
    return json.dumps(tokens(source),ensure_ascii=False,separators=(',',':')).encode()
def original_function(source,name):
    match=re.search(r'\bfn '+re.escape(name)+r'\s*\(',source)
    if not match:raise ValueError('Missing old function')
    start=source.index('{',match.end());depth=0
    for token,a,z in lex(source[start:]):
        if token=='{':depth+=1
        elif token=='}':
            depth-=1
            if depth==0:return source[match.start():start+z]
    raise ValueError('Unclosed old function')


def replace_tokens_once(values, old, new):
    before = tokens(old)
    positions = [i for i in range(len(values)) if values[i:i + len(before)] == before]
    if len(positions) != 1:
        raise ValueError(f'Expected one Native bridge composition site: {old}')
    index = positions[0]
    values[index:index + len(before)] = tokens(new)


def restored_parent_bridge_token_sha(source):
    """Remove the registration bridge to recover the formatted parent sources."""
    restored = strip_fixture_cfg(source)
    # The three exact functions are checked separately against #24 with and
    # without fixture cfg before their removal here.
    for name in BRIDGE_COMMAND_PINS:
        definition = original_function(restored, name)
        replace_at = restored.index(definition)
        prefix_start = restored.rfind('#[tauri::command]', 0, replace_at)
        if prefix_start < 0 or restored[prefix_start:replace_at].split() != ['#[tauri::command]', 'async']:
            raise ValueError(f'Native bridge command declaration changed: {name}')
        restored = restored[:prefix_start] + restored[replace_at + len(definition):]

    values = tokens(restored)
    replace_tokens_once(values, 'mod background_policy; mod background_service; mod background_window;',
                        'mod background_policy; mod background_window;')
    replace_tokens_once(values, '.manage(Runtime { directory, })', '.manage(Runtime { directory })')
    replace_tokens_once(values, 'tauri::generate_handler![background_service_status, '
                        'background_service_enable, background_service_disable,',
                        'tauri::generate_handler![')
    return hashlib.sha256(json.dumps(values, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()


# Parent is the workspace/recovery composition 4ce910b, normalized with rustfmt
# 2021 and skip_children=true, as the integration run formatted the bridge.
# #24's exact command functions and its original 19 tests have their own pins.
PARENT_BRIDGE_TOKEN_SHA256 = 'b5dcc8f034fc0f8f73b8b5d74cffd772f94b2ce4b81eda2c671240696641cb2d'
BACKGROUND_ENGINE_TOKEN_SHA256 = '7794f157346c723f5083631e3bc6fd715e84910af3479aa0bed60cafa00977be'
BRIDGE_COMMAND_PINS = {
    'background_service_status': (
        '7b1e0cc5b72e74104f6e1961be18b445fb8537b53c8897fa81df9794ad2cf5d5',
        '251952c29d5d24714605027a1cc9f9e49c1a8eeee00f6142f8777b42895ad9dc',
    ),
    'background_service_enable': (
        '5b6144f12b08f463dd0669619a66768e69dac058e403178cea35ea75663c6551',
        'f58ff64393e3e29364cb9e19564cce5f6b6701980bec46d0a3a9b234d73f2232',
    ),
    'background_service_disable': (
        '53fadcde009205e94799b524605dafd03a6e9b32ec79a62bffa49b61d9fc62bd',
        'f97e24e7b1a0e1dc8eb85c7c876d88cdd1df08e187986543647a6588ebc7c57a',
    ),
}
ORIGINAL_19_TEST_PINS = {'default_observation_is_disabled_and_creates_nothing': '4b16b3e0b6db9dd3aa180d4cea223921fb0314d44f461ec90e29a08a8abe12bd', 'next_login_registration_and_revocation_never_execute_a_program': 'f25143d68f0f3c14ef0f7dd299defc2b109ffddd5e28de7f11abe62bee1f61f2', 'registration_is_fixed_shell_free_and_xml_escaped': 'e2acde65044673960ccfa5644ffb214f8b5b8f4923eac019b48be5eccfec1c8e', 'xml_and_layout_reject_xml10_forbidden_scalars_without_rejecting_unicode': '9e76c23bee10a614cae54fb389e2472d02267c24df2b2198fd9702bc2f9ed9ec', 'missing_symlink_or_nonprivate_runtime_is_refused_without_repair': '9bf0596002f4245c2902af07484689c476ff83210f670367705aafac8794cd0f', 'runtime_replacement_invalidates_preconfirmed_enable_and_disable': '04ef7743db10c949f60adfcb1c8390c434856b09c0f643669d63651491ef89a2', 'runtime_replacement_after_receipt_is_never_reported_as_applied': 'aa21ef73df6a2112d66c6a7ea4f544b9ad98e82cd69e61710c589ccb015c75d9', 'nonroot_owned_world_writable_sticky_ancestor_is_refused': '92321c582b16a4ea99bd1c9df8d452f0127d1adaf492f6d6bdc5068a237889d3', 'custom_runtime_relative_and_unbundled_installations_are_refused': '2e149ff8ad65c47a60f36657e65c61b3a9a0d5a64959843f9c80f968ecfa4534', 'unknown_existing_plist_or_receipt_is_not_replaced_or_removed': 'a8ad727a0c663c2abd20afa57e6e16db513e5a3499af1ffabf99597e3161e9d5', 'symlink_leaf_ancestor_and_nonprivate_modes_are_refused': 'f17b71c3900c4c574d53ad1f36aad2057cb635fdcaea7cb111586fba62f61d78', 'hardlink_and_replaced_same_content_inode_do_not_gain_ownership': '8046d6e6a1e887c1913f2659233659da4ec75f2192e360f339c39c5f780f9599', 'changed_directory_or_executable_blocks_a_preconfirmed_snapshot': '3ffff0d09af0beb35195c1d5130192d79d4cb753697d644b649d619087a803c5', 'receipt_only_is_disabled_and_explicit_enable_can_complete_it': '7425728e3ca97e708ecb694fef0caa928af7216b3c5cfbb4095e060ab6bee87d', 'directory_replacement_after_receipt_reports_real_partial_and_never_registers': 'f4e3f4a4d4e01f7945814deb29683c24461e88bd07603d1e0bfa18124501738a', 'unknown_file_arriving_after_receipt_is_preserved_and_partial_is_not_success': 'b7d792814a0eb0ee67c2d3af243c7c65eb0bcb978c735dfbd54e69f8e35d2807', 'create_never_clobbers_an_existing_unknown_file': 'f4c0b8545e5ae6cc5aac76acdb30d31c425555325d19862fdd049609287450b5', 'detached_directory_descriptor_cannot_write_into_a_replaced_binding': '687059b71735c73b7b7053bed0f7036824580e707f8964fe64c2d72c9a53e5b7', 'report_contains_only_closed_state_and_never_private_paths': 'f2df32b651c15e6b21bc0e42c57ae7812ccb8cce9260bc7aef06c4cd21b4a4b0'}
UNCHANGED_FILE_PINS = {
    'apps/desktop/src-tauri/src/native_parent.rs': '92e3b3a947a5d2131a136b84de07face812f1bafc3b74ec128238c216fdd1caf',
    'apps/desktop/src-tauri/src/bin/native_smoke.rs': '95da64173f62be210d35fd626006f76ff33cda5ce96b333517c1244d23d2a4e0',
    'apps/desktop/src-tauri/src/bin/native_owner_dialog_smoke.rs': 'e7b8e4bf6d6656f2dc9e5c2c92c656dad2ea7ad6230cb97243f13f2fae2d7fdb',
    'apps/desktop/src-tauri/src/native_owner_dialog_test/controller.rs': 'c82d39abb4074971a8bf43b25454b5b6698f6b3b6a7b35beb85dbfb41b8d959d',
    'apps/desktop/src-tauri/src/native_owner_dialog_test/macos.rs': '636d710271160a6b4640d5da2702fc012cdc61c788bab9463cc40910559a7010',
    'apps/desktop/src-tauri/src/native_owner_dialog_test/integration_database.rs': '8730e078ebb48136802c3a614c44636d656c9336f3974b37d5bf018a83762755',
    'scripts/native-macos-smoke.sh': '79841c8b178ed3a68a6a62d13ccc81581e43798be3c1c29a00bf161a8b3dfefd',
    'scripts/native-owner-dialog-smoke.py': 'e2ca962c8e5d8af29bc04bb477c6d9d4fc876b7143b33ffafca8a730944fad94',
    'scripts/native-run.py': '205d76e38721760744bd4d889e34ab3d2cf436ebc94583fee40fa17029ffac0a',
}
NATIVE_ACCEPTANCE_FOLLOWUP_SHA256 = '4c676e3f72912687d30844afcee5653c9420106ffe43d7cf33babb09b19d1f0a'
PARENT_NATIVE_AUDIT_SHA256 = 'fff2bf0cefea1a1004a83cbef4caf31d6b9b99e5870fb22cc32527706ce0ad58'
PARENT_ENSURE_DAEMON_SHA256 = 'e9708f857866f23dd41f988124b85216b50a1ed432690b2adacfd4f175e30a2b'


class NativeBackgroundSourceBoundaries(unittest.TestCase):
    def test_complete_production_inverse_and_all_nineteen_original_tests(self):
        source = (ROOT/'apps/desktop/src-tauri/src/background_service.rs').read_text()
        self.assertEqual(token_sha(strip_fixture_cfg(source)), BACKGROUND_ENGINE_TOKEN_SHA256)
        self.assertEqual(len(ORIGINAL_19_TEST_PINS), 19)
        for name, pin in ORIGINAL_19_TEST_PINS.items():
            self.assertEqual(hashlib.sha256(original_function(source, name).encode()).hexdigest(), pin, name)

        raw_lib = (ROOT/'apps/desktop/src-tauri/src/lib.rs').read_bytes()
        lib = raw_lib.decode('utf-8')
        for name, (command_pin, production_pin) in BRIDGE_COMMAND_PINS.items():
            function = original_function(lib, name)
            self.assertEqual(token_sha(function), command_pin, name)
            self.assertEqual(token_sha(strip_fixture_cfg(function)), production_pin, name)
        predecessor = restore_host_native_bridge_v1(restore_host_native_schema18(raw_lib)).decode('utf-8')
        self.assertEqual(restored_parent_bridge_token_sha(predecessor), PARENT_BRIDGE_TOKEN_SHA256)
        self.assertEqual(hashlib.sha256(original_function(lib, 'ensure_daemon').encode()).hexdigest(),
                         PARENT_ENSURE_DAEMON_SHA256)

    def test_host_lock_v1_restores_exact_parent_before_original_libc_inverse(self):
        raw = (ROOT/'apps/desktop/src-tauri/Cargo.lock').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), HOST_DESKTOP_LOCK_V1_SHA256)
        parent = strip_host_registration_lock_v1(raw)
        self.assertEqual(hashlib.sha256(parent).hexdigest(), PRE_HOST_DESKTOP_LOCK_SHA256)
        self.assertEqual(strip_host_registration_lock_v1(parent), parent)
        before = tomllib.loads(parent.decode())['package']
        current = tomllib.loads(raw.decode())['package']
        self.assertEqual(len(current), len(before) + 1)
        by_id = {(p['name'], p['version'], p.get('source')): p for p in current}
        for old in before:
            expected = dict(old)
            if old['name'] == 'agentbaton-core':
                expected['dependencies'] = sorted(old['dependencies'] + ['agentbaton-remote-host'])
            self.assertEqual(by_id[(old['name'], old['version'], old.get('source'))], expected)

    def test_host_lock_v1_refuses_changed_edges_nodes_versions_checksums_and_unknown_bytes(self):
        raw = (ROOT/'apps/desktop/src-tauri/Cargo.lock').read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(), HOST_DESKTOP_LOCK_V1_SHA256)
        source = raw.decode()
        checksum = re.search(r'checksum = "([0-9a-f]{64})"', source).group(1)
        changed_checksum = ('0' if checksum[0] != '0' else '1') + checksum[1:]
        bad = {
            'missing Core edge': source.replace(HOST_CORE_EDGE_V1, HOST_CORE_EDGE_V1.replace(' "agentbaton-remote-host",\n', '', 1), 1),
            'duplicate Core edge': source.replace(HOST_CORE_EDGE_V1, HOST_CORE_EDGE_V1.replace(' "agentbaton-remote-host",\n', ' "agentbaton-remote-host",\n "agentbaton-remote-host",\n', 1), 1),
            'changed Core edge': source.replace(HOST_CORE_EDGE_V1, HOST_CORE_EDGE_V1.replace(' "agentbaton-remote-host",', ' "agentbaton-remote-host 0.2.0",'), 1),
            'missing Host node': source.replace(HOST_PACKAGE_V1, '', 1),
            'duplicate Host node': source + HOST_PACKAGE_V1,
            'lock format version': source.replace('version = 4\n', 'version = 3\n', 1),
            'Host node padding': source.replace(HOST_PACKAGE_V1, HOST_PACKAGE_V1[:-1], 1),
            'Host version': source.replace(HOST_PACKAGE_V1, HOST_PACKAGE_V1.replace('version = "0.1.0"', 'version = "0.2.0"'), 1),
            'Host dependency': source.replace(HOST_PACKAGE_V1, HOST_PACKAGE_V1.replace(' "libc",', ' "not-approved",'), 1),
            'unrelated checksum': source.replace(checksum, changed_checksum, 1),
            'unrelated resolved edge': source.replace(' "syn 3.0.9",', ' "syn 2.0.119",', 1),
            'unknown suffix': source + '\n# unreviewed\n',
            'CRLF normalization': source.replace('\n', '\r\n'),
            'UTF8 BOM': '\ufeff' + source,
        }
        for label, changed in bad.items():
            self.assertNotEqual(changed, source, label)
            with self.subTest(label=label), self.assertRaises(ValueError):
                strip_host_registration_lock_v1(changed.encode())
        parent = strip_host_registration_lock_v1(raw)
        for unknown in (b'', b'unknown', parent + b'\n', parent.replace(b'\n', b'\r\n'), b'\xff', source):
            with self.subTest(unknown=type(unknown).__name__), self.assertRaises(ValueError):
                strip_host_registration_lock_v1(unknown)

    def test_all_parent_harness_guards_lock_and_cargo_source_are_exact(self):
        from tests.infrastructure.schema18_source_fixture import restore
        inverse = (Path(__file__).parent/'fixtures/native_acceptance_followup.json').read_bytes()
        self.assertEqual(hashlib.sha256(inverse).hexdigest(), NATIVE_ACCEPTANCE_FOLLOWUP_SHA256)
        layers = json.loads(inverse)
        for path, pin in UNCHANGED_FILE_PINS.items():
            raw = (ROOT/path).read_bytes()
            if path == NATIVE_SMOKE_PATH:
                raw = restore_native_smoke_before_schema19(path, raw)
            if path in layers:
                layer = layers[path]
                self.assertEqual(hashlib.sha256(raw).hexdigest(), layer['after'], path)
                raw = restore(raw.decode(), layer['edits'], pin, AssertionError).encode()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), pin, path)

        lock = strip_host_registration_lock_v1(
            (ROOT/'apps/desktop/src-tauri/Cargo.lock').read_bytes()
        ).decode('utf-8')
        addition = ('name = "agentbaton-desktop"\nversion = "0.1.0"\ndependencies = [\n'
                    ' "agentbaton-core",\n "libc",\n')
        self.assertEqual(lock.count(addition), 1)
        parent_lock = lock.replace(addition, addition.removesuffix(' "libc",\n'), 1)
        self.assertEqual(hashlib.sha256(parent_lock.encode()).hexdigest(),
                         '4d47820152c17d63b61829fa7d83d2fbbdb4c9cbbfdbd03168207f0e442e068d')

        manifest = strip_desktop_dev_profile_layer((ROOT/'apps/desktop/src-tauri/Cargo.toml').read_text())
        additions = (
            '# Reuse the exact already-locked POSIX ABI for per-user filesystem ownership.\nlibc = "=0.2.191"\n',
            '\n# Private generated-tree registration IPC; never enabled in application bundles.\n'
            '[[bin]]\nname = "agentbaton-native-background-service-smoke"\n'
            'path = "src/bin/native_background_service_smoke.rs"\nrequired-features = ["native-smoke"]\n',
        )
        for addition in additions:
            self.assertEqual(manifest.count(addition), 1)
            manifest = manifest.replace(addition, '', 1)
        self.assertEqual(hashlib.sha256(manifest.encode()).hexdigest(),
                         '3b6bb5205adb7610d639fb42cb7154c189e04030d8e18ccf71d2c42070bcb028')

    def test_dev_profile_layer_matches_root_and_refuses_changed_or_repeated_layer(self):
        layer = tomllib.loads(DESKTOP_DEV_PROFILE_LAYER)
        root = tomllib.loads((ROOT/'Cargo.toml').read_text())
        self.assertEqual(set(layer), {'profile'})
        self.assertEqual(set(layer['profile']), {'dev'})
        for package, settings in layer['profile']['dev']['package'].items():
            self.assertEqual(settings, root['profile']['dev']['package'][package])
        manifest = (ROOT/'apps/desktop/src-tauri/Cargo.toml').read_text()
        for corrupt in (
            manifest.replace('opt-level = 2', 'opt-level = 0', 1),
            manifest.replace('debug = 1', 'debug = 0', 1),
            manifest + DESKTOP_DEV_PROFILE_LAYER,
            manifest.replace(DESKTOP_DEV_PROFILE_LAYER, '', 1),
        ):
            with self.assertRaises(ValueError):
                strip_desktop_dev_profile_layer(corrupt)

    def test_workflow_normalizers_peel_only_the_exact_new_layer(self):
        from tests.infrastructure import test_schema17_migration_workflow as workflow
        composed = (ROOT/'.github/workflows/product.yml').read_text()
        parent = strip_background_native_smoke_layer(composed)
        for normalize in (workflow.strip_conversation_schema18, workflow.strip_private_browser_python):
            self.assertEqual(normalize(composed), normalize(parent))
            with self.assertRaises(ValueError):
                normalize(composed.replace('contents: read', 'contents: write', 1))

    def test_exact_new_mac_step_recovers_entire_parent_workflow(self):
        text=(ROOT/'.github/workflows/product.yml').read_text();old=strip_background_native_smoke_layer(text)
        self.assertEqual(hashlib.sha256(old.encode()).hexdigest(),BASE_WORKFLOW_SHA256)
        self.assertEqual(text.count(BG_STEP),1)
        self.assertEqual(restore_previous_mac_matrix(text).replace(BG_STEP,'',1),old)
        for predecessor in ('cargo build --release --locked --bin batond --bin baton',
                            'bash scripts/prepare-desktop-sidecars.sh'):
            # The preparation stays in the original host tests. The new
            # step uses their verified binaries and avoids rebuilding them.
            desktop = text[text.index('  desktop:\n'):]
            self.assertLess(desktop.index(predecessor), desktop.index(BG_STEP))
            self.assertNotIn(predecessor, BG_STEP)
        self.assertNotIn('continue-on-error',BG_STEP);self.assertNotIn('always()',BG_STEP)
        self.assertNotIn('timeout-minutes',BG_STEP)

    def test_any_unknown_missing_duplicate_moved_budget_or_permission_change_refuses(self):
        text=(ROOT/'.github/workflows/product.yml').read_text()
        variants=[text.replace(BG_STEP,'',1),text.replace(BG_STEP,BG_STEP*2,1),
                  BG_STEP+text.replace(BG_STEP,'',1),text+'# unknown\n',text.replace('\n','\r\n'),
                  text.replace('timeout-minutes: 60','timeout-minutes: 61',1),
                  text.replace('contents: read','contents: write',1),
                  text.replace('--lib background_service','--lib background_service --ignored',1),
                  text.replace(BG_STEP_NAME,'Hidden unknown name',1)]
        for value in variants:
            with self.subTest(mutation=value[-40:]),self.assertRaises(ValueError):strip_background_native_smoke_layer(value)

    def test_fixture_is_explicit_nonserializable_and_no_home_or_runtime_selector(self):
        fixture=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/fixture.rs').read_text()
        self.assertIn('Fixture(Option<Arc<Fixture>>)',fixture)
        self.assertIn('let Some(fixture) = fixture else',fixture)
        missing=fixture[fixture.index('let Some(fixture) = fixture else'):fixture.index('if fixture.begin')]
        self.assertIn('return refusal(Code::StateChanged)',missing)
        self.assertNotIn('Layout::current',missing)
        self.assertNotIn('Serialize',fixture[fixture.index('pub(crate) struct Fixture'):fixture.index('impl Fixture')].replace('// Not serializable.',''))
        self.assertIn('Path::new("/private/tmp")',fixture)
        for bad in ('set_var(', 'env::temp_dir(', 'launchctl', 'Command::', 'std::env::args'):
            self.assertNotIn(bad,fixture)
        self.assertIn('s.fixture.resolve_layout(runtime)',fixture)
        self.assertIn('runtime != self.core_path',fixture)
        self.assertIn('self.root.recheck()?',fixture);self.assertIn('self.core.recheck()?',fixture)
        self.assertIn('self.runtime.recheck()?',fixture)
        self.assertIn('root.identities().contains(&account_identity)',fixture)
        self.assertIn('account.identities().contains(&root_identity)',fixture)
        self.assertIn('>= 104',fixture);self.assertIn('Err(Code::CustomRuntime)',fixture)

    def test_real_command_sheet_and_fs_are_rust_authority_not_renderer_report(self):
        fixture=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/fixture.rs').read_text()
        report=original_function(fixture,'ui_report')
        self.assertNotIn('s.index +=',report);self.assertNotIn('before_click',report);self.assertNotIn('apply(',report)
        self.assertIn('!s.completed',report);self.assertIn('s.inflight',report);self.assertIn('s.ui_seen',report)
        for required in ('actual_darwin_home_abi_agrees','receipt_before_plist','source_order_only_not_terminal_chronology','explicit_bridge_only_ui_remains_disabled','actual_os_login'):
            self.assertIn(required,fixture)
        sheet=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/sheet.rs').read_text()
        for required in ('caller.ns_window()', 'parent.attachedSheet()', 'sheet.sheetParent()', 'v.window()', 'size.height', 'size.width', 'button.isEnabled()', 'title != 1 || body != 1'):
            target,whole=tokens(required),tokens(sheet)
            self.assertTrue(any(whole[i:i+len(target)]==target for i in range(len(whole))),required)
        harness=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/mod.rs').read_text()
        whole=tokens(harness)
        for required in ('crate::dispatch_production(invoke)', 'obj.len() != 2', 'len() > 128',
                         'button.performClick(None)', 'crate::require_current_database_schema(&v)?',
                         'Mode::Fixture(Some(fixture.clone()))'):
            target=tokens(required)
            self.assertTrue(any(whole[i:i+len(target)]==target for i in range(len(whole))),required)
        self.assertLess(harness.index('before_click('),harness.index('button.performClick(None)'))

    def test_presentation_is_exact_original_pure_algorithm_and_message(self):
        parent=(ROOT/'apps/desktop/src-tauri/src/native_parent.rs').read_text()
        presentation=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/presentation.rs').read_text()
        self.assertEqual(tokens(original_function(parent,'review_prompts')),tokens(original_function(presentation,'review_prompts')))
        old=(ROOT/'apps/desktop/src-tauri/src/background_service.rs').read_text()
        fixture=(ROOT/'apps/desktop/src-tauri/src/native_background_service_test/fixture.rs').read_text()
        pattern=r'let message\s*=\s*format!\(("(?:\\.|[^"\\])*")'
        self.assertEqual(re.findall(pattern,old),re.findall(pattern,fixture))

    def test_default_audit_keeps_old_whole_source_and_tests_feature_only_controls(self):
        source=(ROOT/'scripts/native-audit-release.sh').read_text()
        added='native_background_service_fixture_report|native_background_fixture_alarm|abg-registration-fixture-v1|'
        self.assertEqual(source.count(added),1)
        self.assertEqual(hashlib.sha256(source.replace(added,'',1).encode()).hexdigest(),PARENT_NATIVE_AUDIT_SHA256)
        lib=(ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        gate='#[cfg(all(feature = "native-smoke", debug_assertions, target_os = "macos"))]'
        self.assertIn(gate+'\npub mod native_background_service_test;',lib)
        builder = original_function(lib, 'desktop_builder')
        self.assertIn(gate+'\n            background_mode: background_service::smoke::Mode::Production,', builder)
        engine=(ROOT/'apps/desktop/src-tauri/src/background_service.rs').read_text()
        self.assertIn(gate+'\n    pub(crate) mod smoke',engine)
        binary=(ROOT/'apps/desktop/src-tauri/src/bin/native_background_service_smoke.rs').read_text()
        self.assertIn('#[cfg(not(debug_assertions))]',binary);self.assertIn('compile_error!',binary)
        self.assertIn('native-smoke is debug-only',lib)
