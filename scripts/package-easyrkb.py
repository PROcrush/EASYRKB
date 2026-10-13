"""Package and inspect native EASYRKB builds without user databases or media."""
from pathlib import Path
import hashlib
import json
import os
import plistlib
import shutil
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
OUT = ROOT / "out"
OUT.mkdir(exist_ok=True)
TARGET = Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))) / "release"
FORBIDDEN = {".db", ".sqlite", ".sqlite3", ".mp3", ".wav", ".flac", ".aiff", ".m4a"}


def run(*args):
    subprocess.run([str(arg) for arg in args], check=True)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inspect_payload(path):
    files = [file for file in path.rglob("*") if file.is_file()]
    assert not any(file.suffix.lower() in FORBIDDEN for file in files)
    for name in ["LICENSE", "LICENSING.md", "GPL-2.0.txt", "phrase-pinyin-LICENSE"]:
        assert any(file.name == name for file in files), name


def pe(path, application=True):
    data = path.read_bytes()
    assert data[:2] == b"MZ"
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    assert data[offset:offset + 4] == b"PE\0\0"
    machine = struct.unpack_from("<H", data, offset + 4)[0]
    assert machine in ({0x8664} if application else {0x14C, 0x8664})
    versions = []
    cursor = 0
    while True:
        position = data.find(b"\xbd\x04\xef\xfe", cursor)
        if position < 0:
            break
        high, low = struct.unpack_from("<II", data, position + 8)
        versions.append(f"{high >> 16}.{high & 65535}.{low >> 16}.{low & 65535}")
        cursor = position + 4
    assert VERSION + ".0" in versions, versions
    return {"machine": "x64" if machine == 0x8664 else "x86", "versions": versions}


assert not list((ROOT / "dist").rglob("*backend-mock*")), "Demo backend in production build"
report = {"product": "EASYRKB", "version": VERSION, "source_commit": os.environ.get("GITHUB_SHA"),
          "production_frontend_has_no_demo_backend": True, "no_database_or_media_distributed": True,
          "licenses_included": True, "usb_player_hardware": "not tested for this release"}

if sys.platform == "darwin":
    app = TARGET / "bundle/macos/EASYRKB.app"
    with (app / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    assert info["CFBundleShortVersionString"] == VERSION
    assert info["CFBundleIdentifier"] == "com.procrush.rbxport.zh"
    binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
    arch = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip()
    assert arch == "arm64", arch
    run("codesign", "--force", "--deep", "--sign", "-", "--entitlements", ROOT / "src-tauri/entitlements.plist", app)
    run("codesign", "--verify", "--deep", "--strict", app)
    inspect_payload(app)
    stage = Path(os.environ.get("RUNNER_TEMP", "/tmp")) / f"easyrkb-{VERSION}-dmg"
    stage.mkdir()
    run("ditto", app, stage / "EASYRKB.app")
    (stage / "Applications").symlink_to("/Applications")
    (stage / "安装说明.txt").write_text("EASYRKB " + VERSION + "\n将应用拖入 Applications 后打开，默认读取本机 rekordbox 曲库。\n本包为 Apple Silicon，临时签名，未经 Apple 公证。\n", encoding="utf-8")
    installer = OUT / f"EASYRKB-{VERSION}-macos-arm64.dmg"
    run("hdiutil", "create", "-volname", "EASYRKB " + VERSION, "-srcfolder", stage, "-fs", "APFS", "-format", "UDZO", installer)
    run("hdiutil", "verify", installer)
    mount = stage.parent / f"easyrkb-{VERSION}-mount"
    mount.mkdir()
    run("hdiutil", "attach", "-readonly", "-nobrowse", "-noautoopen", "-mountpoint", mount, installer)
    try:
        run("codesign", "--verify", "--deep", "--strict", mount / "EASYRKB.app")
        assert digest(mount / "EASYRKB.app/Contents/MacOS/rbxport") == digest(binary)
        inspect_payload(mount / "EASYRKB.app")
    finally:
        run("hdiutil", "detach", mount)
    report.update({"platform": "macos-arm64", "binary_sha256": digest(binary), "dmg_integrity": "passed",
                   "mounted_app_strict_signature": "passed", "signing": "ad-hoc, not notarized"})
else:
    source = TARGET / f"bundle/nsis/EASYRKB_{VERSION}_x64-setup.exe"
    installer = OUT / f"EASYRKB-{VERSION}-windows-x64-setup.exe"
    shutil.copy2(source, installer)
    sevenzip = Path("C:/Program Files/7-Zip/7z.exe")
    run(sevenzip, "t", installer)
    extracted = Path(os.environ["RUNNER_TEMP"]) / f"easyrkb-{VERSION}-extracted"
    run(sevenzip, "x", "-y", f"-o{extracted}", installer)
    inspect_payload(extracted)
    binary = extracted / "rbxport.exe"
    original = (TARGET / "rbxport.exe").read_bytes()
    assert binary.read_bytes() in (original, original.replace(b"__TAURI_BUNDLE_TYPE_VAR_UNK", b"__TAURI_BUNDLE_TYPE_VAR_NSS"))
    report.update({"platform": "windows-x64", "binary_sha256": digest(binary), "binary_identity": pe(binary),
                   "installer_identity": pe(installer, application=False), "nsis_archive_integrity": "passed",
                   "installer_language": "SimpChinese", "signing": "unsigned",
                   "real_library_and_audio_playback": "not tested on Windows"})

report.update({"installer": installer.name, "installer_bytes": installer.stat().st_size, "installer_sha256": digest(installer)})
(OUT / (report["platform"] + "-validation.json")).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(report, ensure_ascii=False, indent=2))
