# Prismatic
A CLI tool for updating Stardew Valley mods.

## Installation
To install run `cargo install --git https://github.com/TheSuperGamer20578/Prismatic`,
or without Nexus support (see [Nexus Mods](#nexus-mods))
`cargo install --git https://github.com/TheSuperGamer20578/Prismatic --no-default-features -F github`

## Usage
To update all mods simply run `prismatic update`.
The mod directory defaults to `~/.steam/steam/steamapps/common/Stardew Valley/Mods` on Unix based systems
and `C:\Program Files (x86)\Steam\steamapps\common\Stardew Valley\Mods` on Windows,
if Stardew Valley is installed to an atypical location you can specify it with `-d <dir>`
(e.g. `prismatic -d <dir> update`)

### Nexus Mods
Nexus support is achieved via grabbing your `sid_develop` cookie from your browser, therefore you must be signed in to
download mods from Nexus. If you are uncomfortable with this you can compile without the `nexus` feature.

### Config Files
Currently only files or directories named `config` or `config.json` are copied when updating.
When updating the mod's directory is moved into `.old` and is otherwise untouched,
if some config files aren't automatically copied they can manually be copied from there.
