-- Luau equivalent of `examples/projects/getting-started/lithos.yml`,
-- demonstrating two reasons to reach for a Luau config:
--
--   1. Reduce repetition with helpers and loops.
--   2. React to Lithos lifecycle events with hooks.
--
-- Run with: `lithos deploy --environment dev examples/projects/luau-config`.
-- Requires the `lune` binary on PATH (see the Configuration docs page).

local function environment(label: string, opts: { public: boolean? }?): { [string]: any }
	opts = opts or {}
	return {
		label = label,
		targetNamePrefix = if opts.public then nil else "environmentLabel",
		targetAccess = if opts.public then "public" else nil,
	}
end

local config = {
	environments = {
		environment("dev"),
		environment("staging"),
		environment("prod", { public = true }),
	},

	target = {
		experience = {
			configuration = {
				genre = "building",
				playableDevices = { "computer", "phone", "tablet" },
			},
			places = {
				start = {
					file = "game.rbxlx",
					configuration = {
						name = "Getting Started with Lithos (Luau)",
						description = "Made with Lithos and Luau.",
						maxPlayerCount = 20,
					},
				},
			},
		},
	},
}

return {
	config = config,

	-- Runs synchronously right after Lithos finishes decoding the config.
	-- Return a new table to replace it, or `nil` to leave it untouched.
	onConfigLoaded = function(config)
		if os.getenv("CI") then
			-- In CI, automatically allow the `release/*` branch family to
			-- deploy to production as well.
			for _, env in ipairs(config.environments) do
				if env.label == "prod" then
					env.branches = { "main", "release/*" }
				end
			end
		end
		return config
	end,

	-- Hooks below are recorded for future deploy lifecycle integration.
	onBeforeDeploy = function() end,
	onAfterDeploy = function() end,
}
