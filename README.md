# xgameruntime rust

This is not yet a full rust implementation.

`xgameruntime.gdk.dll` is the official gdk sdk `.dll` renamed and needed for this wrapper to work.

You find the original `xgameruntime.dll` inside of the 2604 sdk installer `PC Development.msi` [Microsoft GDK](https://github.com/microsoft/gdk), you can use `msiextract` under linux / macOS to get this without windows.

To use Xuser with Minecraft for Windows, you currently need to follow this [PR description](https://github.com/minecraft-linux/xgameruntime-rs/pull/2) to patch `MicrosoftGame.config` until XUser is generally available.

## Getting started with other games

For an overview what had been tested with it look at <https://bellezaemporium.github.io/xodus-list/>

- `cargo run --features key-chain-file -- login` (on xodus) generates `~/.xodus-keyring.ron`
- copy `~/.xodus-keyring.ron` to `%USERPROFILE%/.xodus-keyring.ron` of your wine prefix
- build `--features xuser` of `xgameruntime-rs` (as win32) (you can download xuser version from github actions artifact xgameruntime-xodus-static)
- put xgameruntime.dll into system32 (**use winedll override to native if xgameruntime is builtin, can happen on some winegdk/xodus protons**)
- put ms official gdk as xgameruntime.GDK.dll into system32
- run the game via wine/proton/whatever (now depends if it is fully decrypted or needs xodus + winepatch to launch, proton might need some tweaks for system32 dlls to survive)
