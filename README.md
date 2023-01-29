# OctoBase


[![Issues Closed](https://img.shields.io/github/issues-closed/toeverything/OctoBase?color=6880ff)](https://github.com/toeverything/blocksuite/issues?q=is%3Aissue+is%3Aclosed)
[![Join Telegram](https://img.shields.io/badge/join-telegram-blue)](https://t.me/blocksuite)
<a href="https://twitter.com/BlockSuiteDev">
  <img src="https://img.shields.io/badge/-Twitter-grey?logo=twitter">
</a>

[![stars](https://img.shields.io/github/stars/toeverything/OctoBase.svg?style=flat&logo=github&colorB=red&label=stars)](https://github.com/toeverything/OctoBase)

OctoBase is an offline-available, scalable, self-contained collaborative database, which was originally designed for AFFiNE. AFFiNE is a local-first open source knowledge base that provides full functionality in any network environment.

Based on OctoBase, you can not only implement a rich text editor for offline writing, but also implement richer offline collaboration functions based on OctoBase's data abstraction, such as: multidimensional tables, drawing boards, chat software, etc.

As an offline collaborative data database, OctoBase has the following characteristics:

- 📚 **Multi-platform available** offline collaboration, Schemaless, structured/unstructured/rich text data storage .

- 🗃️ **Binary storage** that supports data deduplication and rich media editing.

- 🔍 **High-performance real-time full-text indexing** with high-quality multilingual word segmentation support.

- 🌐 **Point-to-point / central server synchronization** with rich multi-platform native support.

- 🔒 **Fine-grained permission control** with advanced permission management.

By providing native offline collaboration, full-text indexing, and binary storage, OctoBase enables you to easily build secure, high-performance local-first collaborative applications using the same set of data abstractions on multiple platforms.

OctoBase can be used either as a stand-alone server database, or directly included in your application as an embedded database and remain fully functional.

Open [RoadMap](https://github.com/toeverything/OctoBase/issues/9), know to the future of OctoBase

Open [Document](https://crdts.cloud/docs/index.html), know how to use OctoBase

## Project Overview

```shell
├── apps
│   ├── android ##  Android scaffolding project
│   ├── frontend ## OctoBase playground, landingpage
│   ├── handbook ## OctoBase docs
│   ├── cloud ## affine-cloud backend
│   └── keck ## collaboration backend
└── libs ##
    ├── jwst  ## OctoBase core library
    ├── jwst-ffi ## OctoBase binging for C ffi
    ├── jwst-jni# ## OctoBase binding for JNI
    ├── jwst-wasm ## OctoBase binding for WASM
    ├── logger ## logger plugins for OctoBase
    ├── storage ## multiple platform storage plugins for OctoBase
    └── yrs ## rust implements y-protocol
```

## License

[MPL 2.0](./LICENSE)
