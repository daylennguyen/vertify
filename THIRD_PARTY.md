# Third-party software

Vertify is MIT-licensed. Official release archives **bundle FFmpeg** (`ffmpeg` / `ffprobe`) as a separate program that Vertify launches. FFmpeg is not linked into the Vertify binary.

Those bundled builds typically include x264 and are therefore **GPL**. Their license files ship in `third_party/ffmpeg/` inside the archive or install folder.

- FFmpeg: https://ffmpeg.org
- Windows builds: https://www.gyan.dev/ffmpeg/builds/
- Linux builds: https://github.com/BtbN/FFmpeg-Builds

GUI fonts (Syne, Source Sans 3) are SIL OFL 1.1 — see `assets/fonts/`.
