/// `maca_runtime.h`: declarations + the array macro used by generated code.
pub const RUNTIME_H: &str = r##"#ifndef MACA_RUNTIME_H
#define MACA_RUNTIME_H
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <stdarg.h>

typedef const char* maca_str;

/* ---- allocator: size-tracked, reference-counted blocks with a free-list for
   reuse and a registry drained at exit.

   `maca_alloc` hands back a block with one owner. `maca_dup` adds an owner,
   `maca_drop` removes one, and the block returns to the free-list when the last
   owner lets go, which is where the next same-size request picks it up rather
   than going to malloc. That is the reuse Perceus is named for: the code generator
   inserts the dup/drop calls (see `owned_locals` in the C backend), so a loop
   that builds and discards a value reuses one buffer instead of asking the
   allocator for a new one every time round.

   Blocks are also registered, and the registry is drained at exit, so a program
   that ends while still holding memory is valgrind-clean. ---- */
void maca_init(void);          /* installs atexit(shutdown); call first in main */
void maca_shutdown(void);      /* frees every live block */
void* maca_alloc(size_t n);
void* maca_realloc(void* p, size_t n);
/* Release a string. A `maca_str` is the payload pointer itself rather than a
   struct with a buffer in it, and it is `const` because nothing may write
   through it; neither changes who is allowed to let go of the bytes. */
void maca_drop_str(maca_str s);
/* A fresh copy of `s`.
   Every `maca_str`-returning function in here promises the same thing: what
   comes back is a block this allocator handed out, or a static literal, and
   never one of the arguments. That promise is what lets the back end release a string
   it built without first proving where the bytes came from, and this is how the
   ones with nothing to do keep it: the shortcuts that used to return an
   argument unchanged (`replace` with an empty needle, `pad` of an already-wide
   string, `split` on an empty separator) were free until the caller was allowed
   to let go of the result. */
maca_str maca_str_copy(maca_str s);
void maca_dup(void* p);        /* one more owner */
void maca_drop(void* p);       /* one fewer owner; at zero, back to the free-list */
uint64_t maca_alloc_count(void);
uint64_t maca_reuse_count(void);

/* ---- minconsole (syslog levels; <= warn to stderr) ---- */
void maca_emerg(maca_str s);
void maca_alert(maca_str s);
void maca_crit(maca_str s);
void maca_err(maca_str s);
void maca_warn(maca_str s);
void maca_notice(maca_str s);
void maca_info(maca_str s);
void maca_debug(maca_str s);
/* `fail msg`: longjmp to the nearest `try` handler, else print "error: <msg>"
   to stderr and exit(1) (an unhandled failure). */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noreturn))
#endif
void maca_fail(maca_str s);
/* try/catch scaffolding: push returns a jmp_buf for the caller to `setjmp` on;
   `maca_fail` longjmps to the top handler; `maca_last_fail` is the caught msg. */
jmp_buf* maca_try_push(void);
void maca_try_pop(void);
maca_str maca_last_fail(void);
void maca_print(maca_str s);
maca_str maca_input(void);

/* ---- strings ---- */
maca_str maca_concat(maca_str a, maca_str b);
/* Concatenate `n` strings in one allocation.
   `a ++ b ++ c` used to build `a ++ b`, copy it into the next result and
   abandon it; the intermediate was invisible in the source, so nothing ever
   released it. Written as one call there is no intermediate to release. */
maca_str maca_concat_n(size_t n, ...);
maca_str maca_from_int(int64_t n);
maca_str maca_from_float(double d);
maca_str maca_from_bool(bool b);
bool maca_str_eq(maca_str a, maca_str b);
/* Byte order, the same order `maca_sort_str` puts a list in, so a `sort_by`
   with a `str` key agrees with a plain `sort`. */
int maca_str_cmp(maca_str a, maca_str b);
maca_str maca_join(maca_str* data, int64_t len, maca_str sep);
maca_str maca_str_at(maca_str s, int64_t i); /* single-char str at byte i ("" if OOB) */
maca_str maca_chr(int64_t b);               /* the one-byte string holding b   */
int64_t  maca_ord(maca_str s);              /* the value of s's first byte, -1 for "" */
int64_t maca_strlen(maca_str s);              /* byte length (0 if NULL) */
/* character classes: inspect the first byte of a 1-char str (false if empty) */
bool maca_is_space(maca_str c);
bool maca_is_digit(maca_str c);
bool maca_is_alpha(maca_str c);

/* ---- string stdlib (UFCS methods on `str`) ---- */
maca_str maca_trim(maca_str s);                       /* strip leading/trailing ASCII space */
maca_str maca_upper(maca_str s);
maca_str maca_lower(maca_str s);
bool maca_contains(maca_str s, maca_str sub);
bool maca_starts_with(maca_str s, maca_str prefix);
bool maca_ends_with(maca_str s, maca_str suffix);
maca_str maca_replace(maca_str s, maca_str from, maca_str to); /* all occurrences */
maca_str maca_repeat(maca_str s, int64_t n);          /* s concatenated n times */

/* ---- file I/O ---- */
maca_str maca_read_file(maca_str path);               /* whole file; "" unless it is a readable ordinary file */
bool maca_write_file(maca_str path, maca_str text);   /* truncate + write; ok? */
bool maca_file_exists(maca_str path);
maca_str maca_real_path(maca_str path);     /* symlinks resolved; "" if absent */
/* Is standard output a terminal?
   The one thing a program cannot work out for itself, and the whole basis of
   deciding whether to colour: the same command is read by a person and piped
   into a file, and only the first wants escape codes. */
bool maca_is_tty(void);
bool maca_is_dir(maca_str path);                      /* a directory, not a file? */
int64_t maca_file_size(maca_str path);                /* bytes, or -1 */
int64_t maca_modified_ms(maca_str path);              /* mtime in ms, or -1 */
bool maca_remove_file(maca_str path);                 /* unlink; ok? */
bool maca_remove_dir(maca_str path);                  /* recursive rmdir; ok? */
bool maca_make_dir(maca_str path);                    /* mkdir -p; ok? */
bool maca_copy_bytes(maca_str src, maca_str dst);     /* byte-for-byte copy; ok? */
maca_str maca_read_line(void);                        /* one stdin line, no \n; "" at EOF */
bool maca_at_eof(void);                               /* stdin exhausted? */
maca_str maca_read_stdin(void);                       /* all of stdin */
int64_t maca_now_ms(void);                            /* ms since the Unix epoch */
maca_str maca_now_iso(void);                          /* UTC, "YYYY-MM-DDTHH:MM:SSZ" */
maca_str maca_format_time(int64_t ms, maca_str fmt);  /* strftime over UTC */
maca_str* maca_list_dir(maca_str path, int64_t* out_len); /* names, sorted; malloc'd */

/* ---- processes ---- */
/* Run `cmd` with `argv[0..n)` as its arguments, no shell in between: an
   argument holding a space, a quote or a `$` is one argument, not three. */
int64_t maca_exec(maca_str cmd, maca_str* argv, int64_t n);    /* exit code, or -1 */
maca_str maca_capture(maca_str cmd, maca_str* argv, int64_t n); /* its stdout */
maca_str maca_env(maca_str name);                     /* "" when unset */
maca_str maca_cwd(void);                              /* the working directory */
bool maca_chdir(maca_str path);                       /* change it; ok? */
maca_str maca_pad_start(maca_str s, int64_t w, maca_str p); /* left-pad to width w */
maca_str maca_pad_end(maca_str s, int64_t w, maca_str p);   /* right-pad to width w */
maca_str maca_pad_center(maca_str s, int64_t w, maca_str p); /* centre within width w */
maca_str maca_attr(maca_str name, maca_str value);  /* ` name="escaped"` */
maca_str maca_flag(maca_str name, bool on);         /* ` name` when on, else "" */
maca_str maca_element(maca_str tag, maca_str attrs, maca_str kids); /* tag chosen at runtime */
maca_str maca_fixed(double x, int64_t n);             /* x with n decimal places */
maca_str maca_substr(maca_str s, int64_t start, int64_t len);  /* byte range, clamped */
maca_str maca_str_slice(maca_str s, int64_t from, int64_t to); /* exclusive end */
bool maca_assert(bool cond, maca_str msg);                     /* report + remember */
bool maca_assert_eq(maca_str got, maca_str want, maca_str msg);
int64_t maca_failures(void);        /* how many assertions have failed */
int64_t maca_index_of(maca_str s, maca_str sub);      /* byte index, or -1 */
maca_str* maca_split(maca_str s, maca_str sep, int64_t* out_len); /* malloc'd maca_str[] */

/* ---- closures ----
 * A first-class function value: a code pointer plus a heap environment holding
 * the captured variables. All lambdas lower to this uniform representation, so
 * capturing and non-capturing lambdas (and higher-order stdlib methods like
 * `.map`/`.filter`/`.reduce`) share one ABI. Arguments and results cross the
 * boundary boxed as int64_t (a str/pointer fits; the caller casts back). */
typedef struct { int64_t (*fn)(void*, int64_t); void* env; } maca_closure;
typedef struct { int64_t (*fn)(void*, int64_t, int64_t); void* env; } maca_closure2;
static inline int64_t maca_call1(maca_closure c, int64_t x) { return c.fn(c.env, x); }
static inline int64_t maca_call2(maca_closure2 c, int64_t a, int64_t b) { return c.fn(c.env, a, b); }
/* box/unbox a double across the int64 closure boundary (bit-preserving) */
static inline int64_t maca_box_f64(double d) { int64_t r; memcpy(&r, &d, sizeof(r)); return r; }
static inline double maca_unbox_f64(int64_t i) { double d; memcpy(&d, &i, sizeof(d)); return d; }

/* in-place ascending sort helpers used by `xs.sort()` */
void maca_sort_i64(int64_t* data, int64_t n);
void maca_sort_f64(double* data, int64_t n);
void maca_sort_str(maca_str* data, int64_t n);
uint64_t maca_hash_str(maca_str s);                   /* FNV-1a, for map keys */

/* ---- growable string builder ---- */
typedef struct { char* buf; size_t len; size_t cap; } maca_sb;
void maca_sb_init(maca_sb* sb);
void maca_sb_putc(maca_sb* sb, char c);
void maca_sb_puts(maca_sb* sb, const char* s);
void maca_sb_put_json_str(maca_sb* sb, maca_str s); /* quoted + escaped */
maca_str maca_sb_finish(maca_sb* sb);

/* ---- paths & files ---- */
maca_str maca_dirs_data(void);
maca_str maca_path_join(maca_str a, maca_str b);
bool maca_path_exists(maca_str p);
maca_str maca_read(maca_str p);                 /* aborts on failure */
void maca_write(maca_str p, maca_str content);  /* mkdir -p parent; aborts on failure */

/* ---- JSON value tree ---- */
typedef enum { MJ_NULL, MJ_BOOL, MJ_NUM, MJ_STR, MJ_ARR, MJ_OBJ } maca_json_kind;
typedef struct maca_json maca_json;
struct maca_json {
    maca_json_kind kind;
    bool b;
    double num;
    char* str;
    struct { maca_json** items; int64_t len; } arr;
    struct { char** keys; maca_json** vals; int64_t len; } obj;
};
maca_json* maca_json_parse(maca_str text);      /* aborts on parse error */
maca_json* maca_json_get(maca_json* o, const char* key); /* NULL if absent */
/* The two readers a typed `json.decode` uses. Where the untyped accessors
   below answer with a zero, these `fail` with a message that names the field
   and says what the declared type needed there, because a decode that quietly
   returns zeros is a wrong answer with nothing to say for itself. */
maca_json* maca_json_want(maca_json* o, maca_str field, maca_json_kind kind);
maca_json* maca_json_object(maca_json* j, maca_str type);
/* `s` as a JSON string literal, quotes and escapes included. */
maca_str maca_json_quote(maca_str s);
int64_t maca_json_int(maca_json* j);
double maca_json_float(maca_json* j);
bool maca_json_bool(maca_json* j);
maca_str maca_json_str(maca_json* j);

/* ---- typed dynamic array (monomorphized by generated code) ---- */
/* The array struct alone: only needs `Elem` forward-declared (it stores an
   `Elem*`). Split out so a self-referential record (`Expr { children: Expr[] }`)
   can declare its element array before the record body closes the cycle. */
#define MACA_ARRAY_STRUCT(Name, Elem)                                          \
    typedef struct { Elem* data; int64_t len; int64_t cap; } Name;
/* The array operations: need `Elem` complete (they use `sizeof(Elem)`), so a
   recursive record emits these only after its struct body is defined. */
