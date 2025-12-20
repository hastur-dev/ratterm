-- Utility functions for preload-test extension
-- This file is preloaded before init.lua

Utils = {}

function Utils.format_message(prefix, msg)
    return "[" .. prefix .. "] " .. msg
end

function Utils.count_items(tbl)
    local count = 0
    for _ in pairs(tbl) do
        count = count + 1
    end
    return count
end

function Utils.join(tbl, sep)
    local result = ""
    for i, v in ipairs(tbl) do
        if i > 1 then
            result = result .. sep
        end
        result = result .. tostring(v)
    end
    return result
end

-- Mark as loaded
UTILS_LOADED = true
