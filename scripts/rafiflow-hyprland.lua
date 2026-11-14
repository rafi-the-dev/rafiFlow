-- Omarchy / Hyprland 0.56. Include from ~/.config/hypr/hyprland.lua.
-- Install the binary at this path, or change it to your release executable.
local rafiflow = "~/.local/bin/rafiflow"
local held = false

-- Only remove the chosen chord after checking it is not used by another app.
hl.unbind("CTRL + ALT + SPACE")
o.bind("CTRL + ALT + SPACE", "Start rafiFlow", function()
  if not held then
    held = true
    hl.dispatch(hl.dsp.exec_cmd(rafiflow .. " --control start"))
  end
end)
-- The release must ignore modifier changes and keybind shadowing.
o.bind("SPACE", "Finish rafiFlow", function()
  if held then
    held = false
    hl.dispatch(hl.dsp.exec_cmd(rafiflow .. " --control stop"))
  end
end, { release = true, ignore_mods = true, non_consuming = true, transparent = true })

-- Keep the overlay rules when Hyprland reloads its configuration.
o.window({ class = "^rafiflow$", title = "^rafiFlow recording$" }, {
  name = "rafiflow-overlay",
  enabled = true,
  float = true,
  no_focus = true,
  border_size = 0,
  decorate = false,
  no_shadow = true,
  no_blur = true,
  rounding = 0,
  min_size = { 340, 90 },
  max_size = { 340, 90 },
  size = { 340, 90 },
  move = { "(monitor_w-window_w)/2", "(monitor_h-window_h-60)" },
})
