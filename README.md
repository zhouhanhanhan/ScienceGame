# ScienceGame

A minimal version of the code to demonstrate how the blockchain science game works. This version contains the core logic of the game evaluation design and has not been implemented with the UI and on-chain settings.

This implementation is based on [race protocol](https://github.com/RACE-Game/race).

## How to run
1. Clone the project
```bash
git clone https://github.com/zhouhanhanhan/science-game.git
```
2. Clone the race protocol
Since the race protocol is a developing project and has not been published as a library, we need to clone it from GitHub.
```bash
git clone https://github.com/RACE-Game/race.git
```
3. Put the project folder *science-game* under the *race/examples* folder. The directory structure should look like this:

   ```plaintext
   race/
   ├── examples/
   │   ├── science-game/
   │   │   ├── src/
   │   │   ├── Cargo.toml
   ```
4. Run the command
```bash
cd race/examples/science-game

cargo test -- --nocapture
```