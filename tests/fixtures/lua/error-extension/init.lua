-- Error test extension
-- Tests error handling and recovery

function on_load()
    ratterm.notify("error-test loaded")

    -- Command that throws an error
    ratterm.commands.register("error.throw", function()
        error("Intentional error for testing")
    end, {
        name = "Throw Error",
        description = "Throws an intentional error"
    })

    -- Command that handles errors gracefully
    ratterm.commands.register("error.safe", function()
        local status, err = pcall(function()
            error("Caught error")
        end)

        if not status then
            ratterm.notify("Caught error: " .. tostring(err))
        else
            ratterm.notify("No error occurred")
        end
    end, {
        name = "Safe Error",
        description = "Handles errors gracefully"
    })

    -- Command that tests invalid API usage
    ratterm.commands.register("error.invalid_api", function()
        -- Try to read a non-existent file
        local content = ratterm.fs.read("/nonexistent/path/file.txt")
        if content == nil then
            ratterm.notify("Correctly returned nil for non-existent file")
        else
            ratterm.notify("ERROR: Should have returned nil")
        end
    end, {
        name = "Invalid API Test",
        description = "Tests API with invalid inputs"
    })
end

function on_unload()
    ratterm.notify("error-test unloaded")
end
