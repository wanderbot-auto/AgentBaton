"""Pure strict source layers. No historical source, Native process or SQL executed."""
from pathlib import Path
import hashlib
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
BG_STEP_NAME = 'Actual Native background registration IPC in generated private paths'
BASE_WORKFLOW_SHA256 = '19424794adab93619e420f1ec5304f0d386fe0dcea15d743885b5a2308e3f526'
COMPOSED_WORKFLOW_SHA256 = 'e25d0db6e55d054a6a5c91fedc9ac810bde981d02366ae2ba94809b1c22af90c'
BG_STEP_INDEX = 37
BG_STEP = '      - name: Actual Native background registration IPC in generated private paths\n        run: |\n          set -euo pipefail\n          # Matching normal17 tools/sidecars; later original steps reuse caches.\n          cargo build --locked --bin batond --bin baton\n          cargo build --release --locked --bin batond --bin baton\n          bash scripts/prepare-desktop-sidecars.sh\n          cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --lib background_service -- --test-threads=1\n          cargo build --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-background-service-smoke\n          cp target/debug/batond apps/desktop/src-tauri/target/debug/batond\n          python3 scripts/native-background-service-smoke.py --binary apps/desktop/src-tauri/target/debug/agentbaton-native-background-service-smoke --daemon target/debug/batond --output artifacts/native-smoke/background-registration\n'


def strip_background_native_smoke_layer(text):
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
    if text[start:end] != BG_STEP or headers[BG_STEP_INDEX+1].group(0) != '      - name: Core and native host tests\n':
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
    import json
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


def production_tokens(path,source):
    restored=strip_fixture_cfg(source)
    values=tokens(restored)
    if path.endswith('/lib.rs'):
        # The sole ungated adaptation is an equivalent pure closure block.
        before=tokens('move || { background_service::status(&directory) }')
        after=tokens('move || background_service::status(&directory)')
        positions=[i for i in range(len(values)) if values[i:i+len(before)]==before]
        if len(positions)!=1:raise ValueError('Unknown status closure adaptation')
        i=positions[0];values[i:i+len(before)]=after
        before=tokens('.manage(Runtime { directory, })');after=tokens('.manage(Runtime { directory })')
        positions=[i for i in range(len(values)) if values[i:i+len(before)]==before]
        if len(positions)!=1:raise ValueError('Unknown default builder adaptation')
        i=positions[0];values[i:i+len(before)]=after
        before=tokens('.unwrap_or_else(|_| background_service::observation_failed()))')
        after=tokens('.unwrap_or_else(|_| background_service::observation_failed()),)')
        positions=[i for i in range(len(values)) if values[i:i+len(before)]==before]
        if len(positions)!=1:raise ValueError('Unknown status trailing-comma adaptation')
        i=positions[0];values[i:i+len(before)]=after
    return values

