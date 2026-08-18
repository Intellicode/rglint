# @rglint/napi

Synchronous Node.js bindings for rglint. The package ships a thin portable
loader and selects the native N-API artifact for Linux x64 (glibc), macOS
arm64, or Windows x64.

```js
const { lint } = require("@rglint/napi");

const diagnostics = lint({
  documents: ["query { hero }"],
  rules: { "no-anonymous-operations": ["error", {}] }
});
```

Source strings are used instead of paths, so callers can lint editor buffers
without creating temporary files. `column` is zero-based, matching rglint's
JSON reporter contract.
