# Yjs/Yrs sync protocol implementation

This crate comes with the default implementation of [Yjs/Yrs synchronization protocol](https://github.com/yjs/y-protocols/blob/master/PROTOCOL.md) - it's shared between the projects and can be used to interop between eg. Yjs client and Yrs server.

It also implements [awareness protocol](https://docs.yjs.dev/api/about-awareness) and corresponding `Awareness` structure.

The implementation just like the protocol itself is flexible and can be extended by custom implementation of `Protocol` trait. If that's not necessary, the `DefaultProtocol` is provided.