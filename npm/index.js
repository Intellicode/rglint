const { platform, arch } = process;
const path = require("node:path");

const packages = {
  "darwin-arm64": "@rglint/napi-darwin-arm64",
  "linux-x64-gnu": "@rglint/napi-linux-x64-gnu",
  "win32-x64-msvc": "@rglint/napi-win32-x64-msvc"
};

const artifacts = {
  "darwin-arm64": "rglint-napi.darwin-arm64.node",
  "linux-x64-gnu": "rglint-napi.linux-x64-gnu.node",
  "win32-x64-msvc": "rglint-napi.win32-x64-msvc.node"
};

const key = `${platform}-${arch}${platform === "linux" ? "-gnu" : platform === "win32" ? "-msvc" : ""}`;
const packageName = packages[key];
if (!packageName || !artifacts[key]) {
  throw new Error(`@rglint/napi does not support ${platform}-${arch}`);
}

try {
  module.exports = require(packageName);
} catch (error) {
  // The release workflow runs the smoke test before publishing the loader, so
  // support the checked-out platform artifact as a deterministic fallback.
  try {
    module.exports = require(path.join(__dirname, "platform", key, artifacts[key]));
  } catch {
    throw new Error(
      `Could not load ${packageName}; install the optional platform package for ${key}`,
      { cause: error }
    );
  }
}
