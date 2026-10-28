"""Keep Tauri's standard installer, with one save-safety gate before old uninstall.

The checked-in template is extracted from the pinned Windows CLI, not redesigned.
Run --check in builds; regeneration must be reviewed when the CLI changes.
"""
import argparse
import hashlib
from pathlib import Path

args = argparse.ArgumentParser()
args.add_argument('--check', action='store_true')
options = args.parse_args()
root = Path(__file__).resolve().parents[1]
binary = (root / 'node_modules/@tauri-apps/cli-win32-x64-msvc/cli.win32-x64-msvc.node').read_bytes()
start = binary.index(b'Unicode true')
null = binary.index(b'\0', start)
end = binary.rfind(b'FunctionEnd\r\n', start, null) + len(b'FunctionEnd\r\n')
original = binary[start:end]
expected = '1114a136876d667ca7ba77218d4e966a625d1078dcba18484caae8f7bffb8626'
if hashlib.sha256(original).hexdigest() != expected:
    raise SystemExit('Tauri standard NSIS template changed. Review the new template before regenerating.')
text = original.decode('utf-8').replace('\r\n', '\n')
anchor = '  reinst_uninstall:\n'
if text.count(anchor) != 1:
    raise SystemExit('Expected one old-uninstall insertion point')
patched = text.replace(anchor, anchor + '    ; Qingjian: re-check immediately before a legacy uninstaller can run.\n'
    '    !insertmacro CheckIfAppIsRunning "${MAINBINARYNAME}.exe" "${PRODUCTNAME}"\n')
target = root / 'src-tauri/installer/standard-template.nsi'
license_target = target.parent / 'TAURI-LICENSE-MIT'
license_text = (root / 'node_modules/@tauri-apps/cli/LICENSE_MIT').read_text(encoding='utf-8')
if options.check:
    if target.read_text(encoding='utf-8') != patched or license_target.read_text(encoding='utf-8') != license_text:
        raise SystemExit('Installer template/license differs from the reviewed standard + save gate')
    print('Standard NSIS template verified; only the old-uninstall save gate differs')
else:
    target.write_text(patched, encoding='utf-8', newline='\n')
    license_target.write_text(license_text, encoding='utf-8', newline='\n')
    print('Generated standard NSIS template and retained upstream MIT notice')
