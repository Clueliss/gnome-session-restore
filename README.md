# Gnome Session Restore

A simple utility to save and restore gnome sessions.

## Requirements

Since gnome 40 and upwards made the org.gnome.Shell.Eval only
accessible in unsafe-mode this utilty needs a extension that restores
the functionality that it relied on before this change; without enabling unsafe-mode.

https://github.com/Clueliss/windowctl

I initially used a workaround that temporarily enabled unsafe mode via another
extension, but that was obviously bad and I really only did it as a quick and
dirty hack to get it working again.

## Examples

```shell
gnome-session-restore --file test.json save

-- later -- 

gnome-session-restore --file test.json restore
```
