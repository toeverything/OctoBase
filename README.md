# JWSTBase

<a href="https://join.slack.com/t/blocksuitedev/shared_invite/zt-1h0zz3b8z-nFpWSu6a6~yId7PxiMcBHA">
  <img src="https://img.shields.io/badge/-Slack-grey?logo=slack">
</a>
<a href="https://twitter.com/BlockSuiteDev">
  <img src="https://img.shields.io/badge/-Twitter-grey?logo=twitter">
</a>

[![stars](https://img.shields.io/github/stars/toeverything/JWSTBase.svg?style=flat&logo=github&colorB=red&label=stars)](https://github.com/toeverything/JWSTBase)

JWST is an offline-available, scalable, self-contained collaborative database, which was originally designed for AFFiNE. AFFiNE is a local-first open source knowledge base that provides full functionality in any network environment.

Based on JWST, you can not only implement a rich text editor for offline writing, but also implement richer offline collaboration functions based on JWST's data abstraction, such as: multidimensional tables, drawing boards, chat software, etc.

As an offline collaborative data database, JWST has the following characteristics:

📚 Offline collaboration, Schemaless, structured/unstructured/rich text data storage available across terminals.

🗃️ Binary storage that supports data deduplication and rich media editing.

🔍 High-performance real-time full-text indexing, high-quality multilingual word segmentation support.

🌐 Point-to-point/central server synchronization support, rich multi-platform native support.

🔒 Fine-grained permission control, advanced permission management.

By providing native offline collaboration, full-text indexing, and binary storage, JWST enables you to easily build secure, high-performance local-first collaborative applications using the same set of data abstractions on multiple platforms.

JWST can be used either as a stand-alone server database, or directly included in your application as an embedded database and remain fully functional.

Open [RoadMap](https://github.com/toeverything/JWST/issues/9), know to the future of JWST

Open [Document](https://crdts.cloud/docs/index.html), know how to use JWST

## Project Overview

```shell
├── apps
│   ├── android ##  Android scaffolding project
│   ├── frontend ## jwst playground, landingpage
│   ├── handbook ## jwst docs
│   ├── cloud ## affine-cloud backend
│   └── keck ## collaboration backend
└── libs ##
    ├── jwst  ## jwst core library
    ├── jwst-ffi ## jwst binging for C ffi
    ├── jwst-jni# ## jwst binding for JNI
    ├── jwst-wasm ## jwst binding for WASM
    ├── logger ## logger plugins for jwst
    ├── storage ## multiple platform storage plugins for jwst
    └── yrs ## rust implements y-protocol
```

## License

[MPL 2.0](./LICENSE)
