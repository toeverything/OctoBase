<div align="center">

<h1 style="border-bottom: none">
    <b><a href="https://octobase.pro/">Octobase</a></b><br />
    🐙 Local-first, yet collaborative. A light-weight, scalable, data engine written in Rust.
    <br>
</h1>

[![Rust-version-icon]](https://www.rust-lang.org/)
[![codecov]](https://codecov.io/gh/toeverything/OctoBase)
[![stars]](https://github.com/toeverything/OctoBase)
[![Issues Closed]](https://github.com/toeverything/OctoBase/issues?q=is%3Aissue+is%3Aclosed)
[![Join Telegram]](https://t.me/affineworkos)
[![Follow Twitter]](https://twitter.com/AffineOfficial)

OctoBase is an offline-available, scalable, self-contained collaborative database, which was originally designed for AFFiNE. AFFiNE is a local-first open source knowledge base that provides full functionality in any network environment.

Based on OctoBase, you can not only implement a rich text editor for offline writing, but also implement richer offline collaboration functions based on OctoBase's data abstraction, such as: multidimensional tables, drawing boards, etc.

</div>

## Features

As an offline collaborative data database, OctoBase has the following characteristics:

-   📚 **Multi-platform available** offline collaboration, Schemaless, structured/unstructured/rich text data storage.

-   🗃️ **Binary storage** that supports data deduplication and rich media editing.

-   🔍 **High-performance real-time full-text indexing** with high-quality multilingual word segmentation support.

-   🌐 **CRDT-driven P2P synchronization** with rich multi-platform native support.

-   🔒 **Fine-grained permission control** with advanced permission management.

OctoBase provides native support for offline collaboration, full-text indexing, and binary storage, making it easy for developers to build secure and high-performance local-first collaborative applications that work seamlessly across multiple platforms.

With OctoBase, you will have access to same data abstractions across platform that enable you to maintain consistency and coherence across all your applications, regardless of the devices or platforms used.

Additionally, OctoBase can function as a standalone server database, or it can be integrated directly into your application as an embedded database while remaining fully functional.

## Project status

**The OctoBase project is currently under heavy development, most components are not yet production ready. Major changes may occur at any time before the version reaches 1.0.**

OctoBase has been used in [AFFiNE Cloud]. We are currently working on the following components:

-   Core
    -   ✅ CRDT operation (based on yrs)
    -   🚧 CRDT operation (based on [jwst-codec])
    -   ✅ CRDT-based data structure compatible with [blocksuite]
    -   🚧 Full-text indexing
-   Sync
    -   ✅ Blob Sync abstraction
        -   ✅ RESTful
        -   🚧 S3
    -   ✅ [Collaboration abstraction]
        -   ✅ [WebSocket connector]
        -   ✅ [WebRTC connector]
        -   🚧 libp2p connector
-   Storage
    -   ✅ Storage agnostic CRDT data storage
        -   ✅ SQLite adapter
        -   ✅ Postgres adapter
        -   🚧 S3 adapter
    -   ✅ Storage agnostic Blob storage
        -   ✅ SQLite adapter
        -   ✅ Postgres adapter
        -   🚧 S3 adapter
-   Binding
    -   🚧 [Java binding]
    -   🚧 [Swift binding]

## Contributions

A very good place to ask questions and discuss development work is our [Telegram Group].

We gladly accept contributions via GitHub pull requests, you can go to [contributing] to see more information.

**Before you start contributing, please make sure you have read and accepted our [Contributor License Agreement]. To indicate your agreement, simply edit this file and submit a pull request.**

## Goals

OctoBase aims to make it easy for developers to build local-first applications
on all common platforms. In order to achieve this goal, we will strive to do these things:

-   Make it easy to build on all supported platforms.
-   CRDT-driven peer-to-peer synchronization model.
-   Self-contained library distribution.
-   Minimize external dependencies.
-   Out-of-the-box permission control.

You can see a summary of our recent updates here: [What's New](./apps/homepage/pages/docs/overview/whats_new.md).

## Project Overview

```shell
├── apps
│   ├── android ##  Android scaffolding project
│   ├── homepage ## OctoBase homepage & docs
│   ├── cloud ## affine-cloud backend
│   └── keck ## collaboration backend
│   └── swift ## iOS scaffolding project
└── libs ##
    ├── jwst  ## OctoBase core library
    ├── jwst-binding ## Multilingual FFI bindings for OctoBase
    ├── jwst-codec ## CRDT implementation for OctoBase
    ├── jwst-logger ## logger plugins for OctoBase
    ├── jwst-rpc ## sync plugins for OctoBase
    ├── jwst-storage ## storage plugins for OctoBase
```

In the process of project development, there are many software development concepts that have influenced us. Thank you very much for these excellent software:

-   [Fossil] -- Source code management tool made with CRDTs which inspired our design on block data structure.
-   [SQLite] -- "Small. Fast. Reliable. Choose any three." We like this idea very much.

## Hiring

Some amazing companies including OctoBase are looking for developers! Are you interested in helping build with OctoBase and/or its partners? Check out some of the latest [jobs available](https://github.com/toeverything/AFFiNE/blob/master/docs/jobs.md).

## Building

See [BUILDING.md](./apps/homepage/pages/docs/building_guide.md) for instructions on how to build Octobase from source code.

## License

Currently, this repository is under **active development** and most components are not yet production ready, so all code is under [AGPL-3.0]. We will switch to [MPL 2.0] or a more looser license release after the corresponding components are ready for production.

[agpl-3.0]: /LICENSE
[contributing]: .github/CONTRIBUTING.md
[telegram group]: https://t.me/affineworkos
[mpl 2.0]: https://www.mozilla.org/en-US/MPL/2.0/
[fossil]: https://www2.fossil-scm.org/home/doc/trunk/www/index.wiki
[sqlite]: https://sqlite.org/index.html
[contributor license agreement]: https://github.com/toeverything/octobase/edit/master/.github/CLA.md
[our website]: https://octobase.pro
[codecov]: https://codecov.io/gh/toeverything/octobase/branch/master/graphs/badge.svg?branch=master
[stars]: https://img.shields.io/github/stars/toeverything/OctoBase.svg?style=flat&logo=github&colorB=red&label=stars
[Follow Twitter]: https://img.shields.io/badge/-Twitter-grey?logo=twitter
[Join Telegram]: https://img.shields.io/badge/join-telegram-blue
[Issues Closed]: https://img.shields.io/github/issues-closed/toeverything/OctoBase?color=6880ff&logo=github
[rust-version-icon]: https://img.shields.io/badge/Rust-1.70.0-dea584?logo=rust
[affine cloud]: https://app.affine.pro
[jwst-codec]: ./libs/jwst-codec/
[blocksuite]: https://github.com/toeverything/blocksuite
[Collaboration abstraction]: https://github.com/toeverything/OctoBase/issues/287
[WebSocket connector]: ./libs/jwst-rpc/src/connector/tungstenite_socket.rs
[WebRTC connector]: ./libs/jwst-rpc/src/connector/webrtc.rs
[Java binding]: ./libs/jwst-binding/jwst-jni
[Swift binding]: ./libs/jwst-binding/jwst-swift
