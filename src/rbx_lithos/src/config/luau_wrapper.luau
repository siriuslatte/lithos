--!strict
-- Lithos Luau config wrapper.
--
-- Lune invokes this script as `lune run wrapper.luau <require-path>` where
-- <require-path> is a Lune-compatible relative path (no extension) to the
-- user's `lithos.luau` / `lithos.lua` file.
--
-- Contract:
--   * The user script may return either:
--       - a config table directly, or
--       - a table shaped `{ config = <table>, ... }` (additional fields are
--         currently reserved for future hook support).
--   * Anything written to stdout outside the sentinel markers is ignored by
--     Lithos, but the user is still free to `print` for debugging.
--
-- Important: Lune's `require` resolves relative paths using `debug.getinfo`
-- on the calling stack frame. Wrapping it in `pcall` hides the source frame,
-- which breaks path resolution. Errors from `require` / the user script are
-- therefore left to propagate; Lune prints them to stderr and exits non-zero,
-- and the Rust caller surfaces both.

local serde = require("@lune/serde")
local stdio = require("@lune/stdio")
local process = require("@lune/process")

local userPath = process.args[1]
if type(userPath) ~= "string" or userPath == "" then
	stdio.ewrite("[lithos] internal error: missing user config path argument\n")
	process.exit(64)
end

local mod = require(userPath)

local config
if type(mod) == "table" and type(mod.config) == "table" then
	config = mod.config
elseif type(mod) == "table" then
	config = mod
else
	stdio.ewrite("[lithos] user config must return a table or `{ config = <table> }`, got " .. type(mod) .. "\n")
	process.exit(66)
end

local okEncode, encoded = pcall(serde.encode, "json", config)
if not okEncode then
	stdio.ewrite("[lithos] failed to encode user config as JSON: " .. tostring(encoded) .. "\n")
	process.exit(67)
end

stdio.write("\n@@LITHOS_CONFIG_BEGIN@@\n")
stdio.write(encoded)
stdio.write("\n@@LITHOS_CONFIG_END@@\n")
