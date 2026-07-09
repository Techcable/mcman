# `mcman outdated`

Checks plugins, mods and the server jar (when pinned to a Modrinth, Hangar or
Spigot version) for newer versions available upstream.

This only reports what's outdated - it does not edit `server.toml` or download
anything. Addons pinned to `"latest"`, or sourced from anything other than
Modrinth/Hangar/Spigot (CurseForge, URL, Github Releases, Jenkins, Maven), are
skipped since they're always resolved fresh on the next build.

By default, only a Hangar project's `Release` channel is considered when
looking for a newer version - Beta/Alpha/Snapshot channels are ignored. Pass
`--all-channels` to consider every channel instead.

Example usage:

```sh
~/smp $ mcman outdated
Kind   Addon                Current Latest
------ -------------------- ------- ------
Plugin Modrinth:essentialsx 2.21.0  2.22.0
```