#define MACA_ARRAY_OPS(Name, Elem)                                             \
    static inline Name Name##_new(void) { Name a; a.data = NULL; a.len = 0;     \
        a.cap = 0; return a; }                                                 \
    static inline void Name##_push(Name* a, Elem x) {                          \
        if (a->len == a->cap) {                                                \
            a->cap = a->cap ? a->cap * 2 : 4;                                  \
            a->data = (Elem*)maca_realloc(a->data, (size_t)a->cap * sizeof(Elem)); \
            if (!a->data) { abort(); }                                         \
        }                                                                      \
        a->data[a->len++] = x;                                                 \
    }                                                                          \
    static inline Name Name##_concat(Name a, Name b) {                         \
        Name r = Name##_new();                                                 \
        for (int64_t i = 0; i < a.len; i++) Name##_push(&r, a.data[i]);        \
        for (int64_t i = 0; i < b.len; i++) Name##_push(&r, b.data[i]);        \
        return r;                                                              \
    }                                                                          \
    static inline Name Name##_slice(Name a, int64_t from) {                    \
        Name r = Name##_new();                                                 \
        for (int64_t i = from; i < a.len; i++) Name##_push(&r, a.data[i]);     \
        return r;                                                              \
    }                                                                          \
    /* An index the list does not have leaves it alone, which is what `get`    \
       and `slice` already do: one out-of-range rule for the whole family. */  \
    static inline void Name##_put(Name* a, int64_t at, Elem x) {               \
        if (at >= 0 && at < a->len) a->data[at] = x;                           \
    }                                                                          \
    static inline void Name##_insert(Name* a, int64_t at, Elem x) {            \
        if (at < 0) at = 0;                                                    \
        if (at > a->len) at = a->len;                                          \
        Name##_push(a, x);                                                     \
        for (int64_t i = a->len - 1; i > at; i--) a->data[i] = a->data[i - 1]; \
        a->data[at] = x;                                                       \
    }                                                                          \
    static inline void Name##_erase(Name* a, int64_t at) {                     \
        if (at < 0 || at >= a->len) return;                                    \
        for (int64_t i = at; i + 1 < a->len; i++) a->data[i] = a->data[i + 1]; \
        a->len--;                                                              \
    }

/* A string-keyed hash map, monomorphized on its value type exactly as arrays
 * are on their element type.
 *
 * Keys are `str` and only `str`. That is the whole design decision: one key
 * type means one hash and one comparison, an integer key is `str(n)` away, and
 * the alternative is a second type parameter threaded through every backend for
 * a case the language has not needed. Open addressing with linear probing and
 * backward-shift deletion; grows at 70% load. `_keys` writes into a caller
 * buffer of `_len` entries and sorts them, so iteration order is deterministic,
 * so a generator that walks a map twice produces the same file twice. */
#define MACA_DEFINE_MAP(Name, Val)                                             \
    typedef struct { maca_str* keys; Val* vals; unsigned char* used;           \
                     int64_t len; int64_t cap; } Name;                         \
    static inline Name Name##_new(void) {                                      \
        Name m; m.keys = NULL; m.vals = NULL; m.used = NULL;                   \
        m.len = 0; m.cap = 0; return m;                                        \
    }                                                                          \
    static inline int64_t Name##_slot(const Name* m, maca_str k) {             \
        int64_t i = (int64_t)(maca_hash_str(k) & (uint64_t)(m->cap - 1));      \
        while (m->used[i] && strcmp(m->keys[i], k) != 0)                       \
            i = (i + 1) & (m->cap - 1);                                        \
        return i;                                                              \
    }                                                                          \
    static inline void Name##_grow(Name* m) {                                  \
        int64_t oc = m->cap;                                                   \
        maca_str* ok = m->keys; Val* ov = m->vals; unsigned char* ou = m->used;\
        m->cap = oc ? oc * 2 : 8;                                              \
        m->keys = (maca_str*)maca_alloc((size_t)m->cap * sizeof(maca_str));       \
        m->vals = (Val*)maca_alloc((size_t)m->cap * sizeof(Val));                 \
        m->used = (unsigned char*)maca_alloc((size_t)m->cap);                     \
        memset(m->used, 0, (size_t)m->cap);                                    \
        m->len = 0;                                                            \
        for (int64_t i = 0; i < oc; i++) if (ou[i]) {                          \
            int64_t j = Name##_slot(m, ok[i]);                                 \
            m->keys[j] = ok[i]; m->vals[j] = ov[i];                            \
            m->used[j] = 1; m->len++;                                          \
        }                                                                      \
    }                                                                          \
    static inline void Name##_set(Name* m, maca_str k, Val v) {                \
        if (!k) k = "";                                                        \
        if (m->cap == 0 || (m->len + 1) * 10 >= m->cap * 7) Name##_grow(m);    \
        int64_t i = Name##_slot(m, k);                                         \
        if (!m->used[i]) { m->used[i] = 1; m->keys[i] = k; m->len++; }         \
        m->vals[i] = v;                                                        \
    }                                                                          \
    static inline bool Name##_has(Name m, maca_str k) {                        \
        if (m.cap == 0 || !k) return false;                                    \
        return m.used[Name##_slot(&m, k)] != 0;                                \
    }                                                                          \
    static inline Val Name##_get(Name m, maca_str k, Val dflt) {               \
        if (m.cap == 0 || !k) return dflt;                                     \
        int64_t i = Name##_slot(&m, k);                                        \
        return m.used[i] ? m.vals[i] : dflt;                                   \
    }                                                                          \
    static inline bool Name##_remove(Name* m, maca_str k) {                    \
        if (m->cap == 0 || !k) return false;                                   \
        int64_t i = Name##_slot(m, k);                                         \
        if (!m->used[i]) return false;                                         \
        m->used[i] = 0; m->len--;                                              \
        /* re-seat the run after the hole, or a probe would stop short */      \
        int64_t j = (i + 1) & (m->cap - 1);                                    \
        while (m->used[j]) {                                                   \
            maca_str rk = m->keys[j]; Val rv = m->vals[j];                     \
            m->used[j] = 0; m->len--;                                          \
            int64_t s = Name##_slot(m, rk);                                    \
            m->keys[s] = rk; m->vals[s] = rv; m->used[s] = 1; m->len++;        \
            j = (j + 1) & (m->cap - 1);                                        \
        }                                                                      \
        return true;                                                           \
    }                                                                          \
    static inline int64_t Name##_len(Name m) { return m.len; }                 \
    static inline int64_t Name##_keys(Name m, maca_str* out) {                 \
        int64_t n = 0;                                                         \
        for (int64_t i = 0; i < m.cap; i++)                                    \
            if (m.used[i]) out[n++] = m.keys[i];                               \
        maca_sort_str(out, n);                                                 \
        return n;                                                              \
    }

/* The common case: a non-recursive element, struct + ops together. */
#define MACA_DEFINE_ARRAY(Name, Elem)                                          \
    MACA_ARRAY_STRUCT(Name, Elem)                                              \
    MACA_ARRAY_OPS(Name, Elem)

#endif
"##;

/// `maca_runtime.c`.
pub const RUNTIME_C: &str = r##"#define _GNU_SOURCE
#include "maca_runtime.h"
#include <stdio.h>
#include <ctype.h>
#include <pthread.h>
#include <limits.h>
#include <stdlib.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>
#include <dirent.h>
#include <time.h>
#include <sys/wait.h>

static void die(const char* msg) {
    fputs("maca runtime error: ", stderr);
    fputs(msg, stderr);
    fputc('\n', stderr);
    exit(1);
}

/* ---- allocator: header-tracked blocks + reuse free-list + exit drain ----

   Every one of these globals is reachable from more than one thread the moment
   a program says `spawn`, and `spawn` is the whole of Maca's concurrency: an
   HTTP server is a thread per connection and every string operation in a
   handler allocates. Unlocked, four concurrent tasks that only build strings
   were enough to produce `realloc(): invalid next size` and `double free or
   corruption`; a server at eight concurrent clients silently handed mangled
   request text to its handler, and at forty-eight it took a general protection
   fault inside libc.

   One mutex over the bookkeeping, not per-object locks: the critical sections
   are a free-list walk and an array push, both a handful of instructions, and a
   correct allocator that occasionally contends is worth more than a fast one
   that corrupts. The reference counts move under the same lock, because a
   non-atomic `rc--` racing itself is how a live object reaches the free list.
*/
typedef struct maca_hdr { size_t size; int64_t rc; struct maca_hdr* fl_next; } maca_hdr;
static maca_hdr** g_live = NULL;
static size_t g_live_len = 0, g_live_cap = 0;
static maca_hdr* g_freelist = NULL;
static uint64_t g_alloc_count = 0, g_reuse_count = 0;
static pthread_mutex_t g_heap_lock = PTHREAD_MUTEX_INITIALIZER;

/* Which payload addresses this allocator handed out.

   A `maca_str` is a `const char*` and says nothing about where it came from: a
   string literal lives in .rodata, `maca_concat` returns a heap block, and a
   Maca function returning `"x"` hands back the former where the latter is
   expected. Releasing one means reading a header that is only there for the
   latter, and reading eight bytes before a literal is at best a wrong answer.

   So membership is asked, not assumed. An open-addressed set of payload
   addresses, one pointer per block and never removed, because a block on the free
   list is still a block, and the set is exactly "this address has a header".
   That is an O(1) answer with nothing read out of bounds, which is what makes
   dropping a string safe at all. */
static void** g_blocks = NULL;
static size_t g_blocks_cap = 0, g_blocks_len = 0;
static int g_poison = -1; /* -1 not yet asked, then MACA_POISON */

static size_t addr_slot(void* p, size_t cap) {
    /* Fibonacci hashing: pointers are 16-byte aligned, so the low bits are
       constant and the raw value makes a poor index. */
    uint64_t h = (uint64_t)(uintptr_t)p >> 4;
    h *= 11400714819323198485ULL;
    return (size_t)(h >> 32) & (cap - 1);
}

static void blocks_insert(void* p);

static void blocks_grow(void) {
    size_t old_cap = g_blocks_cap;
    void** old = g_blocks;
    g_blocks_cap = old_cap ? old_cap * 2 : 1024;
    g_blocks = (void**)calloc(g_blocks_cap, sizeof(void*));
    if (!g_blocks) die("out of memory");
    g_blocks_len = 0;
    for (size_t i = 0; i < old_cap; i++) {
        if (old[i]) blocks_insert(old[i]);
    }
    free(old);
}

static void blocks_insert(void* p) {
    if (g_blocks_cap == 0 || (g_blocks_len + 1) * 4 >= g_blocks_cap * 3) blocks_grow();
    size_t i = addr_slot(p, g_blocks_cap);
    while (g_blocks[i]) {
        if (g_blocks[i] == p) return;
        i = (i + 1) & (g_blocks_cap - 1);
    }
    g_blocks[i] = p;
    g_blocks_len++;
}

/* Did this allocator hand out `p`? Called with the lock held. */
static int blocks_has(void* p) {
    if (!g_blocks_cap) return 0;
    size_t i = addr_slot(p, g_blocks_cap);
    while (g_blocks[i]) {
        if (g_blocks[i] == p) return 1;
        i = (i + 1) & (g_blocks_cap - 1);
    }
    return 0;
}

