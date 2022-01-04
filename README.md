# Gnome Session Restore

A simple utility to save and restore gnome sessions.

## Examples

```shell
gnome-session-restore --file test.json save

-- later -- 

gnome-session-restore --file test.json restore
```


## Requirements

Since gnome 40 and upwards made the org.gnome.Shell.Eval only
accessible in unsafe-mode this utilty needs a extension that restores
the functionality that it relied on before this change (without enabling unsafe-mode).

https://github.com/Clueliss/windowctl

I initially used a workaround that temporarily enabled unsafe mode via another
extension, but that was obviously bad and I really only did it as a quick and
dirty hack to get it working again.

## How it works

### Saving

1. Query the above mentioned extensions' dbus interface for
window metadata. This metadata will contain stuff like, window geometry, process id, window manager
class and possibly a gtk app id.

2. Try to find the command that belongs to a specific window. This can be done in one of three ways, which will be
tried one by one.
   1. If that application has a `gtk app id`, this normally means that a desktop file with exactly that
    name exists. If it does the desktop file will be used to execute it.
   2. If that application has a `sanboxed app id` this normally means that a desktop file with exactly that
      name exists. If it does the desktop file will be used to execute it.
   3. Consider the window manager class, and the name of the executable that is found via `/proc/{pid}/cmdline`.
    If a desktop file with any of those names exists it will be used.
   4. If no desktop file could be found the only option left is
    taking the command found in `/proc/{pid}/cmdline`

3. Save all the extracted metadata in a json file.

The reason for considering `/proc/{pid}/cmdline` only as a last resort
is that in my testing, just executing this is often a poor representation of what
launching a program via it's desktop entry is like. Which also makes it
a poor way to restore sessions.

### Restoring

1. Read the given json file
2. Execute all the given commands
3. Try to move the windows to the position they were 
previously in. This will not always work since it relies on the `window manager class`
to track down the resulting windows and some applications do not set this for some reason.
And windows don't really want to be moved when they cover a whole screen; even without being fullscreen (not sure why 
that is).
This is also done via the dbus interface of my extension.
