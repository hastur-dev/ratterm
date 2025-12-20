-- Complex test extension
-- Tests: events, timers, fs operations, terminal operations

event_log = {}
timer_fires = 0
timer_id = nil

function on_load()
    ratterm.notify("complex-test loaded")

    -- Subscribe to multiple events
    ratterm.events.on("file_open", function(path)
        table.insert(event_log, "file_open:" .. path)
    end)

    ratterm.events.on("file_save", function(path)
        table.insert(event_log, "file_save:" .. path)
    end)

    ratterm.events.on("file_close", function(path)
        table.insert(event_log, "file_close:" .. path)
    end)

    -- Register commands for testing
    ratterm.commands.register("complex.event_log", function()
        for i, event in ipairs(event_log) do
            ratterm.notify(tostring(i) .. ": " .. event)
        end
    end, {
        name = "Show Event Log",
        description = "Shows all captured events"
    })

    ratterm.commands.register("complex.fs_test", function(args)
        local path = args[1] or "/tmp/test_file.txt"
        local content = args[2] or "Test content from Lua"

        -- Write file
        local success = ratterm.fs.write(path, content)
        if success then
            ratterm.notify("Wrote file: " .. path)
        else
            ratterm.notify("Failed to write: " .. path)
            return
        end

        -- Check exists
        if ratterm.fs.exists(path) then
            ratterm.notify("File exists")
        end

        -- Read back
        local read_content = ratterm.fs.read(path)
        if read_content then
            ratterm.notify("Read content: " .. read_content)
        end

        -- Clean up
        ratterm.fs.remove(path)
    end, {
        name = "FS Test",
        description = "Tests file system operations"
    })

    ratterm.commands.register("complex.terminal_test", function()
        -- Send some keys to terminal
        ratterm.terminal.send_keys("echo 'Hello from Lua'\n")

        -- Get terminal size
        local cols, rows = ratterm.terminal.get_size()
        ratterm.notify("Terminal size: " .. tostring(cols) .. "x" .. tostring(rows))
    end, {
        name = "Terminal Test",
        description = "Tests terminal operations"
    })

    ratterm.commands.register("complex.start_timer", function()
        timer_fires = 0
        timer_id = ratterm.timer.every(100, function()
            timer_fires = timer_fires + 1
            ratterm.notify("Timer fired: " .. tostring(timer_fires))
        end)
        ratterm.notify("Started timer: " .. tostring(timer_id))
    end, {
        name = "Start Timer",
        description = "Starts a repeating timer"
    })

    ratterm.commands.register("complex.stop_timer", function()
        if timer_id then
            ratterm.timer.cancel(timer_id)
            ratterm.notify("Stopped timer, total fires: " .. tostring(timer_fires))
            timer_id = nil
        else
            ratterm.notify("No timer running")
        end
    end, {
        name = "Stop Timer",
        description = "Stops the repeating timer"
    })
end

function on_unload()
    if timer_id then
        ratterm.timer.cancel(timer_id)
    end
    ratterm.notify("complex-test unloaded")
end

-- Get event log for testing
function get_event_log()
    return event_log
end

-- Get timer fire count
function get_timer_fires()
    return timer_fires
end