void maca_init(void) { atexit(maca_shutdown); }
void maca_shutdown(void) {
    pthread_mutex_lock(&g_heap_lock);
    for (size_t i = 0; i < g_live_len; i++) free(g_live[i]);
    free(g_live);
    free(g_blocks);
    g_live = NULL; g_live_len = 0; g_live_cap = 0; g_freelist = NULL;
    g_blocks = NULL; g_blocks_cap = 0; g_blocks_len = 0;
    pthread_mutex_unlock(&g_heap_lock);
}
void* maca_alloc(size_t n) {
    pthread_mutex_lock(&g_heap_lock);
    for (maca_hdr** pp = &g_freelist; *pp; pp = &(*pp)->fl_next) {
        maca_hdr* h = *pp;
        if (h->size >= n) {
            *pp = h->fl_next; h->fl_next = NULL; h->rc = 1;
            g_reuse_count++;
            pthread_mutex_unlock(&g_heap_lock);
            return (void*)(h + 1);
        }
    }
    maca_hdr* h = (maca_hdr*)malloc(sizeof(maca_hdr) + n);
    if (!h) { pthread_mutex_unlock(&g_heap_lock); die("out of memory"); }
    h->size = n; h->rc = 1; h->fl_next = NULL;
    if (g_live_len == g_live_cap) {
        g_live_cap = g_live_cap ? g_live_cap * 2 : 64;
        g_live = (maca_hdr**)realloc(g_live, g_live_cap * sizeof(maca_hdr*));
        if (!g_live) { pthread_mutex_unlock(&g_heap_lock); die("out of memory"); }
    }
    g_live[g_live_len++] = h;
    blocks_insert((void*)(h + 1));
    g_alloc_count++;
    pthread_mutex_unlock(&g_heap_lock);
    return (void*)(h + 1);
}
void maca_dup(void* p) {
    if (!p) return;
    pthread_mutex_lock(&g_heap_lock);
    if (blocks_has(p)) (((maca_hdr*)p) - 1)->rc++;
    pthread_mutex_unlock(&g_heap_lock);
}
void maca_drop(void* p) {
    if (!p) return;
    pthread_mutex_lock(&g_heap_lock);
    /* A string literal, a static "" a runtime function returned, a pointer
       into someone else's buffer: all reach here, and none of them is ours to
       release. Asking is cheap; guessing corrupts the free list. */
    if (blocks_has(p)) {
        maca_hdr* h = ((maca_hdr*)p) - 1;
        /* `rc <= 0` means the block is already on the free list. Pushing it a
           second time links it to itself and the next allocation walks a cycle
           forever, so an over-drop costs a block that is never reclaimed rather
           than an allocator that never returns. */
        if (h->rc > 0 && --h->rc == 0) {
            /* `MACA_POISON=1` overwrites a released block so that reading one
               after it was let go produces obvious garbage instead of the value
               that happened to still be sitting there. A use-after-free that
               only shows up as a correct answer is a bug the test suite cannot
               see, which is the whole reason for the switch. */
            if (g_poison < 0) {
                /* Set-but-off has to mean off. `MACA_POISON=0` reading as *on*
                   made every suite that looped over "0" and "1" run the
                   poisoned build twice and never test the ordinary one. */
                const char* v = getenv("MACA_POISON");
                g_poison = v && *v && strcmp(v, "0") != 0;
            }
            if (g_poison) memset((void*)(h + 1), 0xDD, h->size);
            h->fl_next = g_freelist; g_freelist = h;
        }
    }
    pthread_mutex_unlock(&g_heap_lock);
}
void* maca_realloc(void* p, size_t n) {
    if (!p) return maca_alloc(n);
    pthread_mutex_lock(&g_heap_lock);
    int ours = blocks_has(p);
    pthread_mutex_unlock(&g_heap_lock);
    /* Growing a buffer means copying `h->size` bytes out of it, and a pointer
       with no header of ours has no size to read, and guessing one reads whatever
       is eight bytes in front of somebody else's memory. */
    if (!ours) die("realloc of a pointer this allocator never handed out");
    maca_hdr* h = ((maca_hdr*)p) - 1;
    if (h->size >= n) return p;
    void* np = maca_alloc(n);
    memcpy(np, p, h->size);
    maca_drop(p);
    return np;
}
void maca_drop_str(maca_str s) { maca_drop((void*)(uintptr_t)s); }
uint64_t maca_alloc_count(void) { return g_alloc_count; }
uint64_t maca_reuse_count(void) { return g_reuse_count; }

static void* xmalloc(size_t n) { return maca_alloc(n); }

/* ---- console ---- */
static void line(FILE* f, const char* s) { fputs(s ? s : "", f); fputc('\n', f); }
void maca_emerg(maca_str s)  { line(stderr, s); }
void maca_alert(maca_str s)  { line(stderr, s); }
void maca_crit(maca_str s)   { line(stderr, s); }
void maca_err(maca_str s)    { line(stderr, s); }
void maca_warn(maca_str s)   { line(stderr, s); }
void maca_notice(maca_str s) { line(stdout, s); }
void maca_info(maca_str s)   { line(stdout, s); }
/* try/catch: a thread-local stack of setjmp handlers + the last caught msg */
static _Thread_local jmp_buf g_handlers[256];
static _Thread_local int g_handler_top = 0;
static _Thread_local maca_str g_last_fail = "";

jmp_buf* maca_try_push(void) {
    if (g_handler_top >= 256) die("try nesting too deep");
    return &g_handlers[g_handler_top++];
}
void maca_try_pop(void) { if (g_handler_top > 0) g_handler_top--; }
/* The message the last caught failure carried. Copied, because `fail e` was
   handed a string somebody else is holding and the caller of `try` is entitled
   to treat what it gets back as its own. */
maca_str maca_last_fail(void) { return maca_str_copy(g_last_fail); }

void maca_fail(maca_str s) {
    if (g_handler_top > 0) {
        g_last_fail = s ? s : "";
        longjmp(g_handlers[--g_handler_top], 1); /* consume the handler as it fires */
    }
    fputs("error: ", stderr);
    fputs(s ? s : "", stderr);
    fputc('\n', stderr);
    exit(1);
}
void maca_debug(maca_str s)  { line(stdout, s); }
void maca_print(maca_str s)  { fputs(s ? s : "", stdout); }
maca_str maca_input(void) {
    char* buf = NULL; size_t cap = 0;
    ssize_t n = getline(&buf, &cap, stdin);
    if (n < 0) { free(buf); return ""; }
    if (n > 0 && buf[n - 1] == '\n') buf[n - 1] = '\0';
    return buf;
}

/* ---- strings ---- */
maca_str maca_str_copy(maca_str s) {
    if (!s || !*s) return "";
    size_t n = strlen(s);
    char* r = (char*)xmalloc(n + 1);
    memcpy(r, s, n + 1);
    return r;
}
maca_str maca_concat(maca_str a, maca_str b) {
    if (!a) a = ""; if (!b) b = "";
    size_t la = strlen(a), lb = strlen(b);
    char* r = (char*)xmalloc(la + lb + 1);
    memcpy(r, a, la); memcpy(r + la, b, lb); r[la + lb] = '\0';
    return r;
}
maca_str maca_concat_n(size_t n, ...) {
    va_list ap, copy;
    va_start(ap, n);
    va_copy(copy, ap);
    size_t total = 0;
    for (size_t i = 0; i < n; i++) {
        maca_str s = va_arg(ap, maca_str);
        if (s) total += strlen(s);
    }
    va_end(ap);
    char* r = (char*)xmalloc(total + 1);
    char* w = r;
    for (size_t i = 0; i < n; i++) {
        maca_str s = va_arg(copy, maca_str);
        if (!s) continue;
        size_t len = strlen(s);
        memcpy(w, s, len);
        w += len;
    }
    va_end(copy);
    *w = '\0';
    return r;
}
maca_str maca_str_at(maca_str s, int64_t i) {
    if (!s || i < 0 || (size_t)i >= strlen(s)) return "";
    char* r = (char*)xmalloc(2);
    r[0] = s[i]; r[1] = '\0';
    return r;
}
/* A byte and the string holding it. `chr` is the inverse of `ord` and nothing
   else: a caller assembling UTF-8 does it a byte at a time on purpose. Outside
   1..255 the answer is the empty string rather than a wrapped value: zero
   would end the string it is in, and anything larger is not a byte. Every
   target agrees on exactly this domain. */
maca_str maca_chr(int64_t b) {
    if (b <= 0 || b > 255) return "";
    char* r = (char*)xmalloc(2);
    r[0] = (char)(b & 0xFF); r[1] = '\0';
    return r;
}
int64_t maca_ord(maca_str s) { return (s && s[0]) ? (int64_t)(unsigned char)s[0] : -1; }
int64_t maca_strlen(maca_str s) { return s ? (int64_t)strlen(s) : 0; }
bool maca_is_space(maca_str c) { return c && c[0] && isspace((unsigned char)c[0]) != 0; }
bool maca_is_digit(maca_str c) { return c && c[0] >= '0' && c[0] <= '9'; }
bool maca_is_alpha(maca_str c) {
    return c && ((c[0] >= 'a' && c[0] <= 'z') || (c[0] >= 'A' && c[0] <= 'Z'));
}
maca_str maca_from_int(int64_t n) {
    char* r = (char*)xmalloc(24);
    snprintf(r, 24, "%lld", (long long)n);
    return r;
}
maca_str maca_from_float(double d) {
    char* r = (char*)xmalloc(32);
    /* a whole-valued float prints as "12.0" (so a float reads as a float, and
     * native matches the playground interpreter); others use %g. */
    if (d == (double)(long long)d && d < 1e15 && d > -1e15) {
        snprintf(r, 32, "%.1f", d);
    } else {
        snprintf(r, 32, "%g", d);
    }
    return r;
}
maca_str maca_from_bool(bool b) { return b ? "true" : "false"; }
bool maca_str_eq(maca_str a, maca_str b) {
    if (a == b) return true;
    if (!a || !b) return false;
    return strcmp(a, b) == 0;
}
int maca_str_cmp(maca_str a, maca_str b) { return strcmp(a ? a : "", b ? b : ""); }
maca_str maca_join(maca_str* data, int64_t len, maca_str sep) {
    maca_sb sb; maca_sb_init(&sb);
    for (int64_t i = 0; i < len; i++) {
        if (i) maca_sb_puts(&sb, sep);
        maca_sb_puts(&sb, data[i]);
    }
    return maca_sb_finish(&sb);
}

/* ---- string stdlib ---- */
maca_str maca_trim(maca_str s) {
    if (!s) return "";
    const char* a = s;
    while (*a == ' ' || *a == '\t' || *a == '\n' || *a == '\r') a++;
    const char* b = s + strlen(s);
    while (b > a && (b[-1] == ' ' || b[-1] == '\t' || b[-1] == '\n' || b[-1] == '\r')) b--;
    size_t n = (size_t)(b - a);
    char* r = (char*)xmalloc(n + 1);
    memcpy(r, a, n); r[n] = '\0';
    return r;
}
maca_str maca_upper(maca_str s) {
    if (!s) return "";
    size_t n = strlen(s); char* r = (char*)xmalloc(n + 1);
    for (size_t i = 0; i < n; i++) r[i] = (char)toupper((unsigned char)s[i]);
    r[n] = '\0'; return r;
}
maca_str maca_lower(maca_str s) {
    if (!s) return "";
    size_t n = strlen(s); char* r = (char*)xmalloc(n + 1);
    for (size_t i = 0; i < n; i++) r[i] = (char)tolower((unsigned char)s[i]);
    r[n] = '\0'; return r;
}
/* ---- file I/O ---- */
/* One line of stdin with the newline stripped. The empty string at EOF, which
   is why `maca_at_eof` exists: a blank line and end-of-input read the same. */
maca_str maca_read_line(void) {
    size_t cap = 128, n = 0;
    char* buf = (char*)xmalloc(cap);
    int c;
    while ((c = fgetc(stdin)) != EOF && c != '\n') {
        if (n + 1 >= cap) {
            cap *= 2;
            char* bigger = (char*)xmalloc(cap);
            memcpy(bigger, buf, n);
            buf = bigger;
        }
        buf[n++] = (char)c;
    }
    buf[n] = '\0';
    return buf;
}
bool maca_at_eof(void) {
    int c = fgetc(stdin);
    if (c == EOF) return true;
    ungetc(c, stdin);
    return false;
}
maca_str maca_read_stdin(void) {
    size_t cap = 4096, n = 0;
    char* buf = (char*)xmalloc(cap);
    size_t got;
    while ((got = fread(buf + n, 1, cap - n - 1, stdin)) > 0) {
        n += got;
        if (n + 1 >= cap) {
            cap *= 2;
            char* bigger = (char*)xmalloc(cap);
            memcpy(bigger, buf, n);
            buf = bigger;
        }
    }
    buf[n] = '\0';
    return buf;
}
/* Time is UTC throughout. A local-time rendering needs a zone database and a
   policy for what to do without one; a program that wants local time can format
   the epoch milliseconds itself. */
int64_t maca_now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (int64_t)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}
maca_str maca_format_time(int64_t ms, maca_str fmt) {
    if (!fmt || !*fmt) fmt = "%Y-%m-%dT%H:%M:%SZ";
    time_t secs = (time_t)(ms / 1000);
    struct tm tmv;
    gmtime_r(&secs, &tmv);
    char* out = (char*)xmalloc(256);
    size_t n = strftime(out, 256, fmt, &tmv);
    out[n] = '\0';
    return out;
}
maca_str maca_now_iso(void) {
    return maca_format_time(maca_now_ms(), "%Y-%m-%dT%H:%M:%SZ");
}
maca_str maca_read_file(maca_str path) {
    if (!path) return "";
    /* Only an ordinary file has a length worth trusting. Opening a directory
       succeeds on Linux, and the size it then reports is not a byte count. The
       allocation below asked for it and the process died with "out of memory",
       naming nothing that pointed at the directory. Walking a tree and reading
       what the walk returns is the shape that hits this. */
    struct stat st;
    if (stat(path, &st) != 0 || !S_ISREG(st.st_mode)) return "";
    FILE* f = fopen(path, "rb");
    if (!f) return "";
    if (fseek(f, 0, SEEK_END) != 0) { fclose(f); return ""; }
    long n = ftell(f);
    if (n < 0) { fclose(f); return ""; }
    rewind(f);
    char* buf = (char*)xmalloc((size_t)n + 1);
    size_t got = fread(buf, 1, (size_t)n, f);
    fclose(f);
    buf[got] = '\0';
    return buf;
}
bool maca_write_file(maca_str path, maca_str text) {
    if (!path) return false;
    if (!text) text = "";
    FILE* f = fopen(path, "wb");
    if (!f) return false;
    size_t n = strlen(text);
    bool ok = fwrite(text, 1, n, f) == n;
    return fclose(f) == 0 && ok;
}
bool maca_file_exists(maca_str path) {
    struct stat st;
    return path && stat(path, &st) == 0;
}
bool maca_is_tty(void) { return isatty(1) == 1; }
/* The path with `.`, `..` *and symlinks* resolved, or "" when it names nothing.
   A server deciding whether a request stayed inside its root cannot do it with
   string arithmetic: a link inside the root points wherever it likes, and only
   the kernel knows where. */
