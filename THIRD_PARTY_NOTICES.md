# Third-Party Notices

`rprof` itself is licensed under MIT (see [`LICENSE`](LICENSE)). This file
documents the licenses of bundled third-party assets that ship inside the
release binary or alongside it.

## uPlot

- **Files:** `assets/uPlot.iife.min.js`, `assets/uPlot.min.css`
- **Version:** 1.6.31
- **Upstream:** https://github.com/leeoniya/uPlot
- **License:** MIT

The minified bundles fetched from upstream contain only a URL comment
(`/*! https://github.com/leeoniya/uPlot (v1.6.31) */`) and no embedded
license text, so the full license is reproduced below per the MIT
"copyright notice and this permission notice shall be included" condition.

```
The MIT License (MIT)

Copyright (c) 2022 Leon Sorokin

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

Both files are also embedded into every HTML report emitted by `rprof view`
via `include_str!`, so this notice is reproduced in the rendered HTML by
keeping the upstream banner intact.

## Rust crate dependencies

The runtime and build-time Rust crate dependencies are listed in
[`Cargo.toml`](Cargo.toml); their licenses follow each crate's own
distribution. Inspect with `cargo tree` and `cargo license` (the latter
is not installed by default).
