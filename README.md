[![Crates.io](https://img.shields.io/crates/v/usb-timing-box-rs.svg)](https://crates.io/crates/usb-timing-box-rs)
[![Documentation](https://docs.rs/usb-timing-box-rs/badge.svg)](https://docs.rs/usb-timing-box-rs/)
[![dependency status](https://deps.rs/repo/github/timolehnertz/usb-timing-box-rs/status.svg)](https://deps.rs/repo/github/timolehnertz/usb-timing-box-rs)

# usb-timing-box-rs
Rust library for interfacing with a race result usb timing box via serial connection.

## Disclaimer
This project is not associated with race result in any way.

## Getting started
To get started either add the library to your own project with `cargo add race-result-decoder` or add it to your `Cargo.toml` file.

Alternatively you can clone this repository and run the example:
```bash
cargo run --example read_passings
```

## Features

Optional Cargo features extend the passing types (`PassingFw25`, `PassingFw26`, `StrengthCombined`, `PassingBatch`, and firmware markers `Fw25` / `Fw26`):

- **`serde`** — `Serialize` and `Deserialize` via [serde](https://docs.rs/serde)
- **`schemars`** — `JsonSchema` via [schemars](https://docs.rs/schemars) (e.g. `schemars::schema_for!(PassingFw25)`)
- **`chrono`** — [`PassingFw25::datetime_utc`] / [`PassingFw26::datetime_utc`] for UTC [`chrono::DateTime`](https://docs.rs/chrono/latest/chrono/struct.DateTime.html)

## Minimum Supported Rust Version (MSRV)
The MSRV is 1.88.0 (The version that stabilized let chains).

## Documentation
This crate is based on the official documentation for the USB Timing Box. You can find that [here](https://www.raceresult.com/en/shophelp/index?id=346-Commands) and [here](https://www.raceresult.com/en/support/kbexport2?id=18).

Most data structures and functions inside this crate are documented with relevant snippets from the official documentation. However it is highly recommended to read the official documentation for the most up to date information.

## Contributing
Contributions are welcome! Please feel free to submit an issue or pull request.

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.