maca_str maca_real_path(maca_str path) {
    if (!path) return "";
    char buf[4096];
    if (!realpath(path, buf)) return "";
    size_t n = strlen(buf);
    char* r = (char*)xmalloc(n + 1);
    memcpy(r, buf, n + 1);
    return r;
}
bool maca_is_dir(maca_str path) {
    struct stat st;
    return path && stat(path, &st) == 0 && S_ISDIR(st.st_mode);
}
/* -1 rather than 0 for "no such file", so an empty file and a missing one are
   distinguishable without a second call. */
int64_t maca_file_size(maca_str path) {
    struct stat st;
    if (!path || stat(path, &st) != 0) return -1;
    return (int64_t)st.st_size;
}
int64_t maca_modified_ms(maca_str path) {
    struct stat st;
    if (!path || stat(path, &st) != 0) return -1;
    return (int64_t)st.st_mtime * 1000;
}
bool maca_remove_file(maca_str path) {
    return path && unlink(path) == 0;
}
/* Depth-first, so a directory with contents goes too: `rm -r`, not `rmdir`. */
bool maca_remove_dir(maca_str path) {
    if (!path) return false;
    DIR* d = opendir(path);
    if (!d) return false;
    struct dirent* e;
    bool ok = true;
    size_t plen = strlen(path);
    while ((e = readdir(d)) != NULL) {
        if (strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0) continue;
        size_t nlen = strlen(e->d_name);
        char* child = (char*)xmalloc(plen + nlen + 2);
        memcpy(child, path, plen);
        child[plen] = '/';
        memcpy(child + plen + 1, e->d_name, nlen + 1);
        ok = (maca_is_dir(child) ? maca_remove_dir(child) : maca_remove_file(child)) && ok;
    }
    closedir(d);
    return rmdir(path) == 0 && ok;
}
/* `mkdir -p`: create each missing component in turn. */
bool maca_make_dir(maca_str path) {
    if (!path || !*path) return false;
    size_t n = strlen(path);
    char* tmp = (char*)xmalloc(n + 1);
    memcpy(tmp, path, n + 1);
    for (size_t i = 1; i <= n; i++) {
        if (tmp[i] == '/' || tmp[i] == '\0') {
            char save = tmp[i];
            tmp[i] = '\0';
            struct stat st;
            if (stat(tmp, &st) != 0 && mkdir(tmp, 0777) != 0) return false;
            tmp[i] = save;
        }
    }
    return true;
}
/* A byte-for-byte copy. `read_file` + `write_file` would stop at the first NUL:
   fine for source, silently truncating for a wasm module or an image. */
bool maca_copy_bytes(maca_str src, maca_str dst) {
    if (!src || !dst) return false;
    FILE* in = fopen(src, "rb");
    if (!in) return false;
    FILE* out = fopen(dst, "wb");
    if (!out) { fclose(in); return false; }
    char buf[65536];
    bool ok = true;
    for (;;) {
        size_t got = fread(buf, 1, sizeof buf, in);
        if (got == 0) break;
        if (fwrite(buf, 1, got, out) != got) { ok = false; break; }
    }
    if (ferror(in)) ok = false;
    fclose(in);
    return fclose(out) == 0 && ok;
}

maca_str* maca_list_dir(maca_str path, int64_t* out_len) {
    *out_len = 0;
    if (!path) return NULL;
    DIR* d = opendir(path);
    if (!d) return NULL;
    size_t cap = 16, n = 0;
    maca_str* names = (maca_str*)xmalloc(cap * sizeof(maca_str));
    struct dirent* e;
    while ((e = readdir(d)) != NULL) {
        if (strcmp(e->d_name, ".") == 0 || strcmp(e->d_name, "..") == 0) continue;
        if (n == cap) {
            cap *= 2;
            maca_str* bigger = (maca_str*)xmalloc(cap * sizeof(maca_str));
            memcpy(bigger, names, n * sizeof(maca_str));
            names = bigger;
        }
        size_t len = strlen(e->d_name);
        char* copy = (char*)xmalloc(len + 1);
        memcpy(copy, e->d_name, len + 1);
        names[n++] = copy;
    }
    closedir(d);
    /* readdir order is arbitrary; sort so builds are reproducible. */
    for (size_t i = 1; i < n; i++) {
        maca_str key = names[i];
        size_t j = i;
        while (j > 0 && strcmp(names[j - 1], key) > 0) { names[j] = names[j - 1]; j--; }
        names[j] = key;
    }
    *out_len = (int64_t)n;
    return names;
}

/* ---- processes ----------------------------------------------------------
   `fork` + `execvp`, not `system`: no shell means no quoting rules, so a path
   with a space in it is a path with a space in it. `execvp` still searches
   PATH, which is the one shell behaviour a build script actually wants. */

/* NULL-terminated argv, with argv[0] set to the program name as convention
   requires. Freed by the child's exec, or by exit. */
static char** maca_argv(maca_str cmd, maca_str* argv, int64_t n) {
    if (n < 0) n = 0;
    char** out = (char**)xmalloc((size_t)(n + 2) * sizeof(char*));
    out[0] = (char*)cmd;
    for (int64_t i = 0; i < n; i++) out[i + 1] = (char*)(argv[i] ? argv[i] : "");
    out[n + 1] = NULL;
    return out;
}

int64_t maca_exec(maca_str cmd, maca_str* argv, int64_t n) {
    if (!cmd) return -1;
    char** args = maca_argv(cmd, argv, n);
    pid_t pid = fork();
    if (pid < 0) return -1;
    if (pid == 0) {
        execvp(cmd, args);
        _exit(127);              /* exec failed: the shell's "not found" code */
    }
    int status = 0;
    if (waitpid(pid, &status, 0) < 0) return -1;
    if (WIFEXITED(status)) return (int64_t)WEXITSTATUS(status);
    if (WIFSIGNALED(status)) return (int64_t)(128 + WTERMSIG(status));
    return -1;
}

/* The child's stdout, whole. Its stderr is left alone, so a build script's
   diagnostics still reach the terminal while its output is captured. */
maca_str maca_capture(maca_str cmd, maca_str* argv, int64_t n) {
    if (!cmd) return "";
    int fds[2];
    if (pipe(fds) != 0) return "";
    char** args = maca_argv(cmd, argv, n);
    pid_t pid = fork();
    if (pid < 0) { close(fds[0]); close(fds[1]); return ""; }
    if (pid == 0) {
        close(fds[0]);
        dup2(fds[1], STDOUT_FILENO);
        close(fds[1]);
        execvp(cmd, args);
        _exit(127);
    }
    close(fds[1]);
    size_t cap = 4096, len = 0;
    char* buf = (char*)xmalloc(cap);
    for (;;) {
        if (len + 1024 > cap) {
            size_t bigger_cap = cap * 2;
            char* bigger = (char*)xmalloc(bigger_cap);
            memcpy(bigger, buf, len);
            buf = bigger; cap = bigger_cap;
        }
        ssize_t got = read(fds[0], buf + len, cap - len - 1);
        if (got <= 0) break;
        len += (size_t)got;
    }
    close(fds[0]);
    int status = 0;
    waitpid(pid, &status, 0);
    buf[len] = '\0';
    return buf;
}

maca_str maca_env(maca_str name) {
    if (!name) return "";
    const char* v = getenv(name);
    return v ? maca_str_copy(v) : "";
}

maca_str maca_cwd(void) {
    char* buf = (char*)xmalloc(4096);
    if (!getcwd(buf, 4096)) return "";
    return buf;
}

bool maca_chdir(maca_str path) {
    return path && chdir(path) == 0;
}

maca_str maca_repeat(maca_str s, int64_t n) {
    if (!s || n <= 0) return "";
    size_t len = strlen(s), total = len * (size_t)n;
    char* r = (char*)xmalloc(total + 1);
    for (int64_t i = 0; i < n; i++) memcpy(r + (size_t)i * len, s, len);
    r[total] = '\0'; return r;
}
/* Pad `s` to width `w` with `p` (repeated, then clipped) on the given side. */
static maca_str maca_pad(maca_str s, int64_t w, maca_str p, bool at_start) {
    if (!s) s = "";
    if (!p || !*p) p = " ";
    size_t len = strlen(s);
    if (w <= 0 || (size_t)w <= len) return maca_str_copy(s);
    size_t fill = (size_t)w - len, plen = strlen(p);
    char* r = (char*)xmalloc((size_t)w + 1);
    char* pad = at_start ? r : r + len;
    for (size_t i = 0; i < fill; i++) pad[i] = p[i % plen];
    memcpy(at_start ? r + fill : r, s, len);
    r[(size_t)w] = '\0'; return r;
}
maca_str maca_pad_start(maca_str s, int64_t w, maca_str p) { return maca_pad(s, w, p, true); }
maca_str maca_pad_end(maca_str s, int64_t w, maca_str p) { return maca_pad(s, w, p, false); }
/* Centre `s` in width `w`. An odd remainder goes on the right, so a column of
   centred cells stays flush left, the same choice Python's `str.center` makes. */
/* ` name="value"`, with the value escaped for an attribute context. Returns the
   empty string for an empty value, so an optional attribute can be passed as ""
   and simply not appear. */
maca_str maca_attr(maca_str name, maca_str value) {
    if (!name || !*name) return "";
    if (!value) value = "";
    size_t n = strlen(name), v = strlen(value);
    /* worst case every byte becomes &quot; (6) */
    char* r = (char*)xmalloc(n + v * 6 + 5);
    char* w = r;
    *w++ = ' ';
    memcpy(w, name, n); w += n;
    *w++ = '='; *w++ = '"';
    for (size_t i = 0; i < v; i++) {
        switch (value[i]) {
            case '&':  memcpy(w, "&amp;", 5);  w += 5; break;
            case '<':  memcpy(w, "&lt;", 4);   w += 4; break;
            case '>':  memcpy(w, "&gt;", 4);   w += 4; break;
            case '"':  memcpy(w, "&quot;", 6); w += 6; break;
            default:   *w++ = value[i];
        }
    }
    *w++ = '"'; *w = '\0';
    return r;
}
/* A boolean attribute: present or absent, never `open="false"`. HTML reads any
   value at all, including "false", as true, so a bool has to control the
   attribute's *existence* rather than its text. */
maca_str maca_flag(maca_str name, bool on) {
    if (!on || !name || !*name) return "";
    size_t n = strlen(name);
    char* r = (char*)xmalloc(n + 2);
    r[0] = ' ';
    memcpy(r + 1, name, n);
    r[n + 1] = '\0';
    return r;
}
static bool maca_void_tag(maca_str t) {
    static const char* v[] = {"area","base","br","col","embed","hr","img","input",
                              "link","meta","source","track","wbr",0};
    for (int i = 0; v[i]; i++) if (strcmp(t, v[i]) == 0) return true;
    return false;
}
/* An element whose tag is only known at run time: `<h1>`…`<h6>` from a depth,
   `<th>` or `<td>` from which row is being written. The static form can't say
   that, and a generator that walks a document needs to. Voidness is decided
   here because it can't be decided at compile time. */
