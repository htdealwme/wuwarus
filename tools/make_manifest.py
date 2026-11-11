"""Build manifest.json for a translation release.

Usage: make_manifest.py <repo owner/name> <tag> <release.json> <pak files...>
"""

import datetime
import hashlib
import json
import pathlib
import re
import sys

NAME_RE = re.compile(r"^[A-Za-z0-9_-][A-Za-z0-9._-]{0,95}\.pak$")


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    repo, tag, meta_path, *paks = sys.argv[1:]
    if not paks:
        sys.exit("no pak files given")
    meta = json.loads(pathlib.Path(meta_path).read_text(encoding="utf-8"))

    files = []
    for p in map(pathlib.Path, paks):
        if not NAME_RE.match(p.name):
            sys.exit(f"bad pak name: {p.name}")
        files.append({
            "name": p.name,
            "url": f"https://github.com/{repo}/releases/download/{tag}/{p.name}",
            "size": p.stat().st_size,
            "sha256": sha256(p),
        })

    manifest = {
        "schema": 1,
        "version": tag.removeprefix("v"),
        "game_version": meta.get("game_version"),
        "published_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "notes": meta.get("notes"),
        "files": files,
    }
    json.dump(manifest, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