PRODUCTION_TOKEN_PINS = {'apps/desktop/src-tauri/src/lib.rs': '017cac89ac762bede996a7afdec3ccd699cc4a04b85e15cc745bfe121e34bc7f', 'apps/desktop/src-tauri/src/background_service.rs': '7794f157346c723f5083631e3bc6fd715e84910af3479aa0bed60cafa00977be'}
ORIGINAL_19_TEST_PINS = {'default_observation_is_disabled_and_creates_nothing': '4b16b3e0b6db9dd3aa180d4cea223921fb0314d44f461ec90e29a08a8abe12bd', 'next_login_registration_and_revocation_never_execute_a_program': 'f25143d68f0f3c14ef0f7dd299defc2b109ffddd5e28de7f11abe62bee1f61f2', 'registration_is_fixed_shell_free_and_xml_escaped': 'e2acde65044673960ccfa5644ffb214f8b5b8f4923eac019b48be5eccfec1c8e', 'xml_and_layout_reject_xml10_forbidden_scalars_without_rejecting_unicode': '9e76c23bee10a614cae54fb389e2472d02267c24df2b2198fd9702bc2f9ed9ec', 'missing_symlink_or_nonprivate_runtime_is_refused_without_repair': '9bf0596002f4245c2902af07484689c476ff83210f670367705aafac8794cd0f', 'runtime_replacement_invalidates_preconfirmed_enable_and_disable': '04ef7743db10c949f60adfcb1c8390c434856b09c0f643669d63651491ef89a2', 'runtime_replacement_after_receipt_is_never_reported_as_applied': 'aa21ef73df6a2112d66c6a7ea4f544b9ad98e82cd69e61710c589ccb015c75d9', 'nonroot_owned_world_writable_sticky_ancestor_is_refused': '92321c582b16a4ea99bd1c9df8d452f0127d1adaf492f6d6bdc5068a237889d3', 'custom_runtime_relative_and_unbundled_installations_are_refused': '2e149ff8ad65c47a60f36657e65c61b3a9a0d5a64959843f9c80f968ecfa4534', 'unknown_existing_plist_or_receipt_is_not_replaced_or_removed': 'a8ad727a0c663c2abd20afa57e6e16db513e5a3499af1ffabf99597e3161e9d5', 'symlink_leaf_ancestor_and_nonprivate_modes_are_refused': 'f17b71c3900c4c574d53ad1f36aad2057cb635fdcaea7cb111586fba62f61d78', 'hardlink_and_replaced_same_content_inode_do_not_gain_ownership': '8046d6e6a1e887c1913f2659233659da4ec75f2192e360f339c39c5f780f9599', 'changed_directory_or_executable_blocks_a_preconfirmed_snapshot': '3ffff0d09af0beb35195c1d5130192d79d4cb753697d644b649d619087a803c5', 'receipt_only_is_disabled_and_explicit_enable_can_complete_it': '7425728e3ca97e708ecb694fef0caa928af7216b3c5cfbb4095e060ab6bee87d', 'directory_replacement_after_receipt_reports_real_partial_and_never_registers': 'f4e3f4a4d4e01f7945814deb29683c24461e88bd07603d1e0bfa18124501738a', 'unknown_file_arriving_after_receipt_is_preserved_and_partial_is_not_success': 'b7d792814a0eb0ee67c2d3af243c7c65eb0bcb978c735dfbd54e69f8e35d2807', 'create_never_clobbers_an_existing_unknown_file': 'f4c0b8545e5ae6cc5aac76acdb30d31c425555325d19862fdd049609287450b5', 'detached_directory_descriptor_cannot_write_into_a_replaced_binding': '687059b71735c73b7b7053bed0f7036824580e707f8964fe64c2d72c9a53e5b7', 'report_contains_only_closed_state_and_never_private_paths': 'f2df32b651c15e6b21bc0e42c57ae7812ccb8cce9260bc7aef06c4cd21b4a4b0'}
UNCHANGED_FILE_PINS = {'apps/desktop/src-tauri/Cargo.lock': 'f1ad092cfc6b1317d259831e197e160d100de8c6ef292b075104531276667515', 'apps/desktop/src-tauri/src/native_parent.rs': '15387d3493af707f9e50b019dc04fe7b6e7b0d95bc68623791a875123a80e9da', 'apps/desktop/src-tauri/src/bin/native_smoke.rs': '6afdd3becc2d278f5032c96567bf7cbeb41d2e36ea612d997690806473edc194', 'apps/desktop/src-tauri/src/bin/native_owner_dialog_smoke.rs': 'e7b8e4bf6d6656f2dc9e5c2c92c656dad2ea7ad6230cb97243f13f2fae2d7fdb', 'apps/desktop/src-tauri/src/native_owner_dialog_test/controller.rs': '77521457c597183eac0ddf37073713e22e223717d608576e23f5dfad71614828', 'apps/desktop/src-tauri/src/native_owner_dialog_test/macos.rs': '636d710271160a6b4640d5da2702fc012cdc61c788bab9463cc40910559a7010', 'apps/desktop/src-tauri/src/native_owner_dialog_test/integration_database.rs': 'b73668985e0367f8f4d8fb2afd3f45fe17077f3a28dc162cb2d0ac390833d138', 'scripts/native-macos-smoke.sh': '22931a868be18fdc3b54689ced42a76392e80f42ca11e7ed60d11fa4b1c88cfd', 'scripts/native-owner-dialog-smoke.py': '583ef9625cfca7c664bb675cabadd38dddf047babc16c7f376d0e2559fe07dba', 'scripts/native-run.py': '314895a19a8a2194f51f1acb5222c05d30f2c5fe1264dcfe4fb9d8a3604541c1'}
OLD_COMPOSITION_PINS = {'apps/desktop/src-tauri/Cargo.toml': '0f32bd15f1052232cafa688f1a62d7c635943e469bc1653f2060bfe3d6c9938f', 'scripts/native-audit-release.sh': 'fff2bf0cefea1a1004a83cbef4caf31d6b9b99e5870fb22cc32527706ce0ad58', 'tests/infrastructure/test_schema17_migration_workflow.py': 'c20f445140c07a86f7d0d1cc9c96db59fbdf3397ecb75f176de03a8fdcd3e04f', 'tests/infrastructure/test_private_browser_python.py': '578bc980e0c707dcc9f15b85346e22409fc631059e98154c3e11c222b2e4a655', 'tests/infrastructure/test_integration_setup_diagnostics.py': 'b962181836ed27b23af5187c7cedefb94a0cb7df39fe08520e43a16d910445c4'}
ENSURE_DAEMON_SHA256 = 'fe08cd65f492bf24f2f8021e77ea4bfc3c6fb925a234ede4d1eafe451c7cd4d6'


class NativeBackgroundSourceBoundaries(unittest.TestCase):
    def test_complete_production_inverse_and_all_nineteen_original_tests(self):
        import json
        for path,pin in PRODUCTION_TOKEN_PINS.items():
            values=production_tokens(path,(ROOT/path).read_text())
            self.assertEqual(hashlib.sha256(json.dumps(values,ensure_ascii=False,separators=(',',':')).encode()).hexdigest(),pin,path)
        source=(ROOT/'apps/desktop/src-tauri/src/background_service.rs').read_text()
        self.assertEqual(len(ORIGINAL_19_TEST_PINS),19)
        for name,pin in ORIGINAL_19_TEST_PINS.items():
            self.assertEqual(hashlib.sha256(original_function(source,name).encode()).hexdigest(),pin,name)
        lib=(ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        self.assertEqual(hashlib.sha256(original_function(lib,'ensure_daemon').encode()).hexdigest(),ENSURE_DAEMON_SHA256)

    def test_all_old_harness_guards_lock_and_cargo_source_are_exact(self):
        for path,pin in UNCHANGED_FILE_PINS.items():
            self.assertEqual(hashlib.sha256((ROOT/path).read_bytes()).hexdigest(),pin,path)
        source=(ROOT/'apps/desktop/src-tauri/Cargo.toml').read_text()
        addition='\n# Private generated-tree registration IPC; never enabled in application bundles.\n[[bin]]\nname = "agentbaton-native-background-service-smoke"\npath = "src/bin/native_background_service_smoke.rs"\nrequired-features = ["native-smoke"]\n'
        self.assertEqual(source.count(addition),1)
        self.assertEqual(hashlib.sha256(source.replace(addition,'',1).encode()).hexdigest(),OLD_COMPOSITION_PINS['apps/desktop/src-tauri/Cargo.toml'])

    def test_three_legacy_python_layers_restore_entire_original_bytes(self):
        path='tests/infrastructure/test_schema17_migration_workflow.py';source=(ROOT/path).read_text()
        source=source.replace('from tests.infrastructure import test_background_service_native_smoke_source as BG\n','',1)
        branch='    if BG.BG_STEP_NAME in text:\n        text = BG.strip_background_native_smoke_layer(text)\n'
        self.assertEqual(source.count(branch),1);source=source.replace(branch,'',1)
        self.assertEqual(hashlib.sha256(source.encode()).hexdigest(),OLD_COMPOSITION_PINS[path])
        for path,new,old,count in [
            ('tests/infrastructure/test_private_browser_python.py',"text=BG.strip_background_native_smoke_layer((ROOT/'.github/workflows/product.yml').read_text())","text=(ROOT/'.github/workflows/product.yml').read_text()",2),
            ('tests/infrastructure/test_integration_setup_diagnostics.py','text = BG.strip_background_native_smoke_layer(F.WORKFLOW.read_text())','text = F.WORKFLOW.read_text()',3)]:
            source=(ROOT/path).read_text()
            declaration='from tests.infrastructure import test_background_service_native_smoke_source as BG\n\n\n'
            self.assertEqual(source.count(declaration),1);source=source.replace(declaration,'',1)
            self.assertEqual(source.count(new),count);source=source.replace(new,old)
            self.assertEqual(hashlib.sha256(source.encode()).hexdigest(),OLD_COMPOSITION_PINS[path])

    def test_exact_new_mac_step_recovers_entire_old_workflow_before_core(self):
        text=(ROOT/'.github/workflows/product.yml').read_text();old=strip_background_native_smoke_layer(text)
        self.assertEqual(hashlib.sha256(old.encode()).hexdigest(),BASE_WORKFLOW_SHA256)
        self.assertEqual(text.count(BG_STEP),1)
        self.assertEqual(text.replace(BG_STEP,'',1),old)
        self.assertIn('cargo build --release --locked --bin batond --bin baton',BG_STEP)
        self.assertIn('bash scripts/prepare-desktop-sidecars.sh',BG_STEP)
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
        self.assertEqual(hashlib.sha256(source.replace(added,'',1).encode()).hexdigest(),OLD_COMPOSITION_PINS['scripts/native-audit-release.sh'])
        lib=(ROOT/'apps/desktop/src-tauri/src/lib.rs').read_text()
        gate='#[cfg(all(feature = "native-smoke", debug_assertions, target_os = "macos"))]'
        self.assertIn(gate+'\npub mod native_background_service_test;',lib)
        engine=(ROOT/'apps/desktop/src-tauri/src/background_service.rs').read_text()
        self.assertIn(gate+'\n    pub(crate) mod smoke',engine)
        binary=(ROOT/'apps/desktop/src-tauri/src/bin/native_background_service_smoke.rs').read_text()
        self.assertIn('#[cfg(not(debug_assertions))]',binary);self.assertIn('compile_error!',binary)
        self.assertIn('native-smoke is debug-only',lib)