maca_str maca_element(maca_str tag, maca_str attrs, maca_str kids) {
    if (!tag || !*tag) return maca_str_copy(kids);
    if (!attrs) attrs = "";
    if (!kids) kids = "";
    size_t t = strlen(tag), a = strlen(attrs), k = strlen(kids);
    bool v = maca_void_tag(tag);
    /* "<tag" attrs ">" kids "</tag>" */
    char* r = (char*)xmalloc(t + a + k + t + 6);
    char* w = r;
    *w++ = '<'; memcpy(w, tag, t); w += t;
    memcpy(w, attrs, a); w += a;
    *w++ = '>';
    if (!v) {
        memcpy(w, kids, k); w += k;
        *w++ = '<'; *w++ = '/'; memcpy(w, tag, t); w += t; *w++ = '>';
    }
    *w = '\0';
    return r;
}
maca_str maca_pad_center(maca_str s, int64_t w, maca_str p) {
    if (!s) s = "";
    if (!p || !*p) p = " ";
    size_t len = strlen(s);
    if (w <= 0 || (size_t)w <= len) return maca_str_copy(s);
    size_t fill = (size_t)w - len, left = fill / 2, plen = strlen(p);
    char* r = (char*)xmalloc((size_t)w + 1);
    for (size_t i = 0; i < left; i++) r[i] = p[i % plen];
    memcpy(r + left, s, len);
    for (size_t i = left + len; i < (size_t)w; i++) r[i] = p[(i - left - len) % plen];
    r[(size_t)w] = '\0'; return r;
}
/* `x` with exactly `n` decimal places, as text. */
maca_str maca_fixed(double x, int64_t n) {
    if (n < 0) n = 0;
    if (n > 17) n = 17;
    int need = snprintf(NULL, 0, "%.*f", (int)n, x);
    char* r = (char*)xmalloc((size_t)need + 1);
    snprintf(r, (size_t)need + 1, "%.*f", (int)n, x);
    return r;
}
bool maca_contains(maca_str s, maca_str sub) {
    if (!s) s = ""; if (!sub) sub = "";
    return strstr(s, sub) != NULL;
}
bool maca_starts_with(maca_str s, maca_str prefix) {
    if (!s) s = ""; if (!prefix) prefix = "";
    size_t lp = strlen(prefix);
    return strlen(s) >= lp && memcmp(s, prefix, lp) == 0;
}
bool maca_ends_with(maca_str s, maca_str suffix) {
    if (!s) s = ""; if (!suffix) suffix = "";
    size_t ls = strlen(s), lx = strlen(suffix);
    return ls >= lx && memcmp(s + ls - lx, suffix, lx) == 0;
}
int64_t maca_index_of(maca_str s, maca_str sub) {
    if (!s) s = ""; if (!sub) sub = "";
    const char* p = strstr(s, sub);
    return p ? (int64_t)(p - s) : -1;
}
maca_str maca_replace(maca_str s, maca_str from, maca_str to) {
    if (!s) return "";
    if (!from || !*from) return maca_str_copy(s); /* empty needle: no-op (avoid infinite loop) */
    if (!to) to = "";
    size_t lf = strlen(from);
    maca_sb sb; maca_sb_init(&sb);
    const char* p = s;
    for (;;) {
        const char* hit = strstr(p, from);
        if (!hit) { maca_sb_puts(&sb, p); break; }
        for (const char* q = p; q < hit; q++) maca_sb_putc(&sb, *q);
        maca_sb_puts(&sb, to);
        p = hit + lf;
    }
    return maca_sb_finish(&sb);
}
maca_str maca_substr(maca_str s, int64_t start, int64_t len) {
    if (!s) return "";
    int64_t n = (int64_t)strlen(s);
    if (start < 0) start = 0;
    if (start > n) start = n;
    if (len < 0) len = 0;
    if (start + len > n) len = n - start;
    char* r = (char*)xmalloc((size_t)len + 1);
    memcpy(r, s + start, (size_t)len); r[len] = '\0';
    return r;
}
/* `slice` takes an exclusive end, matching the list method of the same name.
   The asymmetry with `substr`'s length is deliberate: the two names mean two
   different things, and having both is what lets each keep its own convention. */
maca_str maca_str_slice(maca_str s, int64_t from, int64_t to) {
    if (!s) return "";
    int64_t n = (int64_t)strlen(s);
    if (from < 0) from = 0;
    if (to > n) to = n;
    return maca_substr(s, from, to - from);
}

/* Assertions.
 *
 * A failing assertion prints and keeps going rather than aborting, and the
 * count is what a test returns: one run reports every failure instead of only
 * the first, which is the difference between fixing a suite in one pass and in
 * as many passes as it has bugs. `maca_failures()` is the exit code a test
 * function returns, so the existing "0 or non-zero" contract is unchanged. */
static int64_t maca_failed_count = 0;
int64_t maca_failures(void) { return maca_failed_count; }
bool maca_assert(bool cond, maca_str msg) {
    if (cond) return true;
    maca_failed_count++;
    fprintf(stderr, "assertion failed: %s\n", msg && *msg ? msg : "(no message)");
    return false;
}
bool maca_assert_eq(maca_str got, maca_str want, maca_str msg) {
    if (!got) got = "";
    if (!want) want = "";
    if (strcmp(got, want) == 0) return true;
    maca_failed_count++;
    fprintf(stderr, "assertion failed: %s\n  got:  %s\n  want: %s\n",
            msg && *msg ? msg : "(no message)", got, want);
    return false;
}
static int maca_cmp_i64(const void* a, const void* b) {
    int64_t x = *(const int64_t*)a, y = *(const int64_t*)b;
    return (x > y) - (x < y);
}
static int maca_cmp_f64(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b;
    return (x > y) - (x < y);
}
static int maca_cmp_str(const void* a, const void* b) {
    maca_str x = *(const maca_str*)a, y = *(const maca_str*)b;
    return strcmp(x ? x : "", y ? y : "");
}
void maca_sort_i64(int64_t* data, int64_t n) { if (n > 1) qsort(data, (size_t)n, sizeof(int64_t), maca_cmp_i64); }
void maca_sort_f64(double* data, int64_t n) { if (n > 1) qsort(data, (size_t)n, sizeof(double), maca_cmp_f64); }
void maca_sort_str(maca_str* data, int64_t n) { if (n > 1) qsort(data, (size_t)n, sizeof(maca_str), maca_cmp_str); }
/* FNV-1a: short, dependency-free, and good enough for the key sizes a program
   written in this language actually uses. */
uint64_t maca_hash_str(maca_str s) {
    uint64_t h = 1469598103934665603ULL;
    if (!s) return h;
    for (const unsigned char* p = (const unsigned char*)s; *p; p++) {
        h ^= (uint64_t)*p;
        h *= 1099511628211ULL;
    }
    return h;
}

maca_str* maca_split(maca_str s, maca_str sep, int64_t* out_len) {
    if (!s) s = "";
    size_t cap = 8, n = 0;
    maca_str* out = (maca_str*)xmalloc(cap * sizeof(maca_str));
    /* empty separator: split into whole string as one element */
    if (!sep || !*sep) {
        out[n++] = maca_str_copy(s);
        *out_len = (int64_t)n;
        return out;
    }
    size_t ls = strlen(sep);
    const char* p = s;
    for (;;) {
        const char* hit = strstr(p, sep);
        size_t seglen = hit ? (size_t)(hit - p) : strlen(p);
        char* seg = (char*)xmalloc(seglen + 1);
        memcpy(seg, p, seglen); seg[seglen] = '\0';
        if (n == cap) { cap *= 2; out = (maca_str*)maca_realloc(out, cap * sizeof(maca_str)); if (!out) die("out of memory"); }
        out[n++] = seg;
        if (!hit) break;
        p = hit + ls;
    }
    *out_len = (int64_t)n;
    return out;
}

/* ---- string builder ---- */
void maca_sb_init(maca_sb* sb) { sb->buf = (char*)xmalloc(16); sb->len = 0; sb->cap = 16; }
static void sb_reserve(maca_sb* sb, size_t extra) {
    if (sb->len + extra + 1 > sb->cap) {
        while (sb->len + extra + 1 > sb->cap) sb->cap *= 2;
        sb->buf = (char*)maca_realloc(sb->buf, sb->cap);
        if (!sb->buf) die("out of memory");
    }
}
void maca_sb_putc(maca_sb* sb, char c) { sb_reserve(sb, 1); sb->buf[sb->len++] = c; }
void maca_sb_puts(maca_sb* sb, const char* s) {
    if (!s) return;
    size_t n = strlen(s); sb_reserve(sb, n);
    memcpy(sb->buf + sb->len, s, n); sb->len += n;
}
void maca_sb_put_json_str(maca_sb* sb, maca_str s) {
    if (!s) s = "";
    maca_sb_putc(sb, '"');
    for (const unsigned char* p = (const unsigned char*)s; *p; p++) {
        switch (*p) {
            case '"':  maca_sb_puts(sb, "\\\""); break;
            case '\\': maca_sb_puts(sb, "\\\\"); break;
            case '\n': maca_sb_puts(sb, "\\n"); break;
            case '\t': maca_sb_puts(sb, "\\t"); break;
            case '\r': maca_sb_puts(sb, "\\r"); break;
            default:
                if (*p < 0x20) { char b[8]; snprintf(b, 8, "\\u%04x", *p); maca_sb_puts(sb, b); }
                else maca_sb_putc(sb, (char)*p);
        }
    }
    maca_sb_putc(sb, '"');
}
maca_str maca_sb_finish(maca_sb* sb) { sb->buf[sb->len] = '\0'; return sb->buf; }

/* ---- paths & files ---- */
maca_str maca_path_join(maca_str a, maca_str b) {
    if (!a || !*a) return maca_str_copy(b);
    if (!b || !*b) return maca_str_copy(a);
    size_t la = strlen(a);
    bool slash = a[la - 1] == '/';
    return maca_concat(a, slash ? b : maca_concat("/", b));
}
maca_str maca_dirs_data(void) {
    const char* x = getenv("XDG_DATA_HOME");
    if (x && *x) return maca_str_copy(x);
    const char* h = getenv("HOME");
    if (!h || !*h) h = ".";
    return maca_path_join(h, ".local/share");
}
bool maca_path_exists(maca_str p) { FILE* f = fopen(p, "rb"); if (f) { fclose(f); return true; } return false; }
maca_str maca_read(maca_str p) {
    FILE* f = fopen(p, "rb");
    if (!f) die("cannot read file");
    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);
    if (sz < 0) { fclose(f); die("cannot size file"); }
    char* buf = (char*)xmalloc((size_t)sz + 1);
    size_t got = fread(buf, 1, (size_t)sz, f);
    fclose(f); buf[got] = '\0';
    return buf;
}
static void mkdir_p(const char* path) {
    char tmp[4096]; size_t n = strlen(path);
    if (n >= sizeof(tmp)) return;
    memcpy(tmp, path, n + 1);
    for (char* p = tmp + 1; *p; p++) {
        if (*p == '/') {
            *p = '\0';
#if defined(_WIN32)
            _mkdir(tmp);
#else
            mkdir(tmp, 0755);
#endif
            *p = '/';
        }
    }
}
void maca_write(maca_str p, maca_str content) {
    /* create parent directories */
    const char* slash = strrchr(p, '/');
    if (slash) { char dir[4096]; size_t n = (size_t)(slash - p); if (n < sizeof(dir)) { memcpy(dir, p, n); dir[n] = '\0'; mkdir_p(dir); } }
    FILE* f = fopen(p, "wb");
    if (!f) die("cannot write file");
    if (content) fwrite(content, 1, strlen(content), f);
    fclose(f);
}

