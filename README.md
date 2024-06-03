# Wasting

A game for TINS 2024, by SiegeLord.

## Plot

A deadly disease has struct the local star sector and you are the only ship
that has not been infected. You are tasked to transport supplies to the planets
with survivors, so the scientists can work on a cure.

## Default Controls

- *Space/Up* - Activate thruster
- *Left/Right* - Rotate ship
- *Tab* - Hold to show sector map
- *Escape* - Open in-Game Menu

## Gameplay Hints

- The research progress depends on the total sector population

## Building instructions

1. Install Allegro 5.
2. Install Rust from rust-lang.org. You'll need the nightly version.
3. Run `cargo run --release` to build and run the game.

On Windows, you can use the pre-built binaries (extracted in the `allegro`
subdirectory). `run_msys.sh` may be useful for this purpose.

## Rules

1. *genre rule #99: Trains.* - You form a train of crates behind your ship.

2. *art rule #155: Art Nouveau.* - I tried adding some spindly/womanly things
   to the menus.

3. *tech rule #117: Custom Paint Job.* - You can alter your ship's appearance.

4. *tech rule #98: use a quadratic formula (i.e. ax^2 + bx + c) somewhere in
   the game.* - The terrain on planets is made using the quadratic formula (see
   source code, search for `a * x * x + b * x + c`).

5. *bonus rule #22: Act of "Cool Story, Bro"* - Not used.

## Attributions

### Font

- https://www.dafont.com/neoletters.font

### Music

- https://modarchive.org/index.php?request=view_by_moduleid&query=165819
- https://modarchive.org/index.php?request=view_by_moduleid&query=166285
- https://modarchive.org/index.php?request=view_by_moduleid&query=165797
- https://modarchive.org/index.php?request=view_by_moduleid&query=174124
- https://modarchive.org/index.php?request=view_by_moduleid&query=178992
