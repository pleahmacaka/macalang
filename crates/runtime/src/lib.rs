//! maca-runtime: the C runtime linked into every native binary.
//!
//! Phase 4: `minconsole`, heap strings + a string builder, a typed dynamic
//! array macro, a small robust JSON parser, file/dir helpers. Perceus RC
//! (Phase 5) will replace the current leak-on-purpose allocation; a run-once
//! CLI never frees, which is fine until then.
//!
//! Style note: defensive C in the spirit of sqlite — every allocation checked,
//! bounds respected, no undefined behavior on malformed input (parse errors
//! abort with a message rather than corrupt memory).

/// `maca_runtime.h` — declarations + the array macro used by generated code.
pub const RUNTIME_H: &str = r##"#ifndef MACA_RUNTIME_H
#define MACA_RUNTIME_H
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef const char* maca_str;

/* ---- allocator (Phase 5): size-tracked blocks, a free-list for reuse, and a
   registry drained at exit so a run-once binary is valgrind-clean. Not full
   Perceus (no codegen-inserted dup/drop yet) — arrays return their old buffer
   to the free-list on growth, which the next same-size request reuses. ---- */
void maca_init(void);          /* installs atexit(shutdown); call first in main */
void maca_shutdown(void);      /* frees every live block */
void* maca_alloc(size_t n);
void* maca_realloc(void* p, size_t n);
void maca_drop(void* p);       /* return a block to the reuse free-list */
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
/* `fail msg` — print "error: <msg>" to stderr and exit(1) (unhandled error) */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noreturn))
#endif
void maca_fail(maca_str s);
void maca_print(maca_str s);
maca_str maca_input(void);

/* ---- strings ---- */
maca_str maca_concat(maca_str a, maca_str b);
maca_str maca_from_int(int64_t n);
maca_str maca_from_float(double d);
maca_str maca_from_bool(bool b);
bool maca_str_eq(maca_str a, maca_str b);
maca_str maca_join(maca_str* data, int64_t len, maca_str sep);

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
int64_t maca_json_int(maca_json* j);
double maca_json_float(maca_json* j);
bool maca_json_bool(maca_json* j);
maca_str maca_json_str(maca_json* j);

/* ---- typed dynamic array (monomorphized by generated code) ---- */
#define MACA_DEFINE_ARRAY(Name, Elem)                                          \
    typedef struct { Elem* data; int64_t len; int64_t cap; } Name;             \
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
    }

#endif
"##;

/// `maca_runtime.c`
pub const RUNTIME_C: &str = r##"#define _GNU_SOURCE
#include "maca_runtime.h"
#include <stdio.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>

static void die(const char* msg) {
    fputs("maca runtime error: ", stderr);
    fputs(msg, stderr);
    fputc('\n', stderr);
    exit(1);
}

/* ---- allocator: header-tracked blocks + reuse free-list + exit drain ---- */
typedef struct maca_hdr { size_t size; struct maca_hdr* fl_next; } maca_hdr;
static maca_hdr** g_live = NULL;
static size_t g_live_len = 0, g_live_cap = 0;
static maca_hdr* g_freelist = NULL;
static uint64_t g_alloc_count = 0, g_reuse_count = 0;

