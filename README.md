## Config file

`xrcon` reads `~/.xrcon.ini` (or the file given with `--config`). Sections are
selected with `-n/--name`; `[DEFAULT]` is the fallback. Keys: `server`,
`password`, `type`, `timeout`.

**Known limitation:** the INI parser treats `;` and `#` as comment markers,
even inside a value. A password containing `;` or `#` is cut off at that
character. For such passwords, use `-p` or the interactive prompt instead of
the config file.
