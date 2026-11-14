-- Omarchy's Lua configuration (Hyprland 0.56).
-- Install rafiFlow at ~/.local/bin/rafiflow before enabling these.
-- Include this file from ~/.config/hypr/hyprland.lua.
-- Change both bindings together to choose another shortcut.
o.bind("SUPER + ALT + SPACE", "Start rafiFlow", "~/.local/bin/rafiflow --control start")
o.bind("SUPER + ALT + SPACE", "Finish rafiFlow", "~/.local/bin/rafiflow --control stop", { release = true })
o.window({ class = "rafiflow", title = "^rafiFlow recording$" }, {
  float = true,
  no_focus = true,
  border_size = 0,
  size = { 340, 90 },
  move = { "(monitor_w-window_w)/2", "(monitor_h-window_h-60)" },
})
