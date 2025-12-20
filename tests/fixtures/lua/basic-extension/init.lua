-- Basic test extension
-- Tests core functionality: lifecycle hooks, notify, commands

loaded = false
unloaded = false

function on_load()
    loaded = true
    ratterm.notify("basic-test loaded")

    -- Register a simple command
    ratterm.commands.register("basic.hello", function(args)
        local name = args[1] or "World"
        ratterm.notify("Hello, " .. name .. "!")
    end, {
        name = "Hello Command",
        description = "Says hello to someone"
    })

    -- Register a command that accesses editor
    ratterm.commands.register("basic.cursor", function()
        local line, col = ratterm.editor.get_cursor()
        ratterm.notify("Cursor at line " .. tostring(line) .. ", col " .. tostring(col))
    end, {
        name = "Show Cursor",
        description = "Shows current cursor position"
    })
end

function on_unload()
    unloaded = true
    ratterm.notify("basic-test unloaded")
end

-- Helper function to check state
function get_state()
    return {
        loaded = loaded,
        unloaded = unloaded
    }
end
