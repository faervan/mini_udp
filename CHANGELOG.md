# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Derived `ByteRepr` for `Cow<'a, str>`

### Changed

- Replaced generic `SEND`, `RECV` and `const PROTOCOL_VERSION` parameters to a single
    `CTX` that bundles all other generics in the new `MiniUdpContext` trait to
    reduce repetition of the generics and simplify adding more generics later on.

## [0.4.0] - 2026-08-13

### Added

- A `retain` method to the `RingBuffer` implementation. (Run a provided closure on all items,
    and remove all items for which the closure returns `false`).
- Two new methods on `CommunicatorSocket`: `with_max_reliable_unordered_retries` and
    `with_max_reliable_ordered_retries` that allow the user to configure the maximum amount
    of times a packet will be resend. The default is 100.

## [0.3.1] - 2026-08-11

### Fixed

- Added missing `docsrs` feature to fix `docs.rs` build.

## [0.3.0] - 2026-08-11

### Added

- Publish helper script with automatic test and lint checks.
- Exposed the protocol version that seeds the `CRC` to be defined by the user as a
    `PROTOCOL_VERSION` const generic for `UdpCommunicator` and `MultiUdpCommunicator`.
- Added a `TODO.md`.
- Added this `CHANGELOG.md`.

### Changed

- Moved the `with_fake*` methods into a new `MiniUdpDebugExt` trait that is gated behind
    the `debug` cargo feature.

### Fixed

- Fix or explicitly allow `rustc` and `clippy` lints.

## [0.2.0] - 2026-08-09

### Added

- Implemented `ByteRepr` for `String`.
- Implemented `ByteRepr` for `Cow<T>`.
- Implemented `ByteRepr` for `Option<T>`.
- Usage example of the `ByteRepr` derive in the `ByteRepr` trait docs.
- Usage example in the `UdpCommunicator` docs.
- Usage example in the `MultiUdpCommunicator` docs.
- Added a feature description to the crate page.
- Implemented `OnReceiveCallback` for `()`, to allow `.recv(())` without an actual callback.
- Added an internal `derive_for!` macro to synchronize the `ByteRepr` implementations generated
    by the derive macro with implementations for standard types provided by `mini_udp`.
- The `ByteRepr` derive now also derives `StaticByteRepr` when the type is detected to be of
    static size.

### Changed

- Exposed `MAX_PACKET_LEN`, `PACKET_HEADER_LEN` and `MAX_PACKET_DATA_LEN` constants.
- Array deserialization is now done using `std::array::from_fn` instead of collecting into a
    `Vec`. This does, however, introduce a `T: Default` bound for the contained element.
- Removed unnecessary `unsafe` from the `Ringbuffer::iter_mut` and `values_mut`.
- Now using `cargo-rdme` to sync the crate docs to the `README.md` and expand `rustdoc` links.
