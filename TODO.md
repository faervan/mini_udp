## TODO

### Documentation
- [ ] Add a bevy example.
- [ ] Add a "How it works" readme section, explaining how it works in a nutshell.
    - Unreliable vs. reliable vs. reliable ordered packets/message queues

### Communicator Features
- [ ] Add a `set_packet_header_message` method to `Communicator`, that makes all packets
    start with the provided message.
- [ ] Implement message fragmentation to allow large messages.
- [ ] Consider adding an opt-in auto-disconnect for `MultiUdpCommunicator` connections?

### `ByteRepr`
- [ ] Implement for `Result<T, E>`
- [ ] Implement for `char`
- [ ] Implement for tuples
- [ ] Implement bit packing for booleans, enum variants, etc.
- [ ] Add an option to let the length of one variable length type be inferred from the slice length.
- [ ] Implement `StaticByteRepr` for types with a delegated `T where T: StaticByteRepr` .

### Misc
- [ ] Add tests for behavior of `&'a str`, `Option<&'a str>`, etc.
- [ ] Add an (optional?) packet receive error cache.
    - Adjust the `test_protocol_version_check` test to assert the error.
- [ ] Refactor error handling with a new `mini_udp::Error`.

## DONE
- [x] Add a `CHANGELOG.md` file
- [x] Move the `with_fake_*` methods into a feature gated debug trait.
- [x] Allow the user to define the protocol version used to init the CRC.
- [x] Add a max-resend ~timer~ *count* for reliable packets (we shouldn't keep on trying to send the same
    packets forever if the other side is just not reachable).
