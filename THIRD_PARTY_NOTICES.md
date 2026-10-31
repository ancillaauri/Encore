# Third-party software in Encore

Encore's own code is under the MIT licence (see LICENSE). The installed app also contains:

## FFmpeg
- Used as a separate program to capture, encode and assemble video (not linked into Encore).
- Licence: GNU Lesser General Public License v2.1 or later (LGPL build). The licence text is
  installed next to the program as `FFMPEG-LICENSE.txt`.
- Binary from BtbN/FFmpeg-Builds (https://github.com/BtbN/FFmpeg-Builds), win64 LGPL variant.
  FFmpeg source code: https://ffmpeg.org/download.html

## Tauri
- The app framework. Licence: MIT or Apache-2.0. https://github.com/tauri-apps/tauri

## Microsoft Edge WebView2
- Displays the app's interface; part of Windows 11.

## Rust crates
- serde, serde_json (MIT or Apache-2.0), windows-sys (MIT or Apache-2.0), and their dependencies.
