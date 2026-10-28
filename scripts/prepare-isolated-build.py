"""Snapshot a local build for QA with its own data and single-instance names.

The instrumentation lives only in the ignored snapshot, never the shipped UI.
Run quality:static first, then this script, then tauri build in the printed folder.
"""
import argparse
import json
import re
import shutil
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--source", type=Path, default=Path.cwd())
parser.add_argument("--output", required=True, type=Path)
parser.add_argument("--label", required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parent.parent
source = args.source.resolve()
output = args.output.resolve()
if not output.is_relative_to(root / ".reviews") or not re.fullmatch(r"[a-z0-9-]+", args.label):
    raise SystemExit("Output must be inside workspace .reviews and label must be a simple QA name")
if source == output:
    raise SystemExit("Source and output must differ")
output.mkdir(parents=True, exist_ok=True)
shutil.copytree(source / "dist", output / "dist", dirs_exist_ok=True)
for directory in ["src", "icons", "capabilities", "installer"]:
    if not (source / "src-tauri" / directory).exists():
        continue
    shutil.copytree(source / "src-tauri" / directory, output / "src-tauri" / directory, dirs_exist_ok=True)
for filename in ["Cargo.toml", "Cargo.lock", "build.rs", "tauri.conf.json"]:
    shutil.copy2(source / "src-tauri" / filename, output / "src-tauri" / filename)
shutil.copy2(source / "package.json", output / "package.json")
config_path = output / "src-tauri/tauri.conf.json"
config = json.loads(config_path.read_text(encoding="utf-8"))
config["identifier"] = "com.qingjian.qa." + args.label
config["build"]["beforeBuildCommand"] = ""
config["bundle"]["active"] = False
config_path.write_text(json.dumps(config, ensure_ascii=False, indent=2), encoding="utf-8")
main_path = output / "src-tauri/src/main.rs"
main = main_path.read_text(encoding="utf-8")
for name in ["QingjianDesktopPrimaryInstanceV1", "QingjianDesktopShowPrimaryV1",
             "QingjianQABaselinePrimaryV1", "QingjianQABaselineShowV1"]:
    main = main.replace(name, name + "." + args.label)
main_path.write_text(main, encoding="utf-8")

# Native phase probes are patched only into this ignored QA snapshot.
lib_path = output / 'src-tauri/src/lib.rs'
lib = lib_path.read_text(encoding='utf-8')
native_probe = r'''
fn qingjian_qa_phase(name: &str) {
    use std::io::Write;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
    if let Some(root) = std::env::var_os("LOCALAPPDATA") {
        let directory = std::path::PathBuf::from(root).join("__QA_IDENTIFIER__");
        let _ = std::fs::create_dir_all(&directory);
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(directory.join("qa-native-phases.jsonl")) {
            let _ = writeln!(file, "{{\"pid\":{},\"phase\":\"{}\",\"epochMs\":{}}}", std::process::id(), name, stamp);
        }
    }
}
'''.replace('__QA_IDENTIFIER__', config['identifier'])
lib += '\n' + native_probe
for before, after in [
    ('let data_dir = app.path().app_local_data_dir()?;', 'qingjian_qa_phase("setup"); let data_dir = app.path().app_local_data_dir()?;'),
    ('let state = AppState::initialize(data_dir.clone())?;', 'let state = AppState::initialize(data_dir.clone())?; qingjian_qa_phase("database-ready");'),
    ('windows::restore_visible_note_windows(app.handle(), on_boot);', 'windows::restore_visible_note_windows(app.handle(), on_boot); qingjian_qa_phase("windows-created");'),
]:
    if before not in lib:
        raise SystemExit('Native profiling insertion point missing: ' + before)
    lib = lib.replace(before, after, 1)
lib_path.write_text(lib, encoding='utf-8')
timings = output.parent / (args.label + "-timings")
timings.mkdir(exist_ok=True)
instrument = r"""
<script>
(() => {
  const destination = __DESTINATION__;
  const noteId = new URLSearchParams(location.search).get('noteId');
  const metrics = { runId: crypto.randomUUID(), noteId, timeOrigin: performance.timeOrigin, uiReadyMs: null, inputToPaintMs: [], wakeToPaintMs: [] };
  let writeTimer;
  const write = () => { clearTimeout(writeTimer); writeTimer = setTimeout(() => {
    metrics.phases = performance.getEntriesByType('mark').filter(mark => mark.name.startsWith('qingjian:')).map(mark => ({ name: mark.name, ms: mark.startTime }));
    metrics.resources = performance.getEntriesByType('resource').filter(resource => /\.js(?:\?|$)/.test(resource.name)).map(resource => ({ name: resource.name.split('/').pop(), startMs: resource.startTime, durationMs: resource.duration }));
    window.__TAURI_INTERNALS__?.invoke('export_markdown', {
    destination: destination + '/' + (noteId || 'hub') + '.json', markdown: JSON.stringify(metrics)
    }).catch(() => {});
  }, 1500); };
  let searchStart = 0;
  const observer = new MutationObserver(() => {
    if (metrics.uiReadyMs === null && document.querySelector(noteId ? '[contenteditable="true"]' : '.workbench-note-list[aria-busy="false"]')) {
      metrics.uiReadyMs = performance.now(); write();
      requestAnimationFrame(() => requestAnimationFrame(() => { metrics.twoFramesMs = performance.now(); write(); }));
    }
    if (searchStart && document.querySelector('.workbench-note-list[aria-busy="false"]')) {
      (metrics.searchMs ||= []).push(performance.now() - searchStart); searchStart = 0; write();
    }
  });
  observer.observe(document.documentElement, { subtree: true, childList: true, attributes: true, attributeFilter: ['aria-busy'] });
  document.addEventListener('input', event => {
    if (event.target.closest?.('.workbench-search')) searchStart = performance.now();
    if (!event.target.closest?.('.note-prosemirror')) return;
    const start = performance.now();
    requestAnimationFrame(() => requestAnimationFrame(() => { metrics.inputToPaintMs.push(performance.now() - start); write(); }));
  }, true);
  if (noteId && window.__TAURI_INTERNALS__) {
    const internals = window.__TAURI_INTERNALS__;
    const handler = internals.transformCallback(event => {
      const state = event.payload;
      if (!state?.visible || state.noteId !== noteId) return;
      const start = Date.parse(state.updatedAt);
      requestAnimationFrame(() => requestAnimationFrame(() => {
        metrics.wakeToPaintMs.push(performance.timeOrigin + performance.now() - start); write();
      }));
    });
    internals.invoke('plugin:event|listen', { event: 'qingjian://window-state-changed', target: { kind: 'Any' }, handler }).catch(() => {});
  }
})();
</script>
""".replace("__DESTINATION__", json.dumps(str(timings).replace("\\", "/")))
index = output / "dist/index.html"
html = index.read_text(encoding="utf-8")
html = re.sub(r'<script>\s*\(\(\) => \{\s*const destination = .*?</script>', '', html, count=1, flags=re.S)
index.write_text(html.replace("<head>", "<head>" + instrument, 1), encoding="utf-8")
print(json.dumps({"snapshot": str(output), "identifier": config["identifier"], "timings": str(timings)}))