/* ---- JSON parser (recursive descent) ---- */
typedef struct { const char* s; } jp;
static maca_json* jp_value(jp* p);
static void jp_ws(jp* p) { while (*p->s == ' ' || *p->s == '\t' || *p->s == '\n' || *p->s == '\r') p->s++; }
static maca_json* jnew(maca_json_kind k) { maca_json* j = (maca_json*)xmalloc(sizeof(maca_json)); memset(j, 0, sizeof(*j)); j->kind = k; return j; }
static char* jp_string_raw(jp* p) {
    if (*p->s != '"') die("json: expected string");
    p->s++;
    maca_sb sb; maca_sb_init(&sb);
    while (*p->s && *p->s != '"') {
        char c = *p->s++;
        if (c == '\\') {
            char e = *p->s++;
            switch (e) {
                case 'n': maca_sb_putc(&sb, '\n'); break;
                case 't': maca_sb_putc(&sb, '\t'); break;
                case 'r': maca_sb_putc(&sb, '\r'); break;
                case '"': maca_sb_putc(&sb, '"'); break;
                case '\\': maca_sb_putc(&sb, '\\'); break;
                case '/': maca_sb_putc(&sb, '/'); break;
                case 'u': { /* minimal: skip 4 hex, emit '?' for non-ascii */
                    int v = 0; for (int i = 0; i < 4 && *p->s; i++) { char h = *p->s++; v = v * 16 + (h <= '9' ? h - '0' : (h | 32) - 'a' + 10); }
                    maca_sb_putc(&sb, v < 128 ? (char)v : '?'); break; }
                default: maca_sb_putc(&sb, e);
            }
        } else maca_sb_putc(&sb, c);
    }
    if (*p->s != '"') die("json: unterminated string");
    p->s++;
    return (char*)maca_sb_finish(&sb);
}
static maca_json* jp_value(jp* p) {
    jp_ws(p);
    char c = *p->s;
    if (c == '{') {
        p->s++; maca_json* j = jnew(MJ_OBJ);
        jp_ws(p);
        if (*p->s == '}') { p->s++; return j; }
        for (;;) {
            jp_ws(p);
            char* k = jp_string_raw(p);
            jp_ws(p); if (*p->s != ':') die("json: expected ':'"); p->s++;
            maca_json* v = jp_value(p);
            j->obj.keys = (char**)maca_realloc(j->obj.keys, sizeof(char*) * (j->obj.len + 1));
            j->obj.vals = (maca_json**)maca_realloc(j->obj.vals, sizeof(maca_json*) * (j->obj.len + 1));
            if (!j->obj.keys || !j->obj.vals) die("out of memory");
            j->obj.keys[j->obj.len] = k; j->obj.vals[j->obj.len] = v; j->obj.len++;
            jp_ws(p);
            if (*p->s == ',') { p->s++; continue; }
            if (*p->s == '}') { p->s++; break; }
            die("json: expected ',' or '}'");
        }
        return j;
    }
    if (c == '[') {
        p->s++; maca_json* j = jnew(MJ_ARR);
        jp_ws(p);
        if (*p->s == ']') { p->s++; return j; }
        for (;;) {
            maca_json* v = jp_value(p);
            j->arr.items = (maca_json**)maca_realloc(j->arr.items, sizeof(maca_json*) * (j->arr.len + 1));
            if (!j->arr.items) die("out of memory");
            j->arr.items[j->arr.len++] = v;
            jp_ws(p);
            if (*p->s == ',') { p->s++; continue; }
            if (*p->s == ']') { p->s++; break; }
            die("json: expected ',' or ']'");
        }
        return j;
    }
    if (c == '"') { maca_json* j = jnew(MJ_STR); j->str = jp_string_raw(p); return j; }
    if (c == 't') { if (strncmp(p->s, "true", 4)) die("json: bad literal"); p->s += 4; maca_json* j = jnew(MJ_BOOL); j->b = true; return j; }
    if (c == 'f') { if (strncmp(p->s, "false", 5)) die("json: bad literal"); p->s += 5; maca_json* j = jnew(MJ_BOOL); j->b = false; return j; }
    if (c == 'n') { if (strncmp(p->s, "null", 4)) die("json: bad literal"); p->s += 4; return jnew(MJ_NULL); }
    /* number */
    {
        char* end = NULL;
        double d = strtod(p->s, &end);
        if (end == p->s) die("json: unexpected character");
        p->s = end;
        maca_json* j = jnew(MJ_NUM); j->num = d; return j;
    }
}
maca_json* maca_json_parse(maca_str text) {
    jp p; p.s = text ? text : "";
    maca_json* j = jp_value(&p);
    jp_ws(&p);
    return j;
}
maca_json* maca_json_get(maca_json* o, const char* key) {
    if (!o || o->kind != MJ_OBJ) return NULL;
    for (int64_t i = 0; i < o->obj.len; i++) if (strcmp(o->obj.keys[i], key) == 0) return o->obj.vals[i];
    return NULL;
}
maca_str maca_json_quote(maca_str s) {
    maca_sb sb; maca_sb_init(&sb);
    maca_sb_put_json_str(&sb, s);
    return maca_sb_finish(&sb);
}
static maca_str mj_kind_name(maca_json_kind k) {
    switch (k) {
        case MJ_NULL: return "null";
        case MJ_BOOL: return "a boolean";
        case MJ_NUM: return "a number";
        case MJ_STR: return "a string";
        case MJ_ARR: return "a list";
        case MJ_OBJ: return "an object";
    }
    return "a value";
}
maca_json* maca_json_want(maca_json* o, maca_str field, maca_json_kind kind) {
    maca_json* v = maca_json_get(o, field);
    maca_sb sb; maca_sb_init(&sb);
    maca_sb_puts(&sb, "field `"); maca_sb_puts(&sb, field);
    maca_sb_puts(&sb, "`: expected "); maca_sb_puts(&sb, mj_kind_name(kind));
    if (!v) {
        maca_sb_puts(&sb, ", and the object has no such field");
        maca_fail(maca_sb_finish(&sb));
    }
    if (v->kind != kind) {
        maca_sb_puts(&sb, ", got "); maca_sb_puts(&sb, mj_kind_name(v->kind));
        maca_fail(maca_sb_finish(&sb));
    }
    maca_drop_str(maca_sb_finish(&sb));
    return v;
}
maca_json* maca_json_object(maca_json* j, maca_str type) {
    if (j && j->kind == MJ_OBJ) return j;
    maca_sb sb; maca_sb_init(&sb);
    maca_sb_puts(&sb, "`"); maca_sb_puts(&sb, type);
    maca_sb_puts(&sb, "`: expected an object, got ");
    maca_sb_puts(&sb, j ? mj_kind_name(j->kind) : "nothing");
    maca_fail(maca_sb_finish(&sb));
    return j;
}
int64_t maca_json_int(maca_json* j) { return j && j->kind == MJ_NUM ? (int64_t)j->num : 0; }
double maca_json_float(maca_json* j) { return j && j->kind == MJ_NUM ? j->num : 0.0; }
bool maca_json_bool(maca_json* j) { return j && j->kind == MJ_BOOL ? j->b : false; }
maca_str maca_json_str(maca_json* j) { return j && j->kind == MJ_STR && j->str ? maca_str_copy(j->str) : ""; }
"##;

/// `maca_async.h`: the concurrency runtime interface.
pub const ASYNC_H: &str = r##"#ifndef MACA_ASYNC_H
#define MACA_ASYNC_H
#include <stdint.h>
#include "maca_runtime.h"

/* bounded parallel map over int64: ordered results, at most `max_conc` threads.
 * `f` is a closure (see maca_runtime.h) so it may capture. */
int64_t* maca_parallel_i64(int64_t* xs, int64_t n, maca_closure f, int max_conc);

/* structured cancellation: workers poll a token; a demo proves they stop. */
typedef struct { volatile int flag; } maca_cancel;
void maca_cancel_set(maca_cancel* c);
int maca_cancel_check(maca_cancel* c);
int64_t maca_cancel_demo(int64_t workers);

/* colorblind async: `spawn f(x)` runs `f` concurrently and returns a future;
 * `await fut` blocks until it resolves. A task is an ordinary `int64 -> int64`
 * function, with no `async` coloring and no ABI change. POSIX threads back the
 * model:
 * a suspension point is a real thread boundary, which keeps the runtime small
 * enough to read and costs a thread per task. */
typedef int64_t (*maca_task_fn)(int64_t);
/* `spawn f(a, b)`. A separate arity rather than a boxed argument list: two is
   where real uses stop (a server's port and its handler, a worker's input and
   its sink), and a tuple ABI nobody could see would cost more to read than it
   saves. Three or more is a diagnostic, not a silent truncation. */
typedef int64_t (*maca_task_fn2)(int64_t, int64_t);
typedef struct maca_future maca_future;
maca_future* maca_spawn(maca_task_fn fn, int64_t arg);
maca_future* maca_spawn2(maca_task_fn2 fn, int64_t a, int64_t b);
int64_t maca_await(maca_future* f);
void maca_sleep_ms(int64_t ms);

#endif
"##;

/// `maca_async.c`: the colorblind-async slice, a bounded POSIX-thread worker pool and a cancellation token.
pub const ASYNC_C: &str = r##"#include "maca_async.h"
#include "maca_runtime.h"
#include <pthread.h>
#include <time.h>

#define MACA_MAX_THREADS 64

typedef struct { int64_t* xs; int64_t* out; int64_t from, to; maca_closure f; } pslice;
static void* pworker(void* arg) {
    pslice* s = (pslice*)arg;
    for (int64_t i = s->from; i < s->to; i++) s->out[i] = maca_call1(s->f, s->xs[i]);
    return NULL;
}
int64_t* maca_parallel_i64(int64_t* xs, int64_t n, maca_closure f, int max_conc) {
    int64_t* out = (int64_t*)maca_alloc((size_t)(n > 0 ? n : 1) * sizeof(int64_t));
    if (n <= 0) return out;
    int t = max_conc < 1 ? 1 : max_conc;
    if ((int64_t)t > n) t = (int)n;
    if (t > MACA_MAX_THREADS) t = MACA_MAX_THREADS;
    pthread_t th[MACA_MAX_THREADS];
    pslice sl[MACA_MAX_THREADS];
    int64_t chunk = (n + t - 1) / t;
    int made = 0;
    for (int k = 0; k < t; k++) {
        int64_t from = (int64_t)k * chunk;
        if (from >= n) break;
        int64_t to = from + chunk; if (to > n) to = n;
        sl[k].xs = xs; sl[k].out = out; sl[k].from = from; sl[k].to = to; sl[k].f = f;
        pthread_create(&th[k], NULL, pworker, &sl[k]);
        made++;
    }
    for (int k = 0; k < made; k++) pthread_join(th[k], NULL);
    return out;
}

void maca_cancel_set(maca_cancel* c) { c->flag = 1; }
int maca_cancel_check(maca_cancel* c) { return c->flag; }

typedef struct { maca_cancel* c; int64_t iters; } cwork;
static void* cworker(void* a) {
    cwork* w = (cwork*)a;
    /* loop forever until cancelled; if cancellation is broken, this hangs */
    while (!maca_cancel_check(w->c)) w->iters++;
    return NULL;
}
int64_t maca_cancel_demo(int64_t workers) {
    if (workers < 1) workers = 1;
    if (workers > MACA_MAX_THREADS) workers = MACA_MAX_THREADS;
    maca_cancel c; c.flag = 0;
    pthread_t th[MACA_MAX_THREADS];
    cwork w[MACA_MAX_THREADS];
    for (int64_t k = 0; k < workers; k++) { w[k].c = &c; w[k].iters = 0; pthread_create(&th[k], NULL, cworker, &w[k]); }
    for (volatile int64_t spin = 0; spin < 2000000; spin++) {}
    maca_cancel_set(&c);
    for (int64_t k = 0; k < workers; k++) pthread_join(th[k], NULL);
    int64_t total = 0;
    for (int64_t k = 0; k < workers; k++) total += w[k].iters;
    return total; /* > 0 (workers ran) and finite (they stopped on cancel) */
}

/* ---- futures: spawn / await ---- */
struct maca_future {
    pthread_t th;
    maca_task_fn fn;
    maca_task_fn2 fn2;
    int64_t arg;
    int64_t arg2;
    int64_t result;
    int joined;
};
static void* maca_task_trampoline(void* p) {
    maca_future* f = (maca_future*)p;
    f->result = f->fn2 ? f->fn2(f->arg, f->arg2) : f->fn(f->arg);
    return NULL;
}
maca_future* maca_spawn(maca_task_fn fn, int64_t arg) {
    maca_future* f = (maca_future*)maca_alloc(sizeof(maca_future));
    f->fn = fn; f->fn2 = 0; f->arg = arg; f->arg2 = 0; f->result = 0; f->joined = 0;
    pthread_create(&f->th, NULL, maca_task_trampoline, f);
    return f;
}
maca_future* maca_spawn2(maca_task_fn2 fn, int64_t a, int64_t b) {
    maca_future* f = (maca_future*)maca_alloc(sizeof(maca_future));
    f->fn = 0; f->fn2 = fn; f->arg = a; f->arg2 = b; f->result = 0; f->joined = 0;
    pthread_create(&f->th, NULL, maca_task_trampoline, f);
    return f;
}
int64_t maca_await(maca_future* f) {
    if (!f) return 0;
    if (!f->joined) { pthread_join(f->th, NULL); f->joined = 1; }
    return f->result;
}
void maca_sleep_ms(int64_t ms) {
    if (ms < 0) ms = 0;
    struct timespec ts;
    ts.tv_sec = (time_t)(ms / 1000);
    ts.tv_nsec = (long)((ms % 1000) * 1000000L);
    nanosleep(&ts, NULL);
}
"##;

/// Write `maca_runtime.h` + `maca_runtime.c` (and always the async header) into `dir`.
pub fn write_to(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_runtime.h"), RUNTIME_H)?;
    std::fs::write(dir.join("maca_runtime.c"), RUNTIME_C)?;
    std::fs::write(dir.join("maca_async.h"), ASYNC_H)?;
    Ok(())
}

/// Write `maca_async.c` into `dir` (only linked when a program uses async).
pub fn write_async(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_async.c"), ASYNC_C)?;
    Ok(())
}

