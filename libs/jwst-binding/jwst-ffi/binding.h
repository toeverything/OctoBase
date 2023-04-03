
#ifndef JWST_FFI_H
#define JWST_FFI_H
typedef struct JWSTWorkspace {} JWSTWorkspace;
typedef struct JWSTBlock {} JWSTBlock;
typedef struct YTransaction {} YTransaction;


#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define BLOCK_TAG_NUM 1

#define BLOCK_TAG_INT 2

#define BLOCK_TAG_BOOL 3

#define BLOCK_TAG_STR 4

typedef struct BlockChildren {
  uintptr_t len;
  char **data;
} BlockChildren;

typedef union BlockValue {
  double num;
  int64_t int;
  bool bool;
  char *str;
} BlockValue;

typedef struct BlockContent {
  int8_t tag;
  union BlockValue value;
} BlockContent;

JWSTBlock *block_new(const JWSTWorkspace *workspace,
                     const char *block_id,
                     const char *flavour,
                     uint64_t operator_);

void block_destroy(JWSTBlock *block);

uint64_t block_get_created(const JWSTBlock *block);

uint64_t block_get_updated(const JWSTBlock *block);

char *block_get_flavour(const JWSTBlock *block);

struct BlockChildren *block_get_children(const JWSTBlock *block);

void block_push_children(const JWSTBlock *block, YTransaction *trx, const JWSTBlock *child);

void block_insert_children_at(const JWSTBlock *block,
                              YTransaction *trx,
                              const JWSTBlock *child,
                              uint32_t pos);

void block_insert_children_before(const JWSTBlock *block,
                                  YTransaction *trx,
                                  const JWSTBlock *child,
                                  const char *reference);

void block_insert_children_after(const JWSTBlock *block,
                                 YTransaction *trx,
                                 const JWSTBlock *child,
                                 const char *reference);

void block_children_destroy(struct BlockChildren *children);

struct BlockContent *block_get_content(const JWSTBlock *block, const char *key);

void block_set_content(JWSTBlock *block,
                       const char *key,
                       YTransaction *trx,
                       struct BlockContent content);

void block_content_destroy(struct BlockContent *content);

JWSTWorkspace *workspace_new(const char *id);

void workspace_destroy(JWSTWorkspace *workspace);

JWSTBlock *workspace_get_block(const JWSTWorkspace *workspace, const char *block_id);

JWSTBlock *workspace_create_block(const JWSTWorkspace *workspace,
                                  const char *block_id,
                                  const char *flavour);

bool workspace_remove_block(const JWSTWorkspace *workspace, const char *block_id);

bool workspace_exists_block(const JWSTWorkspace *workspace, const char *block_id);

void trx_commit(YTransaction *trx);

Subscription<UpdateEvent> *workspace_observe(JWSTWorkspace *workspace,
                                             void *env,
                                             void (*func)(void*, const YTransaction*, const UpdateEvent*));

void workspace_unobserve(Subscription<UpdateEvent> *subscription);


#endif            
