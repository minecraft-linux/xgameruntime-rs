# xgameruntime rust

This is not yet a full rust implementation.

`xgameruntime.gdk.dll` is the official gdk sdk `.dll` renamed and needed for this wrapper to work.

You find the original `xgameruntime.dll` inside of the 2604 sdk installer `PC Development.msi` [Microsoft GDK](https://github.com/microsoft/gdk), you can use `msiextract` under linux / macOS to get this without windows.

To use Xuser with Minecraft for Windows, you currently need to follow this [PR description](https://github.com/minecraft-linux/xgameruntime-rs/pull/2) to patch `MicrosoftGame.config` until XUser is generally available.