void maca_init(void) { atexit(maca_shutdown); }
void maca_shutdown(void) {
    for (size_t i = 0; i < g_live_len; i++) free(g_live[i]);
    free(g_live);
    g_live = NULL; g_live_len = 0; g_live_cap = 0; g_freelist = NULL;
}
void* maca_alloc(size_t n) {
    for (maca_hdr** pp = &g_freelist; *pp; pp = &(*pp)->fl_next) {
        maca_hdr* h = *pp;
        if (h->size >= n) { *pp = h->fl_next; g_reuse_count++; return (void*)(h + 1); }
    }
    maca_hdr* h = (maca_hdr*)malloc(sizeof(maca_hdr) + n);
    if (!h) die("out of memory");
    h->size = n; h->fl_next = NULL;
    if (g_live_len == g_live_cap) {
        g_live_cap = g_live_cap ? g_live_cap * 2 : 64;
        g_live = (maca_hdr**)realloc(g_live, g_live_cap * sizeof(maca_hdr*));
        if (!g_live) die("out of memory");
    }
    g_live[g_live_len++] = h;
    g_alloc_count++;
    return (void*)(h + 1);
}
void maca_drop(void* p) {
    if (!p) return;
    maca_hdr* h = ((maca_hdr*)p) - 1;
    h->fl_next = g_freelist; g_freelist = h;
}
void* maca_realloc(void* p, size_t n) {
    if (!p) return maca_alloc(n);
    maca_hdr* h = ((maca_hdr*)p) - 1;
    if (h->size >= n) return p;
    void* np = maca_alloc(n);
    memcpy(np, p, h->size);
    maca_drop(p);
    return np;
}
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
void maca_fail(maca_str s) {
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
maca_str maca_concat(maca_str a, maca_str b) {
    if (!a) a = ""; if (!b) b = "";
    size_t la = strlen(a), lb = strlen(b);
    char* r = (char*)xmalloc(la + lb + 1);
    memcpy(r, a, la); memcpy(r + la, b, lb); r[la + lb] = '\0';
    return r;
}
maca_str maca_from_int(int64_t n) {
    char* r = (char*)xmalloc(24);
    snprintf(r, 24, "%lld", (long long)n);
    return r;
}
maca_str maca_from_float(double d) {
    char* r = (char*)xmalloc(32);
    snprintf(r, 32, "%g", d);
    return r;
}
maca_str maca_from_bool(bool b) { return b ? "true" : "false"; }
bool maca_str_eq(maca_str a, maca_str b) {
    if (a == b) return true;
    if (!a || !b) return false;
    return strcmp(a, b) == 0;
}
maca_str maca_join(maca_str* data, int64_t len, maca_str sep) {
    maca_sb sb; maca_sb_init(&sb);
    for (int64_t i = 0; i < len; i++) {
        if (i) maca_sb_puts(&sb, sep);
        maca_sb_puts(&sb, data[i]);
    }
    return maca_sb_finish(&sb);
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
    if (!a || !*a) return b;
    if (!b || !*b) return a;
    size_t la = strlen(a);
    bool slash = a[la - 1] == '/';
    return maca_concat(a, slash ? b : maca_concat("/", b));
}
maca_str maca_dirs_data(void) {
    const char* x = getenv("XDG_DATA_HOME");
    if (x && *x) return x;
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
int64_t maca_json_int(maca_json* j) { return j && j->kind == MJ_NUM ? (int64_t)j->num : 0; }
double maca_json_float(maca_json* j) { return j && j->kind == MJ_NUM ? j->num : 0.0; }
bool maca_json_bool(maca_json* j) { return j && j->kind == MJ_BOOL ? j->b : false; }
maca_str maca_json_str(maca_json* j) { return j && j->kind == MJ_STR && j->str ? j->str : ""; }
"##;

/// `maca_async.h` — the concurrency runtime interface. Always includable; the
/// implementation (`maca_async.c`) is only linked when a program uses async, so
/// a purely sequential binary carries no scheduler symbols.
pub const ASYNC_H: &str = r##"#ifndef MACA_ASYNC_H
#define MACA_ASYNC_H
#include <stdint.h>

/* bounded parallel map over int64: ordered results, at most `max_conc` threads */
int64_t* maca_parallel_i64(int64_t* xs, int64_t n, int64_t (*f)(int64_t), int max_conc);

/* structured cancellation: workers poll a token; a demo proves they stop. */
typedef struct { volatile int flag; } maca_cancel;
void maca_cancel_set(maca_cancel* c);
int maca_cancel_check(maca_cancel* c);
int64_t maca_cancel_demo(int64_t workers);

#endif
"##;

/// `maca_async.c` — colorblind-async slice: a bounded worker pool (stackful
/// fibers + io_uring are the eventual target; POSIX threads carry the model for
/// now) and a cancellation token. Structured: all workers join before return.
pub const ASYNC_C: &str = r##"#include "maca_async.h"
#include "maca_runtime.h"
#include <pthread.h>

#define MACA_MAX_THREADS 64

typedef struct { int64_t* xs; int64_t* out; int64_t from, to; int64_t (*f)(int64_t); } pslice;
static void* pworker(void* arg) {
    pslice* s = (pslice*)arg;
    for (int64_t i = s->from; i < s->to; i++) s->out[i] = s->f(s->xs[i]);
    return NULL;
}
int64_t* maca_parallel_i64(int64_t* xs, int64_t n, int64_t (*f)(int64_t), int max_conc) {
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
    /* loop forever until cancelled — if cancellation is broken, this hangs */
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

/// FFI binding glue for `import c "sqlite3.h"`. These thin wrappers own the
/// C-side pointers (`sqlite3*`, `sqlite3_stmt*`) that Maca surface syntax can't
/// express, and expose plain `str`/`int` signatures. Real libsqlite3 does the
/// work. (Auto-generating bindings from a clang header parse is future work.)
pub const SQLITE_GLUE: &str = r#"#include "maca_runtime.h"
#include <sqlite3.h>

static sqlite3* g_db = 0;

int64_t sqlite_open(maca_str path) { return sqlite3_open(path, &g_db); }

int64_t sqlite_exec(maca_str sql) {
    char* err = 0;
    int rc = sqlite3_exec(g_db, sql, 0, 0, &err);
    if (err) sqlite3_free(err);
    return rc;
}

maca_str sqlite_query1(maca_str sql) {
    sqlite3_stmt* st = 0;
    if (sqlite3_prepare_v2(g_db, sql, -1, &st, 0) != SQLITE_OK) return "";
    maca_str out = "";
    if (sqlite3_step(st) == SQLITE_ROW) {
        const unsigned char* t = sqlite3_column_text(st, 0);
        if (t) {
            size_t n = strlen((const char*)t);
            char* b = (char*)maca_alloc(n + 1);
            memcpy(b, t, n + 1);
            out = b;
        }
    }
    sqlite3_finalize(st);
    return out;
}

int64_t sqlite_close(void) { int rc = sqlite3_close(g_db); g_db = 0; return rc; }
"#;

pub fn write_sqlite_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_sqlite.c"), SQLITE_GLUE)?;
    Ok(())
}

/// FFI binding glue for `import py "module"`. Embeds CPython — **feature-gated**
/// because it links libpython (a much larger, dynamic binary). `py_call(m, f)`
/// imports module `m`, calls its no-arg function `f`, and returns `str(result)`.
pub const PY_GLUE: &str = r#"#define PY_SSIZE_T_CLEAN
#include <Python.h>
#include "maca_runtime.h"

maca_str py_call(maca_str module, maca_str func) {
    Py_Initialize();
    maca_str out = "<py error>";
    PyObject* m = PyImport_ImportModule(module);
    if (m) {
        PyObject* f = PyObject_GetAttrString(m, func);
        if (f && PyCallable_Check(f)) {
            PyObject* r = PyObject_CallObject(f, 0);
            if (r) {
                PyObject* s = PyObject_Str(r);
                if (s) {
                    const char* c = PyUnicode_AsUTF8(s);
                    if (c) { size_t n = strlen(c); char* b = (char*)maca_alloc(n + 1); memcpy(b, c, n + 1); out = b; }
                    Py_DECREF(s);
                }
                Py_DECREF(r);
            }
            Py_XDECREF(f);
        }
        Py_DECREF(m);
    }
    Py_Finalize();
    return out;
}
"#;

pub fn write_py_glue(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(dir.join("maca_ffi_py.c"), PY_GLUE)?;
    Ok(())
}

/// `std/mqtt` engine (for `import c "mqtt.h"`): a minimal MQTT 3.1.1 broker +
/// client over TCP — CONNECT/CONNACK, SUBSCRIBE/SUBACK, PUBLISH (QoS 0),
/// PINGREQ, and `+`/`#` topic wildcards. Threaded (one thread per client) so
/// the broker serves many concurrent clients. Sockets/pthreads are in musl
/// libc, so this links static-musl with no external library.
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
