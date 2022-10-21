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

Block *block_new(const Workspace *workspace,
                 Transaction *trx,
                 const char *block_id,
                 const char *flavor,
                 uint64_t operator_);

void block_destroy(Block *block);

uint64_t block_get_created(const Block *block);

uint64_t block_get_updated(const Block *block);

char *block_get_flavor(const Block *block);

BlockChildren *block_get_children(const Block *block);

void block_children_destroy(BlockChildren *children);

BlockContent *block_get_content(const Block *block, const char *key);

void block_set_content(Block *block, const char *key, Transaction *trx, BlockContent content);

void block_content_destroy(BlockContent *content);

Workspace *workspace_new(const char *id);

void workspace_destroy(Workspace *workspace);

Block *workspace_get_block(const Workspace *workspace, const char *block_id);

Block *workspace_create_block(const Workspace *workspace, const char *block_id, const char *flavor);

bool workspace_remove_block(const Workspace *workspace, const char *block_id);

bool workspace_exists_block(const Workspace *workspace, const char *block_id);

Transaction *workspace_get_trx(Workspace *workspace);

Subscription<UpdateEvent> *workspace_observe(Workspace *workspace,
                                             void *env,
                                             void (*func)(void*, const Transaction*, const UpdateEvent*));

void workspace_unobserve(Subscription<UpdateEvent> *subscription);

} // extern "C"
