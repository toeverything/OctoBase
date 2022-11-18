# JWSTBase

<a href="https://join.slack.com/t/blocksuitedev/shared_invite/zt-1h0zz3b8z-nFpWSu6a6~yId7PxiMcBHA">
  <img src="https://img.shields.io/badge/-Slack-grey?logo=slack">
</a>
<a href="https://twitter.com/BlockSuiteDev">
  <img src="https://img.shields.io/badge/-Twitter-grey?logo=twitter">
</a>

[![stars](https://img.shields.io/github/stars/toeverything/JWSTBase.svg?style=flat&logo=github&colorB=red&label=stars)](https://github.com/toeverything/JWSTBase)

an open source Firebase alternative. Start your project with a collaborative database, Authentication, instant APIs, Realtime subscriptions, and Storage.

-   MultiPlatform CRDT collaborative database
-   instant APIs
-   Authentication
-   Storage
-   ...

open [Roadmap](https://github.com/toeverything/JWST/issues/9), know to the furture of JWST

## MultiPlatform CRDT collaborative database

JWST is a set of collaborative backend and database implemented based on CRDTs.
It is the core to resolve conflicts between the duplication of data and manage the databases so that real-time collaboration and local-first storage is possible.
It supports local storage and serve-side storage.

https://crdts.cloud/docs/index.html

## AFFiNE-Cloud backend

[keck docs](./apps/keck)

## JWSTBase landing page

[JWSTBase landing page docs](./apps/frontend/README.md)

## Project Overview

```shell
├── apps
│   ├── android ##  Android scaffolding project
│   ├── frontend ## jwst playground, landingpage
│   ├── handbook ## jwst docs
│   └── keck ## affine-cloud backend
└── libs ##
    ├── jwst  ## jwst core library
    ├── jwst-ffi ## A foreign function interface (FFI) is a mechanism by which a program written in one programming language can call routines or make use of services written in another.
    ├── jwst-jni# ##  the Java Native Interface (JNI) is a foreign function interface programming framework that enables Java code running in a Java virtual
    ├── jwst-wasm ## a binary instruction format for a stack-based virtual machine
    ├── logger
    └── yrs ## rust implements y-protocal
```

## License

[MPL 2.0](./LICENSE)
