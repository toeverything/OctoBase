# What's New

## 0.5.1-canary.1-230504

-   [+] feat: assemble runtime crdt state (#408)
-   [+] feat: add workspace avatar (#365)
-   [+] feat: add var buf & var string writer (#396)
-   [+] feat: subscribing synchronized blocks from collaboration server (#397)
-   [*] fix: block subscribing skipped caused by failing to update `observed_blocks` (#410)
-   [*] fix: cannot subscribe blocks with get_blocks_by_flavour (#406)
-   [*] fix: cannot save to local storage of workspace synchronized from collaboration server (#394)

## 0.5.1-canary.0-230419

-   [+] feat: expose method for manually retrieval of modified blocks
-   [+] feat: optimize check_shared
-   [+] feat: new ybinary parser
-   [+] feat: jni binding for block level observation
-   [+] feat: block level observation
-   [+] feat: support page share expire time
-   [*] fix: support blocksuite specific behavier
-   [*] chore: add collaboration doc and best practice
-   [*] chore: test using ymap from different source with same id
-   [*] test: add update merge test
-   [*] fix: awareness subscription memory leak

## 0.5.0-230410

-   [+] feat: enable coverage test & more stable stress test
-   [+] feat: clone nested data in doc (#359)
-   [+] feat: single page sharing permission check (#361)
-   [*] fix: unable to start when missing env file (#353)
-   [*] fix: check permissions of blob API (#354)
-   [*] chore: move hosting feature to cloud infra crate (#357)
-   [*] refactor: add more concrete error types eliminating anyhow::Error in libs crate (#358)

## 0.0.1-230403

-   [+] Add support for extract specific spaces from workspace
-   [+] Add owned subscription in workspace
-   [+] Add debug user for collaboration test
-   [+] Add new homepage and documentation
-   [+] Exposed `get_blocks_by_flavour()` to Android
-   [*] Refactor websocket authentication
-   [*] Exclude sensitive data in logs
-   [*] Improve collaboration test

## 0.0.1-230326

-   [+] Added configuration option for adjustable JWT token expiration
-   [+] Assigned flavour when creating blocks
-   [+] Added block ID for serialized block
-   [+] Added request ID for better error observability
-   [*] Split API modules and added test for Affine-Cloud
-   [*] Documented health check and added new server guide

## 0.0.1-230314

-   [+] Implemented custom field indexes for local search
-   [+] Added support for searching and creating blocks by flavour
-   [+] Added runtime version printing
-   [+] Switched search tokenizer to ngram
-   [+] Added flexible environment reading
-   [*] Enabled garbage collection feature
-   [*] Optimized user login and key context
-   [*] Refactored message broadcast
-   [*] Improved logger and documentation
-   [*] Split cloud modules and optimized configuration
-   [*] Fixed SQLite sub query syntax error

## 0.0.1-230306

-   [+] Added api documentation to affine-cloud
-   [+] Exposed `get_blocks_by_flavour()` to Swift binding
-   [+] Added swift storage binding and exposed `get_blocks_by_flavour()` to Swift binding
-   [*] Migrated jwt to `pnpm workspace` & `vite`
-   [*] Switched search tokenizer to ngram and added jni search binding
-   [*] Adapted to new storage api
-   [*] Fixed deadlock when multithreading r/w map
-   [*] Fixed collaboration cannot connect

## 0.0.1-230228

-   [+] Added support for Swift binding
-   [+] Added ORM support in keck server
-   [+] Added timeout configuration for keck provider
-   [*] Implemented connection level rate limiter
-   [*] Extracted sync module
-   [*] Extracted mail module
-   [*] Fixed bug with viewing shared workspace
-   [*] Removed printing colors on server side
-   [*] Refactored sync threads and added Jni auto-reconnect
-   [*] Improved error handling, workspace caching, and Jni binding for new yrs
-   [*] Removed database name concatenate
