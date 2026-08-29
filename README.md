# mini_udp

[![Docs](https://img.shields.io/docsrs/mini_udp/latest)](https://docs.rs/mini_udp/latest/mini_udp/)
[![License](https://img.shields.io/crates/l/mini_udp.svg)](https://github.com/faervan/mini_udp#license)
[![Crates.io](https://img.shields.io/crates/v/mini_udp.svg)](https://crates.io/crates/mini_udp)
<!-- cargo-rdme start -->

## Overview
A minimal, fully synchronous implementation of a reliability protocol on top of UDP.

This was inspired by Glenn Fiedler, who wrote an amazing set of articles about this:
<https://gafferongames.com/categories/building-a-game-network-protocol/>

The main entry points of this crate are [`UdpCommunicator`](https://docs.rs/mini_udp/latest/mini_udp/communicator/struct.UdpCommunicator.html)
and [`MultiUdpCommunicator`](https://docs.rs/mini_udp/latest/mini_udp/communicator/multi/struct.MultiUdpCommunicator.html), which both wrap
[`std::net::UdpSocket`](https://doc.rust-lang.org/stable/std/net/udp/struct.UdpSocket.html).

All messages are (de)serialized by the in-house [`ByteRepr`](https://docs.rs/mini_udp/latest/mini_udp/byte_repr/trait.ByteRepr.html) trait, which has a derive macro as well:
[`ByteRepr`](https://docs.rs/mini_udp_derive/latest/mini_udp_derive/derive.ByteRepr.html).

### Features
- [x] Derive byte representations for Enums and Structs.
- [x] Send unreliable, reliable, or reliable ordered messages over UDP.
- [x] Verify packet integrity using a 4-byte CRC, seeded with a user-defined protocol version.
- [x] Messages get combined into packets, with a maximum packet size of 1024 bytes
  ([`MAX_PACKET_DATA_LEN`](https://docs.rs/mini_udp/latest/mini_udp/packet/const.MAX_PACKET_DATA_LEN.html)).
- [x] Handle 1-X communication via a single, shared UDP socket
  ([`MultiUdpCommunicator`](https://docs.rs/mini_udp/latest/mini_udp/communicator/multi/struct.MultiUdpCommunicator.html)).
- [x] Have a fully synchronous, non-blocking API, ideal for game networking.
- [ ] Messages cannot be fragmented yet, so **it is not possible to send messages larger than
  1024 bytes! (yet)**
- [ ] The [`ByteRepr`](https://docs.rs/mini_udp_derive/latest/mini_udp_derive/derive.ByteRepr.html) derive is not very mindful of bandwidth yet
  (booleans are padded to 1 byte, strings and vecs use 4 bytes to send their length as u32).

### Example
<details>
<summary><i>Show example</i></summary>

```rust
use mini_udp::prelude::*;

#[derive(ByteRepr, Debug, PartialEq)]
enum MessageToServer {
    Hello,
    Position([f32; 3]),
}
#[derive(ByteRepr, Debug)]
enum MessageToClient {
    WhatIsYourPosition,
    Bye,
}

/// The protocol version is used as seed for the CRC algorithm. Thus, when receiving a packet
/// that was send from a communicator with a different version, the CRC check will fail.
const PROTOCOL_VERSION: u32 = 1;
type ClientCtx = UdpContext<MessageToServer, MessageToClient, PROTOCOL_VERSION>;
type ServerCtx = <ClientCtx as MiniUdpContext>::Reverse;

const POSITION: [f32; 3] = [-1., 0.004, 2482.3];

let mut server = MultiUdpCommunicator::<ServerCtx>::bind("0.0.0.0:7001");
// `UdpCommunicator::default()` binds the communicator to "0.0.0.0:0", which lets the OS decide
// which port to use.
let mut client =
    UdpCommunicator::<ClientCtx>::default().connect("0.0.0.0:7001").unwrap();

// The `write*` methods only add the message to a queue, they won't be send until you explicitly
// call `send()`.
client.write_ordered(MessageToServer::Hello);

let mut messages_read = 0;
loop {
    // Send all queued messages. This is also responsible for resending reliable packets if
    // they have not received an acknowledgement yet.
    client.send().unwrap();
    // Receive all new packets. You can provide a callback function that will be called for each
    // received packet, with a mutable reference to the associated connection.
    server.recv(|mut com: UdpCommunicatorMut<_>| {
        if let Some(msg) = com.read_ordered() {
            messages_read += 1;
            match msg {
                MessageToServer::Hello =>
                    com.write_ordered(MessageToClient::WhatIsYourPosition),
                MessageToServer::Position(pos) => {
                    assert_eq!(pos, POSITION);
                    com.write_ordered(MessageToClient::Bye);
                }
            }
        }
    });
    server.send();
    client.recv();
    // If we would call `client.read()` here, we would not get any messages because ordered and
    // non-ordered receive queues are separated.
    if let Some(msg) = client.read_ordered() {
        messages_read += 1;
        match msg {
            MessageToClient::WhatIsYourPosition => {
                client.write_ordered(MessageToServer::Position(POSITION));
            }
            MessageToClient::Bye => break,
        }
    }
}
assert_eq!(messages_read, 4);
```
</details>

<!-- cargo-rdme end -->
## License
All code in this repository is dual-licensed under either

* MIT License ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))
* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))

at your option.

### Contributions

Unless you explicitly state otherwise,
any contribution intentionally submitted for inclusion in the work by you,
as defined in the Apache-2.0 license,
shall be dual licensed as above,
without any additional terms or conditions.
