#!/usr/bin/env python3
"""Verify pinned native source and its reviewed patch; refresh only intentionally."""
from pathlib import Path
import argparse, difflib, hashlib, io, json, subprocess, tarfile, tempfile
ROOT = Path(__file__).resolve().parent.parent

def digest(data): return hashlib.sha256(data).hexdigest()
def files(SOURCE):
    return {p.relative_to(SOURCE).as_posix(): p.read_bytes() for p in SOURCE.rglob('*')
            if p.is_file() and 'target' not in p.relative_to(SOURCE).parts and p.name != 'VENDORED-SOURCE.json'}

def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--refresh', action='store_true')
    parser.add_argument('--crate', choices=['anydoc', 'pdf-inspector'], default='anydoc')
    args = parser.parse_args()
    SOURCE = ROOT / 'Rust' / args.crate
    provenance = json.loads((SOURCE/'VENDORED-SOURCE.json').read_text())
    stem = args.crate + '-' + provenance['version'] + '-fidelity'
    PATCH = ROOT / 'Native/patches' / (stem + '.patch')
    HASHES = ROOT / 'Native/patches' / (stem + '-files.json')
    actual = files(SOURCE)
    if args.archive:
        data = args.archive.read_bytes()
        if digest(data) != provenance['sha256']: raise ValueError('original crate archive checksum differs')
        with tarfile.open(fileobj=io.BytesIO(data)) as archive:
            original = {m.name.split('/',1)[1]: archive.extractfile(m).read()
                        for m in archive.getmembers() if m.isfile()}
        if args.refresh:
            parts = []
            for name in sorted(set(original) | set(actual)):
                before = original.get(name,b''); after = actual.get(name,b'')
                if before == after: continue
                parts.extend(difflib.unified_diff(before.decode().splitlines(keepends=True), after.decode().splitlines(keepends=True),
                    fromfile='a/'+name if name in original else '/dev/null', tofile='b/'+name if name in actual else '/dev/null'))
            PATCH.write_text(''.join(parts))
        # Reapply the patch to the checksum-pinned archive; compare every byte.
        with tempfile.TemporaryDirectory(prefix='anydoc-provenance-') as folder:
            folder = Path(folder).resolve()
            for name,content in original.items():
                path = folder/name
                if not path.resolve().is_relative_to(folder): raise ValueError('unsafe archive entry')
                path.parent.mkdir(parents=True,exist_ok=True); path.write_bytes(content)
            subprocess.run(['patch','--batch','-p1','-i',str(PATCH)],cwd=folder,check=True,capture_output=True)
            reconstructed = {p.relative_to(folder).as_posix():p.read_bytes() for p in folder.rglob('*') if p.is_file()}
            if reconstructed != actual: raise ValueError('patch does not reconstruct the vendored source exactly')
    if args.refresh:
        if not args.archive: raise ValueError('--refresh requires the original verified archive')
        HASHES.write_text(json.dumps({'archiveSHA256':provenance['sha256'], 'patchSHA256':digest(PATCH.read_bytes()),
            'files':{name:digest(data) for name,data in sorted(actual.items())}},indent=2)+'\n')
    expected = json.loads(HASHES.read_text())
    if expected['files'] != {name:digest(data) for name,data in sorted(actual.items())}: raise ValueError('vendored source changed; review and refresh the patch')
    if expected['patchSHA256'] != digest(PATCH.read_bytes()): raise ValueError('patch checksum differs')
    print('Verified pinned ' + args.crate + ' source and reviewed fidelity patch')

if __name__ == '__main__': main()