/// FFI binding glue for `import c "sqlite3.h"`.
pub const SQLITE_GLUE: &str = r#"#include "maca_runtime.h"
#include <sqlite3.h>

/* Opaque handles: a db/stmt pointer round-trips through an int64, which the
 * Maca surface can hold as a plain `int`. This gives multiple connections and
 * full row/column iteration (a DB browser needs both). */
static int64_t as_handle(void* p) { return (int64_t)(intptr_t)p; }
static sqlite3* as_db(int64_t h) { return (sqlite3*)(intptr_t)h; }
static sqlite3_stmt* as_stmt(int64_t h) { return (sqlite3_stmt*)(intptr_t)h; }

static maca_str dup_text(const unsigned char* t) {
    if (!t) return "";
    size_t n = strlen((const char*)t);
    char* b = (char*)maca_alloc(n + 1);
    memcpy(b, t, n + 1);
    return b;
}

/* open a connection; returns a db handle (0 on failure) */
int64_t sqlite_open(maca_str path) {
    sqlite3* db = 0;
    if (sqlite3_open(path, &db) != SQLITE_OK) { if (db) sqlite3_close(db); return 0; }
    return as_handle(db);
}
int64_t sqlite_close(int64_t db) { return sqlite3_close(as_db(db)); }

/* run a statement with no result set (CREATE/INSERT/UPDATE/…) */
int64_t sqlite_exec(int64_t db, maca_str sql) {
    char* err = 0;
    int rc = sqlite3_exec(as_db(db), sql, 0, 0, &err);
    if (err) sqlite3_free(err);
    return rc;
}

/* prepare a query; returns a stmt handle (0 on failure) */
int64_t sqlite_prepare(int64_t db, maca_str sql) {
    sqlite3_stmt* st = 0;
    if (sqlite3_prepare_v2(as_db(db), sql, -1, &st, 0) != SQLITE_OK) return 0;
    return as_handle(st);
}
/* advance to the next row; true while a row is available */
bool sqlite_step(int64_t st) { return sqlite3_step(as_stmt(st)) == SQLITE_ROW; }
int64_t sqlite_column_count(int64_t st) { return sqlite3_column_count(as_stmt(st)); }
maca_str sqlite_column_name(int64_t st, int64_t col) { return dup_text((const unsigned char*)sqlite3_column_name(as_stmt(st), (int)col)); }
maca_str sqlite_column_text(int64_t st, int64_t col) { return dup_text(sqlite3_column_text(as_stmt(st), (int)col)); }
int64_t sqlite_column_int(int64_t st, int64_t col) { return sqlite3_column_int64(as_stmt(st), (int)col); }
double sqlite_column_float(int64_t st, int64_t col) { return sqlite3_column_double(as_stmt(st), (int)col); }
int64_t sqlite_finalize(int64_t st) { return sqlite3_finalize(as_stmt(st)); }

/* convenience: text of column 0 of the first row (or "") */
maca_str sqlite_query1(int64_t db, maca_str sql) {
    int64_t st = sqlite_prepare(db, sql);
    if (!st) return "";
    maca_str out = sqlite_step(st) ? sqlite_column_text(st, 0) : "";
    sqlite_finalize(st);
    return out;
}
"#;

pub fn write_sqlite_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_sqlite.c"), SQLITE_GLUE)?;
    Ok(())
}

/// FFI binding glue for `import py "module"`.
pub const PY_GLUE: &str = r#"#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "maca_runtime.h"

/* str(result) as a maca_str (owns a fresh copy) */
static maca_str py_str_result(PyObject* r) {
    maca_str out = "<py error>";
    if (!r) return out;
    PyObject* s = PyObject_Str(r);
    if (s) {
        const char* c = PyUnicode_AsUTF8(s);
        if (c) { size_t n = strlen(c); char* b = (char*)maca_alloc(n + 1); memcpy(b, c, n + 1); out = b; }
        Py_DECREF(s);
    }
    return out;
}

/* resolve module.func into a callable (borrowed refs freed by the caller) */
static PyObject* py_lookup(maca_str module, maca_str func) {
    PyObject* m = PyImport_ImportModule(module);
    if (!m) return 0;
    PyObject* f = PyObject_GetAttrString(m, func);
    Py_DECREF(m);
    if (f && PyCallable_Check(f)) return f;
    Py_XDECREF(f);
    return 0;
}

maca_str py_call(maca_str module, maca_str func) {
    Py_Initialize();
    maca_str out = "<py error>";
    PyObject* f = py_lookup(module, func);
    if (f) {
        PyObject* r = PyObject_CallObject(f, 0);
        out = py_str_result(r);
        Py_XDECREF(r);
        Py_DECREF(f);
    }
    Py_Finalize();
    return out;
}

/* call module.func(arg) with a single string argument */
maca_str py_call_s(maca_str module, maca_str func, maca_str arg) {
    Py_Initialize();
    maca_str out = "<py error>";
    PyObject* f = py_lookup(module, func);
    if (f) {
        PyObject* r = PyObject_CallFunction(f, "s", arg ? arg : "");
        out = py_str_result(r);
        Py_XDECREF(r);
        Py_DECREF(f);
    }
    Py_Finalize();
    return out;
}

/* call module.func(n) with a single integer argument */
maca_str py_call_i(maca_str module, maca_str func, int64_t n) {
    Py_Initialize();
    maca_str out = "<py error>";
    PyObject* f = py_lookup(module, func);
    if (f) {
        PyObject* r = PyObject_CallFunction(f, "L", (long long)n);
        out = py_str_result(r);
        Py_XDECREF(r);
        Py_DECREF(f);
    }
    Py_Finalize();
    return out;
}
"#;

pub fn write_py_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_py.c"), PY_GLUE)?;
    Ok(())
}

/// `std/mqtt` engine (for `import c "mqtt.h"`).
pub const MQTT_GLUE: &str = r#"#define _GNU_SOURCE
#include "maca_runtime.h"
#include <stdio.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>

static int write_all(int fd, const unsigned char* b, int n) {
    int off = 0;
    while (off < n) { int w = write(fd, b + off, n - off); if (w <= 0) return -1; off += w; }
    return 0;
}
static int read_n(int fd, unsigned char* b, int n) {
    int off = 0;
    while (off < n) { int r = read(fd, b + off, n - off); if (r <= 0) return -1; off += r; }
    return 0;
}
static int read_remlen(int fd) {
    int mult = 1, value = 0; unsigned char c;
    do { if (read_n(fd, &c, 1)) return -1; value += (c & 127) * mult; mult *= 128; } while (c & 128);
    return value;
}
static int enc_remlen(unsigned char* b, int len) {
    int i = 0;
    do { unsigned char c = len % 128; len /= 128; if (len) c |= 128; b[i++] = c; } while (len);
    return i;
}
/* build an MQTT string (2-byte len + bytes) into b, return bytes written */
static int put_str(unsigned char* b, const char* s) {
    int n = (int)strlen(s);
    b[0] = (n >> 8) & 0xFF; b[1] = n & 0xFF;
    memcpy(b + 2, s, n);
    return n + 2;
}
static int send_packet(int fd, unsigned char type_flags, const unsigned char* body, int blen) {
    unsigned char hdr[5]; hdr[0] = type_flags;
    int hn = 1 + enc_remlen(hdr + 1, blen);
    if (write_all(fd, hdr, hn)) return -1;
    if (blen && write_all(fd, body, blen)) return -1;
    return 0;
}

/* ---- topic wildcard match (`+` one level, `#` rest) ---- */
static int tmatch(const char* f, const char* t) {
    if (*f == '#') return 1;
    if (*f == 0 && *t == 0) return 1;
    if (*f == 0 || *t == 0) return 0;
    const char* fe = f; while (*fe && *fe != '/') fe++;
    const char* te = t; while (*te && *te != '/') te++;
    int fl = (int)(fe - f);
    int ok = (fl == 1 && f[0] == '+') || ((int)(te - t) == fl && strncmp(f, t, fl) == 0);
    if (!ok) return 0;
    return tmatch(*fe == '/' ? fe + 1 : fe, *te == '/' ? te + 1 : te);
}

/* ---- broker ---- */
#define MAX_SUBS 8192
typedef struct { int fd; char pat[256]; } sub_t;
static sub_t g_subs[MAX_SUBS];
static int g_nsubs = 0;
static pthread_mutex_t g_lock = PTHREAD_MUTEX_INITIALIZER;

static void route(const char* topic, const unsigned char* payload, int plen) {
    pthread_mutex_lock(&g_lock);
    for (int i = 0; i < g_nsubs; i++) {
        if (tmatch(g_subs[i].pat, topic)) {
            unsigned char body[1024];
            int n = put_str(body, topic);
            if (plen > 0 && n + plen < (int)sizeof(body)) { memcpy(body + n, payload, plen); n += plen; }
            send_packet(g_subs[i].fd, 0x30, body, n);
        }
    }
    pthread_mutex_unlock(&g_lock);
}

static void* client_thread(void* arg) {
    int fd = (int)(long)arg;
    unsigned char b1;
    while (read_n(fd, &b1, 1) == 0) {
        int rl = read_remlen(fd);
        if (rl < 0 || rl > 65535) break;
        unsigned char* body = (unsigned char*)malloc(rl > 0 ? rl : 1);
        if (rl > 0 && read_n(fd, body, rl)) { free(body); break; }
        int type = b1 >> 4;
        if (type == 1) {                       /* CONNECT → CONNACK */
            unsigned char ack[2] = { 0x00, 0x00 };
            send_packet(fd, 0x20, ack, 2);
        } else if (type == 8) {                /* SUBSCRIBE → register + SUBACK */
            int p = 2;                         /* skip packet id */
            unsigned char subacks[64]; int na = 0;
            while (p + 2 <= rl) {
                int tl = (body[p] << 8) | body[p + 1]; p += 2;
                if (p + tl > rl) break;
                pthread_mutex_lock(&g_lock);
                if (g_nsubs < MAX_SUBS && tl < 256) {
                    g_subs[g_nsubs].fd = fd;
                    memcpy(g_subs[g_nsubs].pat, body + p, tl);
                    g_subs[g_nsubs].pat[tl] = 0;
                    g_nsubs++;
                }
                pthread_mutex_unlock(&g_lock);
                p += tl + 1;                   /* + requested qos byte */
                if (na < 64) subacks[na++] = 0x00;
            }
            unsigned char sb[66]; sb[0] = body[0]; sb[1] = body[1];
            memcpy(sb + 2, subacks, na);
            send_packet(fd, 0x90, sb, 2 + na);
        } else if (type == 3) {                /* PUBLISH (QoS 0) */
            int tl = (body[0] << 8) | body[1];
            char topic[256];
            int cl = tl < 255 ? tl : 255;
            memcpy(topic, body + 2, cl); topic[cl] = 0;
            int po = 2 + tl;
            route(topic, body + po, rl - po);
        } else if (type == 12) {               /* PINGREQ → PINGRESP */
            send_packet(fd, 0xD0, 0, 0);
        } else if (type == 14) {               /* DISCONNECT */
            free(body); break;
        }
        free(body);
    }
    /* drop this fd's subscriptions */
    pthread_mutex_lock(&g_lock);
    int j = 0;
    for (int i = 0; i < g_nsubs; i++) if (g_subs[i].fd != fd) g_subs[j++] = g_subs[i];
    g_nsubs = j;
    pthread_mutex_unlock(&g_lock);
    close(fd);
    return 0;
}

int64_t mqtt_broker_run(int64_t port) {
    int srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv < 0) return -1;
    int one = 1; setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in a; memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET; a.sin_addr.s_addr = INADDR_ANY; a.sin_port = htons((unsigned short)port);
    if (bind(srv, (struct sockaddr*)&a, sizeof(a)) < 0) return -2;
    if (listen(srv, 128) < 0) return -3;
    for (;;) {
        int c = accept(srv, 0, 0);
        if (c < 0) continue;
        pthread_t th;
        pthread_create(&th, 0, client_thread, (void*)(long)c);
        pthread_detach(th);
    }
    return 0;
}

