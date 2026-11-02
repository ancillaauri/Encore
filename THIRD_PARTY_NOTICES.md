# Third-party software in Encore

Encore's own code is under the MIT licence (see LICENSE). The installed app also contains:

## FFmpeg
- Used as a separate program to capture, encode and assemble video (not linked into Encore).
- Licence: GNU Lesser General Public License v2.1 or later (LGPL build). The licence text is
  installed next to the program as `FFMPEG-LICENSE.txt`.
- Binary from BtbN/FFmpeg-Builds (https://github.com/BtbN/FFmpeg-Builds), win64 LGPL variant.
  FFmpeg source code: https://ffmpeg.org/download.html

## Tauri
- The app framework, with its updater, window-state and dialog plugins. Licence: MIT or
  Apache-2.0. https://github.com/tauri-apps/tauri

## Microsoft Edge WebView2
- Displays the app's interface (not the notifications, which Encore draws itself); part of
  Windows 11.

## Rust crates
- nnnoiseless (BSD-3-Clause), a Rust version of RNNoise: reduces background noise on the
  microphone when a clip is saved. https://github.com/jneem/nnnoiseless. Its notice:

```
Copyright (c) 2020, Joe Neeman
Copyright (c) 2017, Mozilla
Copyright (c) 2007-2017, Jean-Marc Valin
Copyright (c) 2005-2017, Xiph.Org Foundation
Copyright (c) 2003-2004, Mark Borgerding

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

- Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

- Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

- Neither the name of the Xiph.Org Foundation nor the names of its
contributors may be used to endorse or promote products derived from
this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION
OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

- serde, serde_json, windows-sys (MIT or Apache-2.0), wasapi (MIT), and their dependencies.
- resvg and usvg (MIT or Apache-2.0), tiny-skia (BSD-3-Clause), fontdb and rustybuzz (MIT), and
  their dependencies: they draw Encore's notifications. https://github.com/linebender/resvg
