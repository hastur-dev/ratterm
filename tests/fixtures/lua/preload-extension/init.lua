-- Preload test extension
-- Tests that preload files are loaded before main file

function on_load()
    -- Verify preload files were loaded
    if not UTILS_LOADED then
        ratterm.notify("ERROR: utils.lua not preloaded")
        return
    end

    if not HELPERS_LOADED then
        ratterm.notify("ERROR: helpers.lua not preloaded")
        return
    end

    -- Use functions from preloaded files
    Helpers.notify_formatted("preload-test", "All preload files loaded successfully")

    ratterm.commands.register("preload.test", function()
        local items = {"apple", "banana", "cherry"}
        local joined = Utils.join(items, ", ")
        Helpers.notify_formatted("preload-test", "Joined: " .. joined)
    end, {
        name = "Preload Test",
        description = "Tests preloaded functions"
    })
end

function on_unload()
    Helpers.notify_formatted("preload-test", "Unloading")
end

-- Check if preload worked
function check_preload()
    return UTILS_LOADED == true and HELPERS_LOADED == true
end
