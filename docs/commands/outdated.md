# `mcman outdated`

Checks plugins, mods and the server jar for newer versions available upstream:
addons pinned to a Modrinth, Hangar or Spigot version, and a PaperMC-family
server jar (`type = "papermc"`, eg. paper/velocity/waterfall) pinned to an
explicit `build`, checked through the [Fill API](https://fill.papermc.io/).

This only reports what's outdated - it does not edit `server.toml` or download
anything. Anything pinned to `"latest"`, or sourced from anything else
(CurseForge, URL, Github Releases, Jenkins, Maven, Purpur, ...), is skipped
since it's always resolved fresh on the next build.

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
