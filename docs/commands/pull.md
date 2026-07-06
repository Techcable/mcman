# `mcman pull <file>`

'Pulls' a file from `server/` to `config/`

Example usage:

```sh
~/smp $ ls
 ...
 server.toml
 ...

~/smp $ cd server/config/SomeMod

~/smp/server/config/SomeMod $ mcman pull config.txt
  server/config/SomeMod/config.txt => config/config/SomeMod/config.txt
```

Use `--dry-run` to preview what would be pulled without copying any files.
