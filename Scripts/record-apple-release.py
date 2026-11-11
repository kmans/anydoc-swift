#!/usr/bin/env python3
"""Record a built Apple candidate's checksum and exact native input fingerprint."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parent.parent
record = root / 'Native/apple-release.json'
archive = root / '.build/artifacts/AnyDocSwiftBridge.xcframework.zip'
paths = []
for base in ['Rust/anydoc', 'Rust/anydoc-swift-bridge/src', 'Native/include', 'Native/framework', 'Native/licenses', 'Native/patches']:
    paths += [p for p in (root/base).rglob('*') if p.is_file() and 'target' not in p.parts]
for name in ['Rust/anydoc-swift-bridge/Cargo.toml', 'Rust/anydoc-swift-bridge/Cargo.lock', 'rust-toolchain.toml', 'LICENSE', 'THIRD_PARTY_NOTICES.txt']:
    paths.append(root/name)
paths += list((root/'Scripts').glob('build-*.sh'))
inputs = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(set(paths))}
data = json.loads(record.read_text())
data['sha256'] = hashlib.sha256(archive.read_bytes()).hexdigest()
data['nativeInputsSHA256'] = hashlib.sha256(json.dumps(inputs, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
data['nativeInputs'] = inputs
data['nativeInputsAlgorithm'] = 'SHA-256 of sorted compact JSON mapping relative paths to SHA-256 file hashes'
record.write_text(json.dumps(data, indent=2) + '\n')
print(data['sha256'])
