# Windows installer

Official releases build `VertifySetup-<version>-windows-x64.exe` with [Inno Setup](https://jrsoftware.org/isinfo.php).

The installer copies `vertify.exe`, `vertify-gui.exe`, `ffmpeg.exe`, and `ffprobe.exe` into `%LOCALAPPDATA%\Programs\Vertify`. Users do **not** need a separate ffmpeg install.

Local compile (after placing binaries in `payload/`):

```text
choco install innosetup
scripts\vendor-ffmpeg.ps1 -DestDir installer\windows\payload
copy target\release\vertify.exe, vertify-gui.exe installer\windows\payload\
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\windows\vertify.iss
```

`payload/` is gitignored — it is filled in CI.
