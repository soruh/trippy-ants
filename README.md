# Trippy Ants

A visually attractive simulation based on cellular automata and particle systems.

Heavily inspired by [Sebastian Lague's Slime Simulation](https://github.com/SebLague/Slime-Simulation).

![screenshot of the running simulation](screenshot-1.png)

![screenshot of the running simulation with anti-ants](screenshot-2.png)

The current state is a proof of concept and needs a lot of cleanup, but I promised to upload it in a timely manner. I might add the ability to live-edit parameters through a simple browser interface.

## How to run

- [install a rust toolchain](https://rust-lang.org/learn/get-started/) if you haven't already
- check out this repository to your local machine
- change into the project directory and run `cargo run --release` from the terminal

A window will open with a simulation while the terminal will output the number of computed frames for each second.

Save a screenshot using the `SPACE` key or close the window using the `ESC` key.

### Troubleshooting

If you're getting the error message "linker 'cc' not found" you might have to [install some basic developer tools](https://stackoverflow.com/questions/52445961/how-do-i-fix-the-rust-error-linker-cc-not-found-for-debian-on-windows-10) and then retry:

- for Ubuntu/Debian/Mint: `sudo apt install build-essential`
- for Arch Linux: `sudo pacman -S base-devel`
- for CentOS: `sudo yum install gcc`
- for Solus: `sudo eopkg it -c system.devel`

## Creating your own simulation

To start your own simulation rather than the included demo, you need to provide the path to a configuration file on the command line. An example can be found in `demo.config.toml`. Start it by running:

```sh
cargo run --release -- demo.config.toml
```

While the simulation is running it keeps watching the configuration file and will update automatically if it changes. Some settings however will only be applied after a restart.

The documentation of all possible options can be found in `src/config.rs`.

## About AI

I used Composer 2 to get a first draft (an ugly fire demo effect) to scaffold a window with an update loop to modify the frame-buffer. It also generated the code for saving a PNG screenshot on pressing space.

I fully understand the entire code and the core algorithm was entirely developed by myself. I used an AI to assist with auto-complete to focus on the actual algorithm.
