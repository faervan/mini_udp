## TODO
- [ ] Add a bevy example.
- [ ] Add a "How it works" readme section, explaining how it works in a nutshell.
    - Unreliable vs. reliable vs. reliable ordered packets/message queues
- [ ] Add a `set_packet_header_message` method to `Communicator`, that makes all packets
    start with the provided message.
- [ ] Implement message fragmentation to allow large messages.
- [ ] Add tests for behavior of `&'a str`, `Option<&'a str>`, etc.
- [ ] Add an (optional?) packet receive error cache.
    - Adjust the `test_protocol_version_check` test to assert the error.
- [ ] Add a max-resend timer for reliable packets (we shouldn't keep on trying to send the same
    packets forever if the other side is just not reachable).
- [ ] Consider adding an opt-in auto-disconnect for `MultiUdpCommunicator` connections?

## DONE
- [x] Add a `CHANGELOG.md` file
- [x] Move the `with_fake_*` methods into a feature gated debug trait.
- [x] Allow the user to define the protocol version used to init the CRC.
