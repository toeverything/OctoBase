# What's New

## 0.0.1-230326

-   [+] Added configuration option for adjustable JWT token expiration
-   [+] Assigned flavor when creating blocks
-   [+] Added block ID for serialized block
-   [+] Added request ID for better error observability
-   [*] Split API modules and added test for Affine-Cloud
-   [*] Documented health check and added new server guide

## 0.0.1-230314

-   [+] Implemented custom field indexes for local search
-   [+] Added support for searching and creating blocks by flavor
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
