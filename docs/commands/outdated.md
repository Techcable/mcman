# `mcman outdated`

Checks plugins, mods and the server jar for newer versions available upstream:
addons pinned to a Modrinth, Hangar or Spigot version, and a PaperMC-family
server jar (`type = "papermc"`, eg. paper/velocity/waterfall) pinned to an
explicit `build`, checked through the [Fill API](https://fill.papermc.io/).

This only reports what's outdated - it does not edit `server.toml` or download
anything. Anything pinned to `"latest"`, or sourced from anything else
(CurseForge, URL, Github Releases, Jenkins, Maven, Purpur, ...), is skipped
since it's always resolved fresh on the next build.

By default, only a Hangar plugin's `Release` channel is considered when
looking for a newer version - Beta/Alpha/Snapshot channels are ignored. Add a
`channels` list to a Hangar plugin to also watch other channels:

```toml
[[plugins]]
type = "hangar"
id = "SomePlugin"
version = "1.2.3"
channels = ["Release", "Beta"]
```

A configured channel other than `Release` that doesn't exist for the project
(or just has no versions) prints a warning and is otherwise skipped; `Release`
is assumed to always exist and never warns.

Modrinth addons work the same way, using Modrinth's three version types
(`release`, `beta`, `alpha`) as channels. The default is `["release"]`:

```toml
[[plugins]]
type = "modrinth"
id = "some-plugin"
version = "abcd1234"
channels = ["release", "beta"]
```

Since Modrinth channels are a fixed set rather than arbitrary per-project
names, an unrecognized entry (eg. a typo) always warns; a recognized channel
simply having no matching version does not.

Pass `--all-channels` to ignore every addon's `channels` setting and search
every channel instead, for both Hangar and Modrinth.

Example usage:

```sh
~/smp $ mcman outdated
Kind   Addon                Current Latest
------ -------------------- ------- ------
Plugin Modrinth:essentialsx 2.21.0  2.22.0
```
