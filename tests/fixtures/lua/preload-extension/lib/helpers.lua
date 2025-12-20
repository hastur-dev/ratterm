-- Helper functions for preload-test extension
-- This file is preloaded after utils.lua

Helpers = {}

function Helpers.notify_formatted(prefix, msg)
    -- Uses Utils from preloaded file
    local formatted = Utils.format_message(prefix, msg)
    ratterm.notify(formatted)
end

function Helpers.list_commands()
    local cmds = ratterm.commands.list()
    return Utils.count_items(cmds)
end

-- Mark as loaded
HELPERS_LOADED = true
