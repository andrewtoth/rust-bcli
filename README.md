# Rust BCLI

A CoreLightning bitcoin backend plugin written in rust to replace the built-in 
bcli.

This plugin implements all five required bitcoin backend methods:
- `getchaininfo`
- `estimatefees`
- `getrawblockbyheight`
- `getutxout`
- `sendrawtransaction`

However, unlike bcli they can be compiled out optionally so only a subset will
be implemented. This allows other plugins which only need to implement one of
the five required methods to be used in conjunction with this plugin.

For instance, using plugin to perform a different sendrawtransaction 
implementation to send over tor. Or substitute estimatefees to use mempool.space
for fee estimation.

## Installation

The following commands build the plugin binary:

```
git clone https://github.com/andrewtoth/rust-bcli
cd rust-bcli
cargo install --path .
```

The binary will now be at `$HOME/.cargo/bin/rust-bcli`. You can now place this
binary into the plugins folder, add it to the conf file with
`plugin=$HOME/.cargo/bin/rust-bcli` (replace `$HOME` with your home directory),
or add it as a command line option via 
`lightningd --plugin=$HOME/.cargo/bin/rust-bcli --disable-plugin=bcli`. You must
also disable bcli with the `--disable-plugin=bcli` command line option or put
`disable-plugin=bcli` in the conf file.

To remove a specific backend method, install with features specifying which 
methods to disable. For example, the following command will remove estimatefees
from the binary installing:
```
cargo install --path . --features noestimatefees
```
In order to use this binary now, another plugin that only implements
`estimatefees` must be used.

