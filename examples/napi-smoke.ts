// This file intentionally uses JavaScript-compatible TypeScript so it can be
// run directly with Node in the N-API CI job.
const assert = require("node:assert/strict");
const { lint } = require("../npm");

const result = lint({
  documents: ["query { hero }"],
  rules: { "no-anonymous-operations": ["error", {}] }
});

assert.equal(result.length, 1);
assert.equal(result[0].ruleId, "no-anonymous-operations");
assert.equal(result[0].line, 1);
assert.equal(result[0].column, 0);
assert.equal(result[0].filePath, "<document-0>");
