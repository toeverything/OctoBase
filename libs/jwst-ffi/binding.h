
#ifndef JWST_FFI_H
#define JWST_FFI_H
typedef struct JWSTWorkspace {} JWSTWorkspace;
typedef struct JWSTBlock {} JWSTBlock;
typedef struct YTransaction {} YTransaction;        


#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

constexpr static const int8_t BLOCK_TAG_NUM = 1;

constexpr static const int8_t BLOCK_TAG_INT = 2;

constexpr static const int8_t BLOCK_TAG_BOOL = 3;

constexpr static const int8_t BLOCK_TAG_STR = 4;

struct BlockChildren {
  uintptr_t len;
  char **data;
};

union BlockValue {
  double num;
  int64_t int;
  bool bool;
  char *str;
};

struct BlockContent {
  int8_t tag;
  BlockValue value;
};

extern "C" {

JWSTBlock *block_new(const JWSTWorkspace *workspace,
                     YTransaction *trx,
                     const char *block_id,
                     const char *flavor,
                     uint64_t operator_);

void block_destroy(JWSTBlock *block);

uint64_t block_get_created(const JWSTBlock *block);

uint64_t block_get_updated(const JWSTBlock *block);

char *block_get_flavor(const JWSTBlock *block);

BlockChildren *block_get_children(const JWSTBlock *block);

void block_children_destroy(BlockChildren *children);

BlockContent *block_get_content(const JWSTBlock *block, const char *key);

void block_set_content(JWSTBlock *block, const char *key, YTransaction *trx, BlockContent content);

void block_content_destroy(BlockContent *content);

JWSTWorkspace *workspace_new(const char *id);

void workspace_destroy(JWSTWorkspace *workspace);

JWSTBlock *workspace_get_block(const JWSTWorkspace *workspace, const char *block_id);

JWSTBlock *workspace_create_block(const JWSTWorkspace *workspace,
                                  const char *block_id,
                                  const char *flavor);

bool workspace_remove_block(const JWSTWorkspace *workspace, const char *block_id);

bool workspace_exists_block(const JWSTWorkspace *workspace, const char *block_id);

YTransaction *workspace_get_trx(JWSTWorkspace *workspace);

Subscription<UpdateEvent> *workspace_observe(JWSTWorkspace *workspace,
                                             void *env,
                                             void (*func)(void*, const YTransaction*, const UpdateEvent*));

void workspace_unobserve(Subscription<UpdateEvent> *subscription);

} // extern "C"


#endif            
