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
--       - a table shaped `{ config = <table>, ... }` with optional hook
--         functions (e.g. `onConfigLoaded`) alongside `config`.
--   * Hook functions discovered at the top level are reported back to Lithos
--     and (for `onConfigLoaded`) invoked here. Function-valued keys are
--     stripped from the config table before JSON encoding so users can mix
--     data and hook callbacks in a single return value.
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

if type(mod) ~= "table" then
	stdio.ewrite("[lithos] user config must return a table or `{ config = <table>, ... }`, got " .. type(mod) .. "\n")
	process.exit(66)
end

-- Collect hook functions defined at the top level of the returned table.
-- Hooks must use the `on<Name>` naming convention so we can distinguish
-- them from user helpers that happen to be defined at the top level.
local hooks = {}
for k, v in pairs(mod) do
	if type(v) == "function" and type(k) == "string" and k:sub(1, 2) == "on" then
		table.insert(hooks, k)
	end
end

-- Resolve the config table.
local config
if type(mod.config) == "table" then
	config = mod.config
else
	-- Treat the returned table itself as the config, stripping out any
	-- function values so JSON encoding does not fail on hook callbacks.
	config = {}
	for k, v in pairs(mod) do
		if type(v) ~= "function" then
			config[k] = v
		end
	end
end

-- Invoke the synchronous `onConfigLoaded` hook if present. It receives the
-- decoded config table and may return a replacement table.
if type(mod.onConfigLoaded) == "function" then
	local okHook, hookResult = pcall(mod.onConfigLoaded, config)
	if not okHook then
		stdio.ewrite("[lithos] onConfigLoaded hook failed: " .. tostring(hookResult) .. "\n")
		process.exit(68)
	end
	if type(hookResult) == "table" then
		config = hookResult
	elseif hookResult ~= nil then
		stdio.ewrite("[lithos] onConfigLoaded must return a table or nil; got " .. type(hookResult) .. "\n")
		process.exit(69)
	end
end

local envelope = {
	config = config,
}
-- Lune's JSON encoder cannot tell an empty Lua table from an empty array,
-- so only attach `hooks` when at least one hook was registered. Rust treats
-- the missing field as an empty list.
if #hooks > 0 then
	envelope.hooks = hooks
end

local okEncode, encoded = pcall(serde.encode, "json", envelope)
if not okEncode then
	stdio.ewrite("[lithos] failed to encode user config as JSON: " .. tostring(encoded) .. "\n")
	process.exit(67)
end

stdio.write("\n@@LITHOS_CONFIG_BEGIN@@\n")
stdio.write(encoded)
stdio.write("\n@@LITHOS_CONFIG_END@@\n")
