"""Disposable local fixtures. Refuses ordinary application data directories."""
import argparse
import json
import sqlite3
import uuid
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("data_dir", type=Path)
parser.add_argument("--notes", type=int, default=0, choices=[0, 1, 5, 10])
args = parser.parse_args()
directory = args.data_dir.resolve()
if not directory.name.startswith("com.qingjian.qa."):
    raise SystemExit("Only an explicitly isolated com.qingjian.qa.* directory is supported")
marker = directory / ".qingjian-qa-fixture"
if directory.exists() and any(directory.iterdir()) and not marker.exists():
    raise SystemExit("Existing directory has no disposable-fixture marker; refused")
directory.mkdir(parents=True, exist_ok=True)
marker.write_text("Disposable Qingjian upgrade test data", encoding="utf-8")
with sqlite3.connect(directory / "qingjian.db") as db:
    db.execute("PRAGMA foreign_keys=ON")
    db.execute("CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL)")
    settings = {"checkUpdatesOnStartup": False, "startOnBoot": False, "restoreNoteWindowsOnBoot": True,
                "hotkey": "Ctrl+Alt+Shift+F11", "themeId": "default-light"}
    db.execute("INSERT OR REPLACE INTO settings VALUES(1,?)", (json.dumps(settings),))
    tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    if "notes" not in tables:
        if args.notes:
            raise SystemExit("Initialize the isolated application once before adding notes")
    else:
        db.execute("DELETE FROM notes")
        db.execute("UPDATE workspace_state SET json=? WHERE id=1", ('{"openNoteIds":[],"activeNoteId":null}',))
        ids = []
        for index in range(args.notes):
            note_id = str(uuid.uuid5(uuid.NAMESPACE_OID, f"qingjian-qa-{index}"))
            ids.append(note_id)
            title = f"测试便签 {index + 1}"
            tasks = [{"type": "taskItem", "attrs": {"checked": n % 3 == 0}, "content": [
                {"type": "paragraph", "content": [{"type": "text", "text": f"待办 {n + 1}：检查桌面记录与完成状态"}]}]}
                for n in range(8)]
            doc = {"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": title}]},
                {"type": "taskList", "content": tasks},
                {"type": "paragraph", "content": [{"type": "text", "text": "用于全文搜索的正文。" * 30 + "独有后段关键字"}]}]}
            plain = title + "\n" + "\n".join(task["content"][0]["content"][0]["text"] for task in tasks) + "\n" + "用于全文搜索的正文。" * 30 + "独有后段关键字"
            db.execute("INSERT INTO notes(id,title,content_json,plain_text,created_at,updated_at,sort_order) VALUES(?,?,?,?,?,?,?)",
                       (note_id, title, json.dumps(doc, ensure_ascii=False), plain, "2026-10-27T00:00:00Z", "2026-10-27T00:00:00Z", index))
            db.execute("INSERT INTO note_window_state(note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_offset,updated_at) VALUES(?,'card',1,0,100,100,0,0,'2026-10-27T00:00:00Z')", (note_id,))
        db.execute("UPDATE workspace_state SET json=? WHERE id=1", (json.dumps({"openNoteIds": ids, "activeNoteId": ids[0] if ids else None}),))
print(json.dumps({"dataDir": str(directory), "notes": args.notes, "noteIds": locals().get("ids", [])}))
