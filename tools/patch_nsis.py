"""Post-process the generated NSIS script, then rebuild the setup exe.

Why: Tauri's default template uses a small default font and fixed-height text
controls. Under Chinese text (and HiDPI) the welcome text overlaps itself and
the directory page's last line is clipped. We also add an explicit Icon so the
setup executable itself gets the pet icon instead of the NSIS default.

Run after `cargo tauri build`.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]          # dsh-pet-tauri
SRC_TAURI = ROOT / "src-tauri"
NSIS_DIR = SRC_TAURI / "target" / "release" / "nsis" / "x64"
SCRIPT = NSIS_DIR / "installer.nsi"


def find_makensis() -> Path:
    candidates = [
        Path(os.environ.get("LOCALAPPDATA", "")) / "tauri" / "NSIS" / "makensis.exe",
        Path(r"C:\Program Files (x86)\NSIS\makensis.exe"),
        Path(r"C:\Program Files\NSIS\makensis.exe"),
    ]
    for p in candidates:
        if p.is_file():
            return p
    raise SystemExit("makensis.exe not found (run `cargo tauri build` once first)")


def main() -> None:
    text = SCRIPT.read_text(encoding="utf-8")

    # 1. Chinese-friendly font: Microsoft YaHei has proper CJK line metrics and
    #    avoids the overlapping-glyph look of the NSIS default bitmap font.
    if "MUI_FONT" not in text:
        text = text.replace(
            "!include MUI2.nsh",
            '!define MUI_FONT "Microsoft YaHei UI"\n'
            "!define MUI_FONTSIZE 10\n"
            "!include MUI2.nsh",
            1,
        )

    # 2. Explicit installer/uninstaller file icons (MUI_ICON alone does not
    #    always set the executable's icon resource on all NSIS builds).
    if 'Icon "${INSTALLERICON}"' not in text:
        text = text.replace(
            'OutFile "${OUTFILE}"',
            'OutFile "${OUTFILE}"\n'
            'Icon "${INSTALLERICON}"\n'
            'UninstallIcon "${UNINSTALLERICON}"',
            1,
        )

    # 3. Shorter welcome/directory strings so they fit in one glance and do not
    #    depend on the fixed control height.
    extra = (
        "\n; --- dsh-pet custom short messages (avoid clipped CJK text) ---\n"
        'LangString MUI_TEXT_WELCOME_INFO_TEXT ${LANG_SIMPCHINESE} '
        '"欢迎使用 dsh-pet 安装程序。$\\r$\\n$\\r$\\n'
        '建议先关闭其他应用程序，然后点击 [下一步(N)] 继续。"\n'
        'LangString ^DirText ${LANG_SIMPCHINESE} '
        '"选择 dsh-pet 的安装目录，然后点击 [下一步(N)] 继续。"\n'
    )
    if "dsh-pet custom short messages" not in text:
        text += extra

    # 4. Uninstaller language: MUI_UNGETLANGUAGE otherwise leaves the
    #    uninstaller on English (1033) on some machines.
    if "StrCpy $LANGUAGE 2052 ; dsh-pet uninstaller" not in text:
        text = text.replace(
            "!insertmacro MUI_UNGETLANGUAGE",
            "!insertmacro MUI_UNGETLANGUAGE\n"
            "  StrCpy $LANGUAGE 2052 ; dsh-pet uninstaller",
            1,
        )

    # 5. One-step update: same version reinstalls directly; a newer version
    #    silently uninstalls the old one and continues installing (no
    #    maintenance page asking the user to uninstall then re-run setup).
    if "dsh-pet auto update" not in text:
        old = "  Pop $R0\n  ; Reinstalling the same version"
        new = (
            "  Pop $R0\n"
            "  ; dsh-pet auto update: skip maintenance page\n"
            "  ${If} $R0 = 1\n"
            "    ReadRegStr $4 SHCTX \"${MANUPRODUCTKEY}\" \"\"\n"
            "    ReadRegStr $R1 SHCTX \"${UNINSTKEY}\" \"UninstallString\"\n"
            "    ${If} $R1 != \"\"\n"
            "      HideWindow\n"
            "      StrCpy $R1 \"$R1 /S _?=$4\"\n"
            "      ExecWait '$R1' $0\n"
            "      BringToFront\n"
            "    ${EndIf}\n"
            "    Abort\n"
            "  ${EndIf}\n"
            "  ${If} $R0 = 0\n"
            "    Abort\n"
            "  ${EndIf}\n"
            "  ; Reinstalling the same version"
        )
        if old not in text:
            raise SystemExit("version-compare anchor not found")
        text = text.replace(old, new, 1)

    SCRIPT.write_text(text, encoding="utf-8")

    m = re.search(r'!define OUTFILE "([^"]+)"', text)
    if not m:
        raise SystemExit("OUTFILE not found in installer.nsi")
    out_name = m.group(1)

    makensis = find_makensis()
    subprocess.run([str(makensis), str(SCRIPT)], cwd=NSIS_DIR, check=True)

    built = NSIS_DIR / out_name
    if not built.is_file():
        raise SystemExit(f"expected installer not found: {built}")
    dest_dir = SRC_TAURI / "target" / "release" / "bundle" / "nsis"
    dest_dir.mkdir(parents=True, exist_ok=True)
    conf = json.loads((SRC_TAURI / "tauri.conf.json").read_text(encoding="utf-8"))
    dest_name = f"{conf['productName']}_{conf['version']}_x64-setup.exe"
    dest = dest_dir / dest_name
    shutil.copy2(built, dest)
    print(f"patched installer: {dest} ({dest.stat().st_size / 1024 / 1024:.1f} MB)")


if __name__ == "__main__":
    main()
