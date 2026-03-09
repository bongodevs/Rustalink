# Installation

Get Rustalink up and running in minutes. Whether you prefer building from source or using our pre-packaged binaries, we've got you covered.

## Prerequisites

Before building from source, ensure you have the following installed on your system:

- **[Rust](https://rustup.rs/)** (latest stable toolchain)
- **CMake** (required for building certain C dependencies)
- **OpenSSL** (required for secure connections)

## Install from Releases

The fastest way to get started is by downloading a pre-compiled binary from our GitHub Releases page.

1. Navigate to the [Rustalink Releases](https://github.com/appujet/Rustalink/releases) page.
2. Download the latest archive for your operating system (Windows, macOS, or Linux).
3. Extract the binary and place it in your desired folder.
4. Download the `config.example.toml`, rename it to `config.toml`, and place it in the same directory.
5. Execute the binary: `./rustalink` (or `rustalink.exe` on Windows).

## Build from Source

Compiling Rustalink from source guarantees you get the very latest performance optimizations specific to your machine architecture.

1. **Clone the repository:**

   ```bash
   git clone https://github.com/appujet/Rustalink.git
   cd Rustalink
   ```

2. **Build the project:**
   This step will download dependencies and compile the server. It might take a few minutes on the first run.

   ```bash
   cargo build --release
   ```

## Run the Server

Before starting, ensure your `config.toml` is correctly configured in your working directory. You can copy the example configuration:

```bash
cp config.example.toml config.toml
```

Start the compiled binary:

```bash
./target/release/rustalink
```

Once running, you should see logs indicating that the server is listening for connections. You're now ready to connect your Lavalink clients!
