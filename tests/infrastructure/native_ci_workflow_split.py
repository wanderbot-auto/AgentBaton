"""Exact reviewed split inverse for historical source readers, never runtime CI."""
import hashlib
import re

BASE_SHA256 = 'edfffb07b5d5c8ecb3039962f5ad54e77cd187a4afd33f89941564b89603bff4'
SPLIT_SHA256 = '46a898f5322de6edfc58fb770d77f35ebc396f1922af661a1ab25c64744808cc'
ORIGINAL_HEADER = '  desktop:\n    name: Product / ${{ matrix.os }}\n    runs-on: ${{ matrix.os }}\n    # Native tests, two application profiles, release audit and DMG packaging\n    # must finish on the same exact checkout. Per-test/product limits stay fixed.\n    timeout-minutes: 60\n    strategy:\n      fail-fast: false\n      max-parallel: 2\n      matrix:\n        os: [macos-15]\n    steps:\n'
ORIGINAL_STEP_SHA256 = ['2239b480ddc0d0fbe9979f3da6a52d850d51b69b4539cd7e8e6fbf9587420c18', '115e0283c13eced69f4383698c9901aea62132afa339f024ecc96587c729c67a', '152ad8b67453845b05a70d72c1d2beab88463433469f9ad54605be33d4e7b996', 'f53786d0312126a54d87ec87c3f7ea3d973af9d4a8968028133ba28baef3df5f', '37511450d193dab9e5f5c53aaa7fb8e2ec53dacecc61c4ce997f6ebbb0cd1d8e', 'b0e501f0c4d79eb18ab4127ffa65257a8cf086294c0e2f93dc3e8d4982e56cb4', '2fd615d3b81b4599572c8543f47307e9f61e9f0ef4aaf7879cfc050df4581ce3', 'fb7aa52b6b729f38891a5e6f6f3e5f0a5befaaf8344d7a1ddbc5e939455311fe', 'a6f0ff10590dfa398aa1489b629f9a63f4ce471190d3cda73699b058998c833f', '445d834beb6f9f8cad2285b0d3698cae7c75d20d59226c3f1702e4c387f38d12', 'fdacbf4e3cb595588731fa5e93e4a0d5c4452328a7e34694920937dfd727b006', '9f6a4932688fe2e30c1b3439e35a5d9c03f2a9dcc2dcdf59748435201058daa9', '66c6d4f64a65d74624e1dd5f2f2a02680329ce6dc011685fb52d83f541c67074', 'a14b8d0227b8c5cb4f2ca0f88b89a86e9b439656b20689c885ec8fe0b4cb92fe', 'e03239832ab58f2c1a37d3f75fca37644fcf17076bd023b41570ae8c9c0aa46f', '1be65206e3990f63c27a5af31bfcebd307d98fc82d0fe8955d1a859594982ea1', '0c4e0ccdb9cc5f9e3a8168e6edb865772a75c2c8b629bfb3ba1bfad91710d5e5', '0d6868d45c646b8ddf689dbb287712fce1ab493e59fba3f8d385d64010915135', 'c26e35374fd69b7883c1c3229d1007ef8faf101f1c21d476181e63defdecdf2d', '410372774e33bc369b2aa2032aef2527c5bb8bbbca10ad769a659848661f9511', '3fbfc3b567c18a25c818b5e1760663c263d1fdb18de2b04ef38ec812bef73174', 'd1f131cfddba8da635f7b9ccde55afed3057cdbce3c75217f93339945c371eba', '668f8dc849955248237a67efdfb58125f8554ff4469d7d97e4cb5dad194ed04d', '01e78e3ac555636a79d53b519c197ba47ac0388405f380fc6065b58e49b02954', '731d4682ac9ad8c526c7751bbcae4475ec4a56f5a89de3e34277aa55ebb74ec2']


def restore_native_ci_split(text):
    # Historical layer tests also call this on already peeled source.
    if '\n  desktop_acceptance:\n' not in text:
        return text
    if hashlib.sha256(text.encode()).hexdigest() != SPLIT_SHA256:
        raise ValueError('Complete reviewed Native CI split bytes changed')
    prefix = text.split('  desktop:\n', 1)[0]
    headers = list(re.finditer(r'^      - .*\n|^  [A-Za-z_][A-Za-z0-9_-]*:\n', text, re.M))
    blocks = [text[h.start():headers[i+1].start() if i+1 < len(headers) else len(text)]
              for i, h in enumerate(headers) if h.group().startswith('      - ')]
    originals = []
    for expected in ORIGINAL_STEP_SHA256:
        found = [b for b in blocks if hashlib.sha256(b.encode()).hexdigest() == expected]
        # Original checkout/setup actions are deliberately repeated for fresh
        # jobs. Their identical bytes still restore just one historical copy.
        if not found or any(b != found[0] for b in found):
            raise ValueError('Original Native command block missing or changed')
        originals.append(found[0])
    restored = prefix + ORIGINAL_HEADER + ''.join(originals)
    if hashlib.sha256(restored.encode()).hexdigest() != BASE_SHA256:
        raise ValueError('Native split did not restore complete exact c3 workflow')
    return restored
