# OctoBase

[![Issues Closed](https://img.shields.io/github/issues-closed/toeverything/OctoBase?color=6880ff)](https://github.com/toeverything/OctoBase/issues?q=is%3Aissue+is%3Aclosed)
[![Join Telegram](https://img.shields.io/badge/join-telegram-blue)](https://t.me/affineworkos)
[![Follow Twitter](https://img.shields.io/badge/-Twitter-grey?logo=twitter)](https://twitter.com/AffineOfficial)
[![stars](https://img.shields.io/github/stars/toeverything/OctoBase.svg?style=flat&logo=github&colorB=red&label=stars)](https://github.com/toeverything/OctoBase)

OctoBase is an offline-available, scalable, self-contained collaborative database, which was originally designed for AFFiNE. AFFiNE is a local-first open source knowledge base that provides full functionality in any network environment.

Based on OctoBase, you can not only implement a rich text editor for offline writing, but also implement richer offline collaboration functions based on OctoBase's data abstraction, such as: multidimensional tables, drawing boards, chat software, etc.

As an offline collaborative data database, OctoBase has the following characteristics:

-   📚 **Multi-platform available** offline collaboration, Schemaless, structured/unstructured/rich text data storage .

-   🗃️ **Binary storage** that supports data deduplication and rich media editing.

-   🔍 **High-performance real-time full-text indexing** with high-quality multilingual word segmentation support.

-   🌐 **Point-to-point / central server synchronization** with rich multi-platform native support.

-   🔒 **Fine-grained permission control** with advanced permission management.

By providing native offline collaboration, full-text indexing, and binary storage, OctoBase enables you to easily build secure, high-performance local-first collaborative applications using the same set of data abstractions on multiple platforms.

OctoBase can be used either as a stand-alone server database, or directly included in your application as an embedded database and remain fully functional.

## Project status

**The OctoBase project is currently under heavy development, most components are not yet production ready. Major changes may occur at any time before the version reaches 1.0.**

## Contributions

A very good place to ask questions and discuss development work is our [Telegram Group].

We gladly accept contributions via GitHub pull requests, you can go to [contributing] to see more information.

**Before you start contributing, please make sure you have read and accepted our [Contributor License Agreement]. To indicate your agreement, simply edit this file and submit a pull request.**

## Goals

OctoBase aims to make it easy for developers to build local-first applications
on all common platforms. In order to achieve this goal, we will strive to do these things:

-   Make it easy to build on all supported platforms.
-   Implement basic data types that support collaboration.
-   Support peer-to-peer synchronization.
-   Self-contained library distribution.
-   Minimize external dependencies.

## Project Overview

```shell
├── apps
│   ├── android ##  Android scaffolding project
│   ├── frontend ## OctoBase playground, landing-page, OctoBase typescript version
│   ├── handbook ## OctoBase docs
│   ├── cloud ## affine-cloud backend
│   └── keck ## collaboration backend
└── libs ##
    ├── jwst  ## OctoBase core library
    ├── jwst-binding/jwst-ffi ## OctoBase binging for C ffi
    ├── jwst-binding/jwst-jni# ## OctoBase binding for JNI
    ├── jwst-binding/jwst-wasm ## OctoBase binding for WASM
    ├── jwst-logger ## logger plugins for OctoBase
    └── jwst-storage ## multiple platform storage plugins for OctoBase
```

In the process of project development, there are many software development concepts that have influenced us. Thank you very much for these excellent software:

-   [Fossil] -- Source code management tool made with CRDTs which inspired our design on block data structure.
-   [SQLite] -- "Small. Fast. Reliable. Choose any three." We like this idea very much.

## License

Currently, this repository is under **active development** and most components are not yet production ready, so all code is under [AGPL-3.0]. We will switch to [MPL 2.0] or a more looser license release after the corresponding components are ready for production.

[agpl-3.0]: /LICENSE
[contributing]: .github/CONTRIBUTING.md
[telegram group]: https://t.me/affineworkos
[mpl 2.0]: https://www.mozilla.org/en-US/MPL/2.0/
[document]: https://crdts.cloud/docs/index.html
[roadmap]: https://github.com/toeverything/OctoBase/issues/9
[fossil]: https://www2.fossil-scm.org/home/doc/trunk/www/index.wiki
[sqlite]: https://sqlite.org/index.html
[contributor license agreement]: https://github.com/toeverything/octobase/edit/master/.github/CLA.md