/* ---- client ---- */
int64_t mqtt_connect(maca_str host, int64_t port) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return -1;
    struct sockaddr_in a; memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET; a.sin_port = htons((unsigned short)port);
    inet_pton(AF_INET, host ? host : "127.0.0.1", &a.sin_addr);
    if (connect(fd, (struct sockaddr*)&a, sizeof(a)) < 0) { close(fd); return -1; }
    unsigned char cx[] = { 0, 4, 'M', 'Q', 'T', 'T', 4, 2, 0, 60, 0, 0 };
    send_packet(fd, 0x10, cx, sizeof(cx));
    unsigned char b1; read_n(fd, &b1, 1); int rl = read_remlen(fd);
    unsigned char tmp[8]; if (rl > 0 && rl <= 8) read_n(fd, tmp, rl);
    return fd;
}
int64_t mqtt_subscribe(int64_t fd, maca_str topic) {
    unsigned char body[512];
    body[0] = 0; body[1] = 1;               /* packet id 1 */
    int n = 2 + put_str(body + 2, topic);
    body[n++] = 0;                          /* requested qos 0 */
    send_packet((int)fd, 0x82, body, n);
    unsigned char b1; read_n((int)fd, &b1, 1); int rl = read_remlen((int)fd);
    unsigned char tmp[16]; if (rl > 0 && rl <= 16) read_n((int)fd, tmp, rl);
    return 0;
}
int64_t mqtt_publish(int64_t fd, maca_str topic, maca_str payload) {
    unsigned char body[2048];
    int n = put_str(body, topic);
    int pl = (int)strlen(payload ? payload : "");
    if (n + pl < (int)sizeof(body)) { memcpy(body + n, payload, pl); n += pl; }
    send_packet((int)fd, 0x30, body, n);
    return 0;
}
maca_str mqtt_receive(int64_t fd) {
    unsigned char b1;
    while (read_n((int)fd, &b1, 1) == 0) {
        int rl = read_remlen((int)fd);
        if (rl < 0 || rl > 65535) return "";
        unsigned char* body = (unsigned char*)malloc(rl > 0 ? rl : 1);
        if (rl > 0 && read_n((int)fd, body, rl)) { free(body); return ""; }
        if ((b1 >> 4) == 3) {               /* PUBLISH → return payload */
            int tl = (body[0] << 8) | body[1];
            int po = 2 + tl;
            int pl = rl - po;
            char* out = (char*)maca_alloc(pl + 1);
            if (pl > 0) memcpy(out, body + po, pl);
            out[pl > 0 ? pl : 0] = 0;
            free(body);
            return out;
        }
        free(body);
    }
    return "";
}
int64_t mqtt_disconnect(int64_t fd) { send_packet((int)fd, 0xE0, 0, 0); close((int)fd); return 0; }
"#;

pub fn write_mqtt_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_mqtt.c"), MQTT_GLUE)?;
    Ok(())
}

/// `modules/http` engine (for `import c "http.h"`): an HTTP/1.1 server.
pub const HTTP_GLUE: &str = r##"#define _GNU_SOURCE
#include "maca_runtime.h"
#include <stdio.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <arpa/inet.h>
#include <signal.h>
#include <ctype.h>

/* A request larger than this is answered 413 rather than buffered. A server
   that grows its buffer to whatever a client sends is a server anyone can take
   down from one socket. */
#define HTTP_MAX_REQUEST (1 << 20)

typedef struct { int fd; maca_closure handler; } http_conn;

static int http_write_all(int fd, const char* b, size_t n) {
    size_t off = 0;
    while (off < n) {
        ssize_t w = write(fd, b + off, n - off);
        if (w <= 0) return -1;
        off += (size_t)w;
    }
    return 0;
}

/* How long a connection may stay silent before it is dropped, in seconds. A
   thread per connection is only affordable while connections end: without a
   timeout, one socket that connects and says nothing holds a thread forever,
   which is a denial of service anybody can mount from a laptop. */
#define HTTP_IDLE_SECONDS 30

/* The end of the headers, or -1 while it has not arrived yet. */
static long header_end(const char* b, size_t n) {
    for (size_t i = 0; i + 3 < n; i++) {
        if (b[i] == '\r' && b[i+1] == '\n' && b[i+2] == '\r' && b[i+3] == '\n') return (long)(i + 4);
    }
    /* tolerate bare LF, which hand-written clients and `printf | nc` produce */
    for (size_t i = 0; i + 1 < n; i++) {
        if (b[i] == '\n' && b[i+1] == '\n') return (long)(i + 2);
    }
    return -1;
}

/* `Content-Length`, or 0 when absent. Case-insensitive: the header name is not
   the client's to spell. */
static long content_length(const char* head, size_t n) {
    static const char* key = "content-length:";
    for (size_t i = 0; i + 15 < n; i++) {
        if (i && head[i-1] != '\n') continue;
        size_t k = 0;
        while (k < 15 && (char)tolower((unsigned char)head[i+k]) == key[k]) k++;
        if (k != 15) continue;
        size_t j = i + 15;
        while (j < n && (head[j] == ' ' || head[j] == '\t')) j++;
        if (j < n && (head[j] == '-' || head[j] == '+')) return -1;   /* not a length */
        if (j >= n || head[j] < '0' || head[j] > '9') return -1;      /* nor is nothing */
        long v = 0;
        while (j < n && head[j] >= '0' && head[j] <= '9') {
            /* Overflow is not a large request, it is a malformed one: wrapping
               made the server wait for a body that would never arrive. */
            if (v > (long)HTTP_MAX_REQUEST) return -1;
            v = v * 10 + (head[j] - '0');
            j++;
        }
        return v;
    }
    return 0;
}

/* Is the request framed in a way this server understands?

   `Transfer-Encoding` is not implemented, and a request that uses it cannot be
   framed by `Content-Length`, and guessing produces a body that is really the
   chunk headers, and behind a proxy that is a request-smuggling primitive. So
   it is refused rather than misread. */
static int has_transfer_encoding(const char* head, size_t n) {
    for (size_t i = 0; i + 18 < n; i++) {
        if (i && head[i-1] != '\n') continue;
        if (strncasecmp(head + i, "transfer-encoding:", 18) == 0) return 1;
    }
    return 0;
}

/* One connection's read buffer. It outlives a single request, because a client
   may send two in one packet and the second one's bytes arrive while the first
   is still being read, and dropping them answered the first request and hung on
   the second. */
typedef struct { char* buf; size_t cap, len; } http_buf;

/* Why `read_request` stopped. */
typedef enum {
    REQ_OK = 0, REQ_CLOSED, REQ_TOO_LARGE, REQ_UNSUPPORTED, REQ_MALFORMED
} req_status;

/* Read one whole request out of `b`, leaving anything past it in place for the
   next call. `*out` is the request text (caller frees). */
static req_status read_request(int fd, http_buf* b, char** out) {
    for (;;) {
        long head = header_end(b->buf, b->len);
        if (head >= 0) {
            if (has_transfer_encoding(b->buf, (size_t)head)) return REQ_UNSUPPORTED;
            long want = content_length(b->buf, (size_t)head);
            if (want < 0) return REQ_MALFORMED;   /* a length that is not one */
            if (head + want > (long)HTTP_MAX_REQUEST) return REQ_TOO_LARGE;
            if ((long)b->len >= head + want) {
                size_t take = (size_t)(head + want);
                char* req = (char*)malloc(take + 1);
                if (!req) return REQ_CLOSED;
                memcpy(req, b->buf, take);
                req[take] = 0;
                memmove(b->buf, b->buf + take, b->len - take);
                b->len -= take;
                *out = req;
                return REQ_OK;
            }
        }
        if (b->len >= HTTP_MAX_REQUEST) return REQ_TOO_LARGE;
        if (b->len + 1 >= b->cap) {
            size_t cap = b->cap ? b->cap * 2 : 8192;
            char* g = (char*)realloc(b->buf, cap);
            if (!g) return REQ_CLOSED;
            b->buf = g; b->cap = cap;
        }
        ssize_t r = read(fd, b->buf + b->len, b->cap - b->len - 1);
        if (r <= 0) return REQ_CLOSED;
        b->len += (size_t)r;
        b->buf[b->len] = 0;
    }
}

/* Does this message's `Connection` header say `close`?

   Matched at the start of a header line and stopping at the blank line, not as
   a substring: `X-Upstream-Connection: close` is a different header, and it
   used to close a connection whose real header said keep-alive. A body that
   happens to contain the text is likewise not a header. */
static int says_close(const char* msg) {
    for (const char* p = msg; *p; p++) {
        if (p != msg && p[-1] != '\n') continue;
        if (p[0] == '\r' || p[0] == '\n') break;          /* end of headers */
        if (strncasecmp(p, "connection:", 11) != 0) continue;
        const char* v = p + 11;
        while (*v == ' ' || *v == '\t') v++;
        return strncasecmp(v, "close", 5) == 0;
    }
    return 0;
}

/* A canned reply for the requests that never reach a handler. */
static void http_refuse(int fd, const char* line, const char* body) {
    char out[256];
    int n = snprintf(out, sizeof(out),
                     "HTTP/1.1 %s\r\nContent-Type: text/plain; charset=utf-8\r\n"
                     "Content-Length: %d\r\nConnection: close\r\n\r\n%s",
                     line, (int)strlen(body), body);
    if (n > 0) http_write_all(fd, out, (size_t)n);
}

static void* http_client(void* arg) {
    http_conn* c = (http_conn*)arg;
    int fd = c->fd;
    maca_closure handler = c->handler;
    free(c);
    /* A silent or stalled client releases its thread instead of holding it. */
    struct timeval tv;
    tv.tv_sec = HTTP_IDLE_SECONDS; tv.tv_usec = 0;
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv));

    http_buf b; b.buf = NULL; b.cap = 0; b.len = 0;
    for (;;) {
        char* req = NULL;
        req_status st = read_request(fd, &b, &req);
        if (st == REQ_TOO_LARGE) {
            http_refuse(fd, "413 Payload Too Large", "413 Payload Too Large\n");
            break;
        }
        if (st == REQ_UNSUPPORTED) {
            http_refuse(fd, "501 Not Implemented", "501 Not Implemented\n");
            break;
        }
        if (st == REQ_MALFORMED) {
            http_refuse(fd, "400 Bad Request", "400 Bad Request\n");
            break;
        }
        if (st != REQ_OK) break;
        /* The client's own wish, read before the request is handed over. */
        int client_done = says_close(req);
        maca_str reply = (maca_str)(intptr_t)maca_call1(handler, (int64_t)(intptr_t)req);
        free(req);
        if (!reply) break;
        if (http_write_all(fd, reply, strlen(reply))) break;
        if (client_done || says_close(reply)) break;
    }
    free(b.buf);
    close(fd);
    return NULL;
}

/* Bind and listen. Returns the socket, or a negative code naming the step that
   failed so the caller can say something better than "it didn't work". */
int64_t http_listen(int64_t port) {
    int srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv < 0) return -1;
    int one = 1;
    setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    struct sockaddr_in a;
    memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_addr.s_addr = INADDR_ANY;
    a.sin_port = htons((unsigned short)port);
    if (bind(srv, (struct sockaddr*)&a, sizeof(a)) < 0) { close(srv); return -2; }
    if (listen(srv, 512) < 0) { close(srv); return -3; }
    return srv;
}

/* Serve until the process ends. A write to a client that vanished raises
   SIGPIPE, whose default action is to kill the process, so one disconnecting
   client would take the server with it. */
int64_t http_accept_loop(int64_t srv, maca_closure handler) {
    signal(SIGPIPE, SIG_IGN);
    for (;;) {
        int fd = accept((int)srv, 0, 0);
        if (fd < 0) continue;
        int one = 1;
        setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof(one));
        http_conn* c = (http_conn*)malloc(sizeof(http_conn));
        if (!c) { close(fd); continue; }
        c->fd = fd;
        c->handler = handler;
        pthread_t th;
        if (pthread_create(&th, 0, http_client, c) != 0) { free(c); close(fd); continue; }
        pthread_detach(th);
    }
    return 0;
}

/* One request, for a client and for the tests: send `request` to host:port and
   return the whole reply. */
maca_str http_fetch(maca_str host, int64_t port, maca_str request) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) return "";
    struct sockaddr_in a;
    memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_port = htons((unsigned short)port);
    inet_pton(AF_INET, host && *host ? host : "127.0.0.1", &a.sin_addr);
    if (connect(fd, (struct sockaddr*)&a, sizeof(a)) < 0) { close(fd); return ""; }
    if (http_write_all(fd, request, strlen(request))) { close(fd); return ""; }
    shutdown(fd, SHUT_WR);
    size_t cap = 8192, n = 0;
    char* buf = (char*)malloc(cap);
    if (!buf) { close(fd); return ""; }
    for (;;) {
        if (n + 1 >= cap) {
            cap *= 2;
            char* g = (char*)realloc(buf, cap);
            if (!g) { free(buf); close(fd); return ""; }
            buf = g;
        }
        ssize_t r = read(fd, buf + n, cap - n - 1);
        if (r <= 0) break;
        n += (size_t)r;
    }
    buf[n] = 0;
    close(fd);
    return buf;
}
"##;

pub fn write_http_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_http.c"), HTTP_GLUE)?;
    Ok(())
}
