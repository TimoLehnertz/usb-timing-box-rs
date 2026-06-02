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

## Minimum Supported Rust Version (MSRV)
The MSRV is 1.88.0 (The version that stabilized let chains).

## Documentation
This crate is based on the official documentation for the USB Timing Box. You can find that [here](https://www.raceresult.com/en/shophelp/index?id=346-Commands) and [here](https://www.raceresult.com/en/support/kbexport2?id=18).

Most data structures and functions inside this crate are documented with relevant snippets from the official documentation. However it is highly recommended to read the official documentation for the most up to date information.

## Contributing
Contributions are welcome! Please feel free to submit an issue or pull request.

## License
This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.