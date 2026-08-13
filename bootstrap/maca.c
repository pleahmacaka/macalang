#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdarg.h>
#include <ctype.h>
#include <unistd.h>
#include <sys/stat.h>
#include <dirent.h>
#include <time.h>
#include <stdint.h>
#include <strings.h>
#include <sys/time.h>
typedef float f32x4 __attribute__((vector_size(16)));
typedef float f32x8 __attribute__((vector_size(32)));
typedef double f64x2 __attribute__((vector_size(16)));
typedef double f64x4 __attribute__((vector_size(32)));
typedef int i32x4 __attribute__((vector_size(16)));
typedef int i32x8 __attribute__((vector_size(32)));
typedef struct maca_hdr { long size; long used; } maca_hdr;
static long maca_allocs = 0;
static long* maca_cells(long bytes) { long n = bytes > 0 ? bytes : (long)sizeof(long); maca_hdr* h = (maca_hdr*)malloc(sizeof(maca_hdr) + (size_t)n); if (!h) { fputs("out of memory\n", stderr); exit(1); } h->size = n; h->used = n / (long)sizeof(long); maca_allocs++; return (long*)(h + 1); }
static long maca_alloc_count(void) { return maca_allocs; }
static long maca_reuse_count(void) { return 0; }
static char* maca_cat(const char* a, const char* b) { char* r = malloc(strlen(a) + strlen(b) + 1); strcpy(r, a); strcat(r, b); return r; }
static void maca_drop_str(const char* s) { free((void*)(uintptr_t)s); }
static char* maca_cat_own(const char* a, const char* b, int own) { char* r = maca_cat(a, b); if (own & 1) maca_drop_str(a); if (own & 2) maca_drop_str(b); return r; }
static int maca_say(FILE* f, const char* s, const char* end, int own) { fprintf(f, "%s%s", s ? s : "", end); if (own) maca_drop_str(s); return 0; }
static char* maca_int_to_str(long n) { char* r = malloc(24); snprintf(r, 24, "%ld", n); return r; }
static char* maca_float_to_str(double x) { char* r = malloc(32); if (x == (double)(long long)x && x < 1e15 && x > -1e15) snprintf(r, 32, "%.1f", x); else snprintf(r, 32, "%g", x); return r; }
static char* maca_fixed(double x, int n) { if (n < 0) n = 0; if (n > 17) n = 17; int need = snprintf(NULL, 0, "%.*f", n, x); char* r = malloc((size_t)need + 1); snprintf(r, (size_t)need + 1, "%.*f", n, x); return r; }
static const char* maca_bool_to_str(int b) { return b ? "true" : "false"; }
static char* maca_upper(const char* s) { size_t n = strlen(s); char* r = malloc(n + 1); for (size_t i = 0; i < n; i++) r[i] = toupper((unsigned char)s[i]); r[n] = 0; return r; }
typedef struct { long* data; int len; } MacaList;
typedef struct { void* fn; MacaList env; } MacaFn;
typedef struct { MacaList keys; MacaList vals; } MacaMap;
static MacaMap maca_map_new(void) { MacaMap m; m.keys.data = NULL; m.keys.len = 0; m.vals.data = NULL; m.vals.len = 0; return m; }
static int maca_map_at(MacaMap m, const char* k) { for (int i = 0; i < m.keys.len; i++) if (strcmp((const char*)m.keys.data[i], k) == 0) return i; return -1; }
static int maca_map_has(MacaMap m, const char* k) { return maca_map_at(m, k) >= 0; }
static MacaList maca_list_sorted(MacaList a, int kind);
static MacaList maca_map_keys(MacaMap m) { return maca_list_sorted(m.keys, 1); }
static MacaList maca_map_vals(MacaMap m) { return m.vals; }
static long maca_map_or(MacaMap m, const char* k, long d) { int i = maca_map_at(m, k); return i < 0 ? d : m.vals.data[i]; }
static long maca_map_get(MacaMap m, const char* k) { return maca_map_or(m, k, 0); }
static MacaMap maca_map_remove(MacaMap m, const char* k) { int at = maca_map_at(m, k); if (at < 0) return m; MacaMap r; r.keys.len = m.keys.len - 1; r.vals.len = r.keys.len; r.keys.data = maca_cells((r.keys.len ? r.keys.len : 1) * sizeof(long)); r.vals.data = maca_cells((r.vals.len ? r.vals.len : 1) * sizeof(long)); int w = 0; for (int i = 0; i < m.keys.len; i++) { if (i == at) continue; r.keys.data[w] = m.keys.data[i]; r.vals.data[w] = m.vals.data[i]; w++; } return r; }
static MacaMap maca_map_set(MacaMap m, const char* k, long v) { int i = maca_map_at(m, k); MacaMap r; if (i >= 0) { r = m; r.vals.data = maca_cells((m.vals.len ? m.vals.len : 1) * sizeof(long)); memcpy(r.vals.data, m.vals.data, m.vals.len * sizeof(long)); r.vals.data[i] = v; return r; } r.keys.len = m.keys.len + 1; r.vals.len = m.vals.len + 1; r.keys.data = maca_cells(r.keys.len * sizeof(long)); r.vals.data = maca_cells(r.vals.len * sizeof(long)); memcpy(r.keys.data, m.keys.data, m.keys.len * sizeof(long)); memcpy(r.vals.data, m.vals.data, m.vals.len * sizeof(long)); r.keys.data[m.keys.len] = (long)k; r.vals.data[m.vals.len] = v; return r; }
static MacaList maca_listv(int n, ...) { MacaList l; l.data = maca_cells(n * sizeof(long)); l.len = n; va_list ap; va_start(ap, n); for (int i = 0; i < n; i++) l.data[i] = va_arg(ap, long); va_end(ap); return l; }
static MacaList maca_list_cat(MacaList a, MacaList b) { MacaList l; l.len = a.len + b.len; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data, a.len * sizeof(long)); memcpy(l.data + a.len, b.data, b.len * sizeof(long)); return l; }
static MacaList maca_list_pushed(MacaList a, long v) { maca_hdr* h = a.data ? ((maca_hdr*)a.data) - 1 : 0; long room = h ? h->size / (long)sizeof(long) : 0; if (!h || a.len != h->used || a.len >= room) { long want = a.len * 2 > 8 ? a.len * 2 : 8; long* g = maca_cells(want * (long)sizeof(long)); if (a.len > 0) memcpy(g, a.data, (size_t)a.len * sizeof(long)); a.data = g; h = ((maca_hdr*)g) - 1; } a.data[a.len] = v; a.len++; h->used = a.len; return a; }
static MacaList maca_list_slice(MacaList a, int lo, int hi) { if (lo < 0) lo = 0; if (hi > a.len) hi = a.len; if (hi < lo) hi = lo; MacaList l; l.len = hi - lo; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data + lo, l.len * sizeof(long)); return l; }
static int maca_list_index_of(MacaList a, long v) { for (int i = 0; i < a.len; i++) if (a.data[i] == v) return i; return -1; }
static int maca_list_index_of_str(MacaList a, const char* v) { for (int i = 0; i < a.len; i++) if (strcmp((const char*)a.data[i], v) == 0) return i; return -1; }
static int maca_str_index_of(const char* a, const char* b) { const char* p = strstr(a, b); return p ? (int)(p - a) : -1; }
static char* maca_str_slice(const char* s, int from, int to) { int n = (int)strlen(s); if (from < 0) from = 0; if (to > n) to = n; if (to < from) to = from; int m = to - from; char* r = malloc(m + 1); memcpy(r, s + from, m); r[m] = 0; return r; }
static int maca_ends_with(const char* s, const char* suf) { size_t n = strlen(s), m = strlen(suf); return m <= n && memcmp(s + n - m, suf, m) == 0; }
static char* maca_list_join(MacaList a, const char* sep) { size_t n = 1; for (int i = 0; i < a.len; i++) n += strlen((const char*)a.data[i]) + strlen(sep); char* r = malloc(n); r[0] = 0; for (int i = 0; i < a.len; i++) { if (i) strcat(r, sep); strcat(r, (const char*)a.data[i]); } return r; }
static long maca_box(int n, const void* p) { void* r = malloc(n); memcpy(r, p, n); return (long)r; }
static MacaList maca_chars(const char* s) { int n = (int)strlen(s); MacaList l; l.len = n; l.data = maca_cells((n ? n : 1) * sizeof(long)); for (int i = 0; i < n; i++) { char* c = malloc(2); c[0] = s[i]; c[1] = 0; l.data[i] = (long)c; } return l; }
static MacaList maca_args(int argc, char** argv) { MacaList l; l.len = argc - 1; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); for (int i = 0; i < l.len; i++) l.data[i] = (long)argv[i + 1]; return l; }
static char* maca_read_file(const char* path) { struct stat st; if (stat(path, &st) != 0 || !S_ISREG(st.st_mode)) { char* e = malloc(1); e[0] = 0; return e; } FILE* f = fopen(path, "rb"); if (!f) { char* e = malloc(1); e[0] = 0; return e; } fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET); char* r = malloc(n + 1); size_t got = fread(r, 1, n, f); r[got] = 0; fclose(f); return r; }
static int maca_write_file(const char* path, const char* text) { FILE* f = fopen(path, "wb"); if (!f) return 0; fputs(text, f); fclose(f); return 1; }
static char* maca_str_at(const char* s, int i) { char* r = malloc(2); r[0] = (i >= 0 && i < (int)strlen(s)) ? s[i] : 0; r[1] = 0; return r; }
static MacaList maca_range(int lo, int hi) { MacaList l; l.len = hi > lo ? hi - lo : 0; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); for (int i = 0; i < l.len; i++) l.data[i] = lo + i; return l; }
static MacaList maca_list_reverse(MacaList a) { MacaList l = maca_list_slice(a, 0, a.len); for (int i = 0; i < l.len / 2; i++) { long t = l.data[i]; l.data[i] = l.data[l.len - 1 - i]; l.data[l.len - 1 - i] = t; } return l; }
static int maca_cmp_cell(const void* a, const void* b) { long x = *(const long*)a, y = *(const long*)b; return (x > y) - (x < y); }
static int maca_cmp_cell_str(const void* a, const void* b) { return strcmp((const char*)*(const long*)a, (const char*)*(const long*)b); }
static int maca_cmp_cell_float(const void* a, const void* b) { double x = *(double*)*(const long*)a, y = *(double*)*(const long*)b; return (x > y) - (x < y); }
static MacaList maca_list_sorted(MacaList a, int kind) { MacaList l = maca_list_slice(a, 0, a.len); if (l.len > 1) qsort(l.data, (size_t)l.len, sizeof(long), kind == 1 ? maca_cmp_cell_str : kind == 2 ? maca_cmp_cell_float : maca_cmp_cell); return l; }
static MacaList maca_list_set(MacaList a, int at, long v) { MacaList l = maca_list_slice(a, 0, a.len); if (at >= 0 && at < l.len) l.data[at] = v; return l; }
static MacaList maca_list_insert(MacaList a, int at, long v) { if (at < 0) at = 0; if (at > a.len) at = a.len; MacaList l; l.len = a.len + 1; l.data = maca_cells((size_t)l.len * sizeof(long)); memcpy(l.data, a.data, (size_t)at * sizeof(long)); l.data[at] = v; memcpy(l.data + at + 1, a.data + at, (size_t)(a.len - at) * sizeof(long)); return l; }
static MacaList maca_list_remove(MacaList a, int at) { if (at < 0 || at >= a.len) return maca_list_slice(a, 0, a.len); MacaList l; l.len = a.len - 1; l.data = maca_cells((size_t)(l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data, (size_t)at * sizeof(long)); memcpy(l.data + at, a.data + at + 1, (size_t)(l.len - at) * sizeof(long)); return l; }
static char* maca_trim(const char* s) { const char* a = s; while (*a == ' ' || *a == '\t' || *a == '\n' || *a == '\r') a++; const char* b = s + strlen(s); while (b > a && (b[-1] == ' ' || b[-1] == '\t' || b[-1] == '\n' || b[-1] == '\r')) b--; size_t n = (size_t)(b - a); char* r = malloc(n + 1); memcpy(r, a, n); r[n] = 0; return r; }
static char* maca_lower(const char* s) { size_t n = strlen(s); char* r = malloc(n + 1); for (size_t i = 0; i < n; i++) r[i] = tolower((unsigned char)s[i]); r[n] = 0; return r; }
static int maca_starts_with(const char* s, const char* p) { size_t n = strlen(p); return strlen(s) >= n && memcmp(s, p, n) == 0; }
static char* maca_substr(const char* s, int at, int n) { return maca_str_slice(s, at, at + (n < 0 ? 0 : n)); }
static char* maca_repeat(const char* s, int n) { if (n < 0) n = 0; size_t len = strlen(s), total = len * (size_t)n; char* r = malloc(total + 1); for (int i = 0; i < n; i++) memcpy(r + (size_t)i * len, s, len); r[total] = 0; return r; }
static char* maca_replace(const char* s, const char* from, const char* to) { if (!*from) return maca_cat(s, ""); size_t lf = strlen(from), lt = strlen(to), hits = 0; for (const char* p = strstr(s, from); p; p = strstr(p + lf, from)) hits++; char* r = malloc(strlen(s) + hits * (lt > lf ? lt - lf : 0) + 1); char* w = r; const char* p = s; for (;;) { const char* hit = strstr(p, from); if (!hit) { strcpy(w, p); break; } memcpy(w, p, (size_t)(hit - p)); w += hit - p; memcpy(w, to, lt); w += lt; p = hit + lf; } return r; }
static char* maca_pad(const char* s, int w, const char* p, int at_start) { if (!*p) p = " "; size_t len = strlen(s); if (w <= 0 || (size_t)w <= len) return maca_cat(s, ""); size_t fill = (size_t)w - len, pl = strlen(p); char* r = malloc((size_t)w + 1); char* into = at_start ? r : r + len; for (size_t i = 0; i < fill; i++) into[i] = p[i % pl]; memcpy(at_start ? r + fill : r, s, len); r[(size_t)w] = 0; return r; }
static char* maca_pad_start(const char* s, int w, const char* p) { return maca_pad(s, w, p, 1); }
static char* maca_pad_end(const char* s, int w, const char* p) { return maca_pad(s, w, p, 0); }
static char* maca_pad_center(const char* s, int w, const char* p) { if (!*p) p = " "; size_t len = strlen(s); if (w <= 0 || (size_t)w <= len) return maca_cat(s, ""); size_t fill = (size_t)w - len, left = fill / 2, pl = strlen(p); char* r = malloc((size_t)w + 1); for (size_t i = 0; i < left; i++) r[i] = p[i % pl]; memcpy(r + left, s, len); for (size_t i = left + len; i < (size_t)w; i++) r[i] = p[(i - left - len) % pl]; r[(size_t)w] = 0; return r; }
static MacaList maca_split(const char* s, const char* sep) { MacaList l; int cap = 8; l.len = 0; l.data = maca_cells(cap * sizeof(long)); if (!*sep) { l.data[l.len++] = (long)maca_cat(s, ""); return l; } size_t ls = strlen(sep); const char* p = s; for (;;) { if (l.len == cap) { cap *= 2; long* d = maca_cells(cap * sizeof(long)); memcpy(d, l.data, (size_t)l.len * sizeof(long)); l.data = d; } const char* hit = strstr(p, sep); if (!hit) { l.data[l.len++] = (long)maca_str_slice(p, 0, (int)strlen(p)); break; } l.data[l.len++] = (long)maca_str_slice(p, 0, (int)(hit - p)); p = hit + ls; } return l; }
static int maca_failed_count = 0;
static int maca_failures(void) { return maca_failed_count; }
static int maca_assert(int cond, const char* msg) { if (cond) return 1; maca_failed_count++; fprintf(stderr, "assertion failed: %s\n", msg && *msg ? msg : "(no message)"); return 0; }
static int maca_assert_eq(const char* got, const char* want, const char* msg) { if (!got) got = ""; if (!want) want = ""; if (strcmp(got, want) == 0) return 1; maca_failed_count++; fprintf(stderr, "assertion failed: %s\n  got:  %s\n  want: %s\n", msg && *msg ? msg : "(no message)", got, want); return 0; }
static char* maca_chr(int b) { char* r = malloc(2); r[0] = (b > 0 && b < 256) ? (char)b : 0; r[1] = 0; return r; }
static int maca_ord(const char* s) { return (s && s[0]) ? (int)(unsigned char)s[0] : -1; }
static char* maca_env(const char* name) { const char* v = getenv(name); return maca_cat(v ? v : "", ""); }
static char* maca_cwd(void) { char* r = malloc(4096); if (!getcwd(r, 4096)) r[0] = 0; return r; }
static int maca_chdir(const char* p) { return chdir(p) == 0; }
static int maca_is_tty(void) { return isatty(1); }
static int maca_file_exists(const char* p) { struct stat st; return stat(p, &st) == 0; }
static int maca_is_dir(const char* p) { struct stat st; return stat(p, &st) == 0 && S_ISDIR(st.st_mode); }
static long maca_file_size(const char* p) { struct stat st; return stat(p, &st) == 0 ? (long)st.st_size : -1; }
static long maca_modified_ms(const char* p) { struct stat st; return stat(p, &st) == 0 ? (long)(st.st_mtime * 1000) : -1; }
static int maca_make_dir(const char* p) { char* d = maca_cat(p, ""); for (char* q = d + 1; *q; q++) if (*q == '/') { *q = 0; mkdir(d, 0777); *q = '/'; } mkdir(d, 0777); return maca_is_dir(d); }
static int maca_remove_file(const char* p) { return unlink(p) == 0; }
static int maca_remove_dir(const char* p) { DIR* d = opendir(p); if (!d) return 0; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, ".") == 0 || strcmp(it->d_name, "..") == 0) continue; char* c = maca_cat(p, maca_cat("/", it->d_name)); if (maca_is_dir(c)) maca_remove_dir(c); else maca_remove_file(c); } closedir(d); return rmdir(p) == 0; }
static int maca_copy_bytes(const char* src, const char* dst) { FILE* a = fopen(src, "rb"); if (!a) return 0; FILE* b = fopen(dst, "wb"); if (!b) { fclose(a); return 0; } char buf[8192]; size_t n; while ((n = fread(buf, 1, sizeof buf, a)) > 0) fwrite(buf, 1, n, b); fclose(a); fclose(b); return 1; }
static MacaList maca_list_dir(const char* p) { MacaList l; l.len = 0; int cap = 16; l.data = maca_cells((size_t)cap * sizeof(long)); DIR* d = opendir(p); if (!d) return l; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, ".") == 0 || strcmp(it->d_name, "..") == 0) continue; if (l.len == cap) { cap *= 2; long* g = maca_cells((size_t)cap * sizeof(long)); memcpy(g, l.data, (size_t)l.len * sizeof(long)); l.data = g; } l.data[l.len++] = (long)maca_cat(it->d_name, ""); } closedir(d); if (l.len > 1) qsort(l.data, (size_t)l.len, sizeof(long), maca_cmp_cell_str); return l; }
static char* maca_real_path(const char* p) { char* r = malloc(4096); if (!realpath(p, r)) return maca_cat(p, ""); return r; }
static char* maca_path_join(const char* a, const char* b) { if (!*a) return maca_cat(b, ""); if (!*b) return maca_cat(a, ""); return a[strlen(a) - 1] == '/' ? maca_cat(a, b) : maca_cat(a, maca_cat("/", b)); }
static long maca_now_ms(void) { struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts); return (long)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000); }
static char* maca_now_iso(void) { time_t t = time(NULL); struct tm g; gmtime_r(&t, &g); char* r = malloc(32); strftime(r, 32, "%Y-%m-%dT%H:%M:%SZ", &g); return r; }
static char* maca_format_time(long ms, const char* fmt) { time_t t = (time_t)(ms / 1000); struct tm g; gmtime_r(&t, &g); char* r = malloc(128); if (!strftime(r, 128, fmt, &g)) r[0] = 0; return r; }
static void maca_sleep_ms(int ms) { if (ms > 0) usleep((unsigned)ms * 1000); }
static char* maca_input(const char* prompt) { if (prompt && *prompt) { printf("%s", prompt); fflush(stdout); } size_t cap = 128, n = 0; char* b = malloc(cap); int c; while ((c = fgetc(stdin)) != EOF && c != '\n') { if (n + 1 >= cap) { cap *= 2; char* g = malloc(cap); memcpy(g, b, n); b = g; } b[n++] = (char)c; } b[n] = 0; return b; }
static int maca_at_eof(void) { int c = fgetc(stdin); if (c == EOF) return 1; ungetc(c, stdin); return 0; }
static char* maca_attr(const char* name, const char* value) { if (!name || !*name) return maca_cat("", ""); size_t n = strlen(name), v = strlen(value); char* r = malloc(n + v * 6 + 5); char* w = r; *w++ = ' '; memcpy(w, name, n); w += n; *w++ = '='; *w++ = '"'; for (size_t i = 0; i < v; i++) { char c = value[i]; if (c == '&') { memcpy(w, "&amp;", 5); w += 5; } else if (c == '<') { memcpy(w, "&lt;", 4); w += 4; } else if (c == '>') { memcpy(w, "&gt;", 4); w += 4; } else if (c == '"') { memcpy(w, "&quot;", 6); w += 6; } else { *w++ = c; } } *w++ = '"'; *w = 0; return r; }
static char* maca_flag(const char* name, int on) { if (!on || !name || !*name) return maca_cat("", ""); return maca_cat(" ", name); }
static int maca_void_tag(const char* t) { const char* v[] = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", 0}; for (int i = 0; v[i]; i++) if (strcmp(t, v[i]) == 0) return 1; return 0; }
static char* maca_element(const char* tag, const char* attrs, const char* kids) { size_t t = strlen(tag), a = strlen(attrs), k = strlen(kids); char* r = malloc(t * 2 + a + k + 6); char* w = r; *w++ = '<'; memcpy(w, tag, t); w += t; memcpy(w, attrs, a); w += a; *w++ = '>'; if (!maca_void_tag(tag)) { memcpy(w, kids, k); w += k; *w++ = '<'; *w++ = '/'; memcpy(w, tag, t); w += t; *w++ = '>'; } *w = 0; return r; }
#define MACA_STYLES "*,*::before,*::after{box-sizing:border-box}\nhtml,body{margin:0}\n.absolute { position:absolute; }\n.block { display:block; }\n.border { border-width:1px;border-style:solid; }\n.fixed { position:fixed; }\n.h\\[i-1\\] { height:i-1; }\n.h\\[j\\] { height:j; }\n.inline { display:inline; }\n.lowercase { text-transform:lowercase; }\n.p\\[-1\\] { padding:-1; }\n.p\\[0\\] { padding:0; }\n.rounded { border-radius:0.25rem; }\n.static { position:static; }\n.table { display:table; }\n"
static int maca_fail(const char* s) { fprintf(stderr, "error: %s\n", s ? s : ""); exit(1); return 0; }
#include <sys/wait.h>
static int maca_exec(const char* cmd, MacaList args) { char** argv = malloc((args.len + 2) * sizeof(char*)); argv[0] = (char*)cmd; for (int i = 0; i < args.len; i++) argv[i + 1] = (char*)args.data[i]; argv[args.len + 1] = NULL; pid_t pid = fork(); if (pid < 0) return -1; if (pid == 0) { execvp(cmd, argv); _exit(127); } int st = 0; if (waitpid(pid, &st, 0) < 0) return -1; if (WIFEXITED(st)) return WEXITSTATUS(st); if (WIFSIGNALED(st)) return 128 + WTERMSIG(st); return -1; }
static char* maca_capture_fd(const char* cmd, MacaList args, int child_fd) { int fd[2]; if (pipe(fd) < 0) return maca_cat("", ""); char** argv = malloc((size_t)(args.len + 2) * sizeof(char*)); argv[0] = (char*)cmd; for (int i = 0; i < args.len; i++) argv[i + 1] = (char*)args.data[i]; argv[args.len + 1] = NULL; pid_t pid = fork(); if (pid < 0) { close(fd[0]); close(fd[1]); return maca_cat("", ""); } if (pid == 0) { close(fd[0]); dup2(fd[1], child_fd); close(fd[1]); execvp(cmd, argv); _exit(127); } close(fd[1]); size_t cap = 4096, n = 0; char* b = malloc(cap); ssize_t got; while ((got = read(fd[0], b + n, cap - n - 1)) > 0) { n += (size_t)got; if (n + 1 >= cap) { cap *= 2; char* g = malloc(cap); memcpy(g, b, n); b = g; } } close(fd[0]); int st = 0; waitpid(pid, &st, 0); b[n] = 0; return b; }
static char* maca_capture(const char* cmd, MacaList args) { return maca_capture_fd(cmd, args, 1); }
static char* maca_capture_err(const char* cmd, MacaList args) { return maca_capture_fd(cmd, args, 2); }
typedef enum { TInt, TFloat, TIdent, TStr, TPath, KwLet, KwIf, KwElse, KwFor, KwIn, KwMatch, KwWhile, KwBreak, KwContinue, KwReturn, KwImport, KwWith, KwFail, KwTry, KwAlias, KwTrue, KwFalse, LParen, RParen, LBracket, RBracket, LBrace, RBrace, Comma, Colon, Dot, DotDot, Ellipsis, Arrow, FatArrow, Eq, EqEq, NotEq, Lt, Gt, Le, Ge, AmpAmp, PipePipe, Plus, Minus, Star, Slash, Percent, PlusPlus, Bar, Shl, Shr, PipeGt, Question, QuestionPost, Bang, Comment, Newline, Eof } Kind;
typedef struct { MacaList tokens; MacaList marks; MacaList errors; long pos; long broke; } Lexed;
typedef enum { EInt, EFloat, EStr, EBool, EIdent, ECall, EField, EBinary, EUnary, ETernary, EIf, EList, ERecord, EWith, ELambda, EBlock, EMatch, EMethod, EGuard, EWhile, EFor, EJump, EAttr, EBad } ExprKind;
typedef enum { SExpr, SBind, SSet, SFn, SRecord, SSum } StmtKind;
typedef struct { MacaList items; MacaList errors; } Module;
typedef struct { MacaList params; long pnext; } PParams;
typedef struct { MacaList bstmts; long bnext; } PBlock;
typedef struct { MacaList aitems; long anext; } PArgs;
typedef struct { const char* tname; long tnext; } PType;
typedef enum { KInt, KFloat, KStr, KBool, KBytes, KUnit, KAny, KError, KVar, KCon, KFn, KRec, KOpt } TyKind;
typedef struct { MacaList subst; } Infer;
typedef struct { const char* code; const char* message; const char* note; long pos; } Diagnostic;
typedef struct { MacaList items; long n; } Lift;
typedef struct { const char* base; MacaList names; MacaList texts; } Assets;
typedef struct { const char* name; MacaList tys; } Want;
typedef struct { MacaList gens; MacaList wants; MacaList done; } Poly;
typedef struct { const char* name; const char* triple; const char* cpu; const char* flash; long flash_k; const char* ram; long ram_k; } Mcu;
typedef struct { const char* name; long start; long end; long depth; long parent; long closed; } Span;
typedef struct { const char* label; MacaList spans; } Trace;
typedef struct { long base; long total; long cols; } Scale;
typedef struct { MacaList seen; MacaList names; MacaList owners; MacaList asks; MacaList toks; MacaList errs; MacaList unknown; } Unit;
typedef struct { const char* module; const char* named; const char* path; } Entry;
typedef struct { const char* kind; const char* spec; const char* names; const char* text; } PageAsset;
typedef struct { const char* kind; const char* file; const char* why; } Found;
typedef struct { const char* name; const char* path; } Bin;
typedef struct { MacaList walked; MacaList pairs; } Asked;
typedef struct { MacaList fns; MacaList own; MacaList edges; MacaList costs; long total; } Dump;
typedef struct { const char* to; long ir; } Call;
typedef struct { long depth; long x; long parent; } Frame;
typedef struct { const char* name; const char* kind; const char* pkg; const char* req; } Spec;
typedef struct { const char* version; const char* tarball; const char* integrity; const char* commit; const char* why; } Pin;
typedef struct { const char* name; const char* spec; } Dep;
typedef struct { const char* name; const char* block; } Pkg;
typedef struct { long major; long minor; long patch; long ok; } Ver;
typedef struct { Kind kind; const char* text; long pos; long fresh; } Token;
typedef struct { ExprKind kind; const char* text; long ival; const char* ty; MacaList children; MacaList stmts; } Expr;
typedef struct { TyKind kind; const char* name; long slot; MacaList args; MacaList labels; long open_mc; } Ty;
typedef struct { Infer infer; const char* error; } Unify;
typedef struct { Infer infer; MacaList tys; } Batch;
typedef struct { MacaList names; MacaList types; MacaList fns; MacaList sigs; MacaList fields; MacaList ftypes; MacaList ctors; MacaList owners; MacaList slots; MacaList varargs; MacaList frozen; MacaList errors; Infer infer; MacaList holes; MacaList fills; long here; } Env;
typedef struct { MacaList nodes; Lift at; } LiftedAll;
typedef struct { MacaList stmts; Lift at; } LiftedBody;
typedef struct { MacaList nodes; Poly at; } PolyAll;
typedef struct { MacaList stmts; Poly at; } PolyBody;
typedef struct { Dump d; const char* here; const char* callee; } Scan;
typedef struct { StmtKind kind; const char* name; Expr value; const char* ret; long pos; MacaList params; MacaList body; long frozen; } Stmt;
typedef struct { Expr node; long next; } PExpr;
typedef struct { Infer infer; Ty ty; } Instance;
typedef struct { MacaList slots; Ty ty; } Scheme;
typedef struct { Env env; Ty ty; } Typed;
typedef struct { Env env; MacaList tys; } Signature;
typedef struct { Expr node; Lift at; } Lifted;
typedef struct { Expr node; Poly at; } PolyNode;
typedef struct { Stmt snode; long snext; } PStmt;
static const const char* ElementTags = " html head title meta link style script body div span p pre code a button input textarea select option label form header footer main section article nav aside ul ol li table thead tbody tr td th h1 h2 h3 h4 h5 h6 img svg canvas small strong em b i hr br blockquote figure figcaption details summary dialog progress meter video audio ";
static const char* NixosRoots;
static const const char* AllEffects = " io net os async exn ";
static const const char* KnownTargets = " native c js jvm rust tauri embedded nix all ";
static const const char* TwFixed = "|flex=display:flex|inline-flex=display:inline-flex|grid=display:grid|block=display:block|inline=display:inline|inline-block=display:inline-block|hidden=display:none|flex-col=flex-direction:column|flex-row=flex-direction:row|flex-wrap=flex-wrap:wrap|flex-1=flex:1 1 0%|flex-auto=flex:1 1 auto|flex-none=flex:none|items-center=align-items:center|items-start=align-items:flex-start|items-end=align-items:flex-end|items-baseline=align-items:baseline|items-stretch=align-items:stretch|justify-center=justify-content:center|justify-between=justify-content:space-between|justify-start=justify-content:flex-start|justify-end=justify-content:flex-end|justify-around=justify-content:space-around|self-start=align-self:flex-start|self-end=align-self:flex-end|self-center=align-self:center|self-stretch=align-self:stretch|text-center=text-align:center|text-left=text-align:left|text-right=text-align:right|font-bold=font-weight:700|font-semibold=font-weight:600|font-medium=font-weight:500|font-normal=font-weight:400|font-sans=font-family:'Pretendard GOV Variable',sans-serif|font-mono=font-family:'JetBrainsMono Nerd Font',monospace|uppercase=text-transform:uppercase|lowercase=text-transform:lowercase|italic=font-style:italic|underline=text-decoration-line:underline|whitespace-pre=white-space:pre|whitespace-pre-wrap=white-space:pre-wrap|whitespace-nowrap=white-space:nowrap|whitespace-normal=white-space:normal|truncate=overflow:hidden;text-overflow:ellipsis;white-space:nowrap|break-words=overflow-wrap:break-word|break-all=word-break:break-all|break-keep=word-break:keep-all|overflow-auto=overflow:auto|overflow-hidden=overflow:hidden|overflow-x-auto=overflow-x:auto|overflow-y-auto=overflow-y:auto|relative=position:relative|absolute=position:absolute|fixed=position:fixed|sticky=position:sticky|static=position:static|list-none=list-style:none|list-disc=list-style:disc|list-decimal=list-style:decimal|border-collapse=border-collapse:collapse|table=display:table|table-auto=table-layout:auto|table-fixed=table-layout:fixed|text-inherit=color:inherit|border-separate=border-collapse:separate|align-top=vertical-align:top|align-middle=vertical-align:middle|align-baseline=vertical-align:baseline|font-serif=font-family:ui-serif,Georgia,serif|no-underline=text-decoration-line:none|shrink-0=flex-shrink:0|grow=flex-grow:1|content-none=content:\"\"|shadow=box-shadow:0 1px 3px rgba(0,0,0,.1)|shadow-md=box-shadow:0 4px 8px rgba(0,0,0,.12)|shadow-lg=box-shadow:0 4px 14px rgba(0,0,0,.14)|shadow-none=box-shadow:none|overscroll-contain=overscroll-behavior:contain|appearance-none=appearance:none|inset-0=top:0;right:0;bottom:0;left:0|cursor-pointer=cursor:pointer|cursor-default=cursor:default|resize-none=resize:none|outline-none=outline:none|pointer-events-none=pointer-events:none|pointer-events-auto=pointer-events:auto|select-none=user-select:none|box-border=box-sizing:border-box|box-content=box-sizing:content-box|tabular-nums=font-variant-numeric:tabular-nums|w-full=width:100%|h-full=height:100%|w-screen=width:100vw|h-screen=height:100vh|w-auto=width:auto|h-auto=height:auto|min-h-0=min-height:0|min-w-0=min-width:0|max-w-full=max-width:100%|rounded-none=border-radius:0|rounded=border-radius:0.25rem|rounded-md=border-radius:0.375rem|rounded-lg=border-radius:0.5rem|rounded-xl=border-radius:0.75rem|rounded-full=border-radius:9999px|border=border-width:1px;border-style:solid|border-0=border-width:0|border-t=border-top-width:1px;border-top-style:solid|border-b=border-bottom-width:1px;border-bottom-style:solid|border-l=border-left-width:1px;border-left-style:solid|border-r=border-right-width:1px;border-right-style:solid|border-x=border-left-width:1px;border-right-width:1px;border-left-style:solid;border-right-style:solid|border-y=border-top-width:1px;border-bottom-width:1px;border-top-style:solid;border-bottom-style:solid|";
static const const char* TwColor = "|white=#ffffff|black=#000000|transparent=transparent|current=currentColor|zinc-50=#fafafa|zinc-100=#f4f4f5|zinc-200=#e4e4e7|zinc-300=#d4d4d8|zinc-400=#a1a1aa|zinc-500=#71717a|zinc-600=#52525b|zinc-700=#3f3f46|zinc-800=#27272a|zinc-850=#1f1f23|zinc-900=#18181b|zinc-950=#0d0d0f|violet-300=#c4b5fd|violet-400=#a78bfa|violet-500=#8b5cf6|violet-600=#7c3aed|cyan-400=#22d3ee|emerald-400=#34d399|emerald-500=#10b981|rose-400=#fb7185|rose-500=#f43f5e|amber-400=#fbbf24|amber-500=#f59e0b|sky-400=#38bdf8|";
static const const char* TwProp = "|text=font-size|w=width|h=height|min-w=min-width|min-h=min-height|max-w=max-width|max-h=max-height|bg=background-color|border=border-color|p=padding|px=padding-inline|py=padding-block|pt=padding-top|pb=padding-bottom|pl=padding-left|pr=padding-right|m=margin|mx=margin-inline|my=margin-block|mt=margin-top|mb=margin-bottom|ml=margin-left|mr=margin-right|gap=gap|gap-x=column-gap|gap-y=row-gap|top=top|right=right|bottom=bottom|left=left|inset=inset|scroll-mt=scroll-margin-top|scroll-mb=scroll-margin-bottom|leading=line-height|font=font-family|content=content|shadow=box-shadow|grid-cols=grid-template-columns|";
static const const char* TwText = "|xs=0.75rem|sm=0.875rem|base=1rem|lg=1.125rem|xl=1.25rem|2xl=1.5rem|3xl=1.875rem|";
static const const char* TwLead = "|none=1|tight=1.25|snug=1.375|normal=1.5|relaxed=1.625|loose=2|";
static const const char* TwTrack = "|tight=-0.025em|normal=0|wide=0.025em|wider=0.05em|widest=0.1em|";
static const const char* TwWide = "|xs=20rem|sm=24rem|md=28rem|lg=32rem|xl=36rem|2xl=42rem|3xl=48rem|4xl=56rem|5xl=64rem|6xl=72rem|7xl=80rem|prose=65ch|none=none|";
static const const char* TwEdge = "|border-l=left|border-r=right|border-t=top|border-b=bottom|border-x=left right|border-y=top bottom|";
static const const char* TwPseudo = "|hover=:hover|focus=:focus|active=:active|first=:first-child|last=:last-child|open=[open]|before=::before|after=::after|marker=::marker|details-marker=::-webkit-details-marker|placeholder=::placeholder|";
static const const char* TwMedia = "|dark=(prefers-color-scheme:dark)|sm=(min-width:40rem)|md=(min-width:48rem)|lg=(min-width:64rem)|xl=(min-width:80rem)|max-sm=(max-width:40rem)|max-md=(max-width:48rem)|max-lg=(max-width:64rem)|";
static const const char* TwEscaped = "/.:[](),#%'\"!$&*+;<=>?@^`{|}~";
static const const char* TwSide = "|pt=top|pr=right|pb=bottom|pl=left|mt=top|mr=right|mb=bottom|ml=left|";
static const const char* TwReset = "*,*::before,*::after{box-sizing:border-box}\\nhtml,body{margin:0}\\n";
static const const char* Reserved = " auto break case char const continue default do double else enum extern float for goto if inline int long register restrict return short signed sizeof static struct switch typedef union unsigned void volatile while bool class new delete this template typeof unix linux i386 abs bind clock close div exit free index link listen log open pow read remove stat strlen time write ";
static const const char* Fresh = " maca_attr maca_cat maca_cat_own maca_chr maca_cwd maca_element maca_env maca_fixed maca_flag maca_float_to_str maca_format_time maca_input maca_int_to_str maca_last_fail maca_list_join maca_lower maca_now_iso maca_pad_center maca_pad_end maca_pad_start maca_path_join maca_read_file maca_real_path maca_repeat maca_replace maca_str_at maca_str_slice maca_substr maca_trim maca_upper ";
static const const char* RustReserved = " as async await become box break const continue crate do dyn else enum extern false final fn for if impl in let loop macro match mod move mut override priv pub ref return Self static struct super trait true try type typeof union unsafe unsized use virtual where while yield ";
static const const char* JsReserved = " arguments await case catch class const debugger default delete do enum eval export extends finally function implements in instanceof interface let new null package private protected public static super switch this throw typeof var void yield ";
static const const char* JsPreamble = "\nfunction _mstr(v) {\n  if (v === null || v === undefined) return \"\";\n  if (Array.isArray(v)) return \"[\" + v.map(_mstr).join(\", \") + \"]\";\n  if (typeof v === \"object\") return typeof v.$ === \"string\" ? v.$ : JSON.stringify(v);\n  return String(v);\n}\nfunction _meq(a, b) {\n  if (a === b) return true;\n  if (a === null || b === null || typeof a !== \"object\" || typeof b !== \"object\") return false;\n  if (Array.isArray(a) !== Array.isArray(b)) return false;\n  const ka = Object.keys(a);\n  if (ka.length !== Object.keys(b).length) return false;\n  return ka.every((k) => _meq(a[k], b[k]));\n}\nfunction _mhas(x, v) { return typeof x === \"string\" ? x.includes(v) : x.some((e) => _meq(e, v)); }\nfunction _msubstr(s, a, n) { return s.slice(a, a + n); }\nfunction _mpad(s, w, p, mode) {\n  const n = w - s.length;\n  if (n <= 0) return s;\n  const fill = (c) => (p || \" \").repeat(c).slice(0, c);\n  if (mode === 0) return fill(n) + s;\n  if (mode === 1) return s + fill(n);\n  const l = Math.floor(n / 2);\n  return fill(l) + s + fill(n - l);\n}\nfunction _mclass(s, kind) {\n  const c = s[0] || \"\";\n  if (kind === 0) return /\\s/.test(c);\n  if (kind === 1) return c >= \"0\" && c <= \"9\";\n  return /[A-Za-z]/.test(c);\n}\nfunction _mfold(xs, init, f) { let a = init; for (const x of xs) a = f(a, x); return a; }\nfunction _mcmp(a, b) { return typeof a === \"number\" ? a - b : a < b ? -1 : a > b ? 1 : 0; }\nfunction _msort(xs) { return [...xs].sort(_mcmp); }\nfunction _msortby(xs, key) { return [...xs].sort((a, b) => _mcmp(key(a), key(b))); }\nfunction _mpick(xs, dir) { return xs.reduce((a, b) => (_mcmp(b, a) * dir > 0 ? b : a)); }\nfunction _mlast(xs) { return xs[xs.length - 1]; }\nfunction _mset(xs, i, v) { const r = xs.slice(); if (i >= 0 && i < r.length) r[i] = v; return r; }\nfunction _mins(xs, i, v) { const r = xs.slice(); r.splice(Math.max(0, Math.min(i, r.length)), 0, v); return r; }\nfunction _mrem(xs, i) { const r = xs.slice(); if (i >= 0 && i < r.length) r.splice(i, 1); return r; }\nfunction _mrange(a, b) { const r = []; for (let i = a; i < b; i++) r.push(i); return r; }\nfunction _mor(m, k, d) { return m.has(k) ? m.get(k) : d; }\nfunction _mdel(m, k) { const r = new Map(m); r.delete(k); return r; }\nfunction _mchr(b) { return b > 0 && b < 1114112 ? String.fromCharCode(b) : \"\"; }\nfunction _mord(s) { return s && s.length ? s.charCodeAt(0) : -1; }\nfunction _mgcd(a, b) { a = Math.abs(a); b = Math.abs(b); while (b) { const t = a % b; a = b; b = t; } return a; }\nfunction _mint(x) { return typeof x === \"string\" ? Math.trunc(parseFloat(x.trim())) || 0 : Math.trunc(Number(x)); }\nfunction _msleep(ms) { const t = Date.now() + ms; while (Date.now() < t); return 0; }\nfunction _mfail(m) { throw new Error(_mstr(m)); }\nfunction _mtry(f) { try { f(); return \"\"; } catch (e) { return _mstr(e && e.message ? e.message : e); } }\nfunction _minfo(x) { console.log(_mstr(x)); return 0; }\nfunction _merr(x) { console.error(_mstr(x)); return 0; }\nfunction _mprint(x) { const s = _mstr(x); if (typeof process !== \"undefined\" && process.stdout) process.stdout.write(s); else console.log(s); return 0; }\nfunction _mesc(v) { return String(v).split(\"&\").join(\"&amp;\").split(\"<\").join(\"&lt;\").split(\">\").join(\"&gt;\").split('\"').join(\"&quot;\"); }\nfunction maca_attr(n, v) { return n ? \" \" + n + \"=\" + '\"' + _mesc(v) + '\"' : \"\"; }\nfunction maca_flag(n, on) { return on && n ? \" \" + n : \"\"; }\nconst _mvoid = \"area base br col embed hr img input link meta source track wbr\".split(\" \");\nfunction maca_element(t, a, k) { return _mvoid.includes(t) ? \"<\" + t + a + \">\" : \"<\" + t + a + \">\" + k + \"</\" + t + \">\"; }\nlet _mfailed = 0;\nfunction _mfailures() { return _mfailed; }\nfunction _massert(c, m) { if (c) return true; _mfailed++; console.error(\"assertion failed: \" + _mstr(m)); return false; }\nfunction _massert_eq(a, b, m) { if (_meq(a, b)) return true; _mfailed++; console.error(\"assertion failed: \" + _mstr(m) + \"\\n  got:  \" + _mstr(a) + \"\\n  want: \" + _mstr(b)); return false; }\n";
static const const char* NixUser = "alice";
static const const char* EmbedPreamble = "/* generated by maca --target embedded: freestanding, no libc */\n#include <stdint.h>\n\nstatic inline void maca_delay(uint32_t n) { while (n) { __asm__ volatile(\"nop\"); n--; } }\n";
static const const char* EmbedStartup = "\n/* ---- Cortex-M startup ---- */\nextern uint32_t _sdata, _edata, _sidata, _sbss, _ebss, _estack;\nvoid main(void);\n\nvoid Reset_Handler(void) {\n    uint32_t *src = &_sidata, *dst = &_sdata;\n    while (dst < &_edata) *dst++ = *src++;\n    for (dst = &_sbss; dst < &_ebss;) *dst++ = 0;\n    main();\n    for (;;) { __asm__ volatile(\"wfi\"); }\n}\n\n__attribute__((section(\".isr_vector\"), used))\nvoid (* const g_vectors[])(void) = {\n    (void (*)(void)) &_estack,\n    Reset_Handler,\n};\n";
static const const char* JavaReserved = " abstract assert boolean break byte case catch char class const continue default do double else enum extends final finally float for goto if implements import instanceof int interface long native new package private protected public return short static strictfp super switch synchronized this throw throws transient try var void volatile while ";
static const const char* Indent = "    ";
static const const char* Version = "0.3.3";
static const long WatchPollMs = 300;
static const const char* EntryDir = ".maca";
static const const char* CargoName = "maca_app";
static const const char* NpmPrefix = "npm:";
static const const char* PackageJson = "package.json";
static const const char* Manifest = "maca.toml";
static const const char* SpecDoc = "docs/SPEC.md";
static const const char* SpecHeading = "## Language cheatsheet";
static const long SpecBudget = 15000;
static const const char* Carried = "a package this tree carries";
static const long ProfileWidth = 1080;
static const long ProfileDepth = 40;
static const long ProfileRows = 12;
static const const char* DevFile = "dev.maca";
static const char* StarterDev;
static const const char* DepsDir = "maca_modules";
static const const char* LockFile = "maca.lock";
static const const char* NpmRegistry = "https://registry.npmjs.org";
static const const char* MacaRegistry = "https://registry.maca.dev";
static const const char* Releases = "https://api.github.com/repos/pleahmacaka/macalang/releases/latest";
static const const char* ReleasePage = "https://github.com/pleahmacaka/macalang/releases/latest";
static const long JsonWindow = 512;
Kind keyword_kind(const char* w);
Token mk_token(Kind kind, const char* text, long pos);
long is_space(const char* c);
long is_digit(const char* c);
long is_alpha(const char* c);
long is_alnum(const char* c);
long run_end(MacaList cs, long i, MacaFn pred);
const char* span(MacaList cs, long i, long j);
Lexed lexed(MacaList tokens, MacaList errors);
Lexed keep(Lexed acc, Token t);
Lexed mark(Lexed acc, Token t);
Lexed crossed(Lexed acc, const char* c);
Lexed moved(Lexed acc, long i);
Lexed note_error(Lexed acc, const char* msg);
MacaList lex(const char* src);
MacaList lex_marked(const char* src);
Lexed lex_all(const char* src);
Lexed scan(MacaList cs, Lexed out, long hi);
Lexed halves(MacaList cs, Lexed out, long hi, long mid);
Lexed emptied(Lexed acc);
Lexed merged_runs(Lexed out, Lexed left, Lexed right);
Lexed end_run(Lexed out, long i, long n);
Lexed step(MacaList cs, Lexed out);
Lexed lex_comment(MacaList cs, long i, Lexed out);
long comments(MacaList cs, long i);
long line_end(MacaList cs, long i);
long triple(MacaList cs, long i);
long raw_end(MacaList cs, long i);
Lexed lex_raw(MacaList cs, long i, Lexed out);
const char* escape_raw(MacaList cs, long i, const char* acc);
const char* raw_char(const char* c);
Lexed lex_string(MacaList cs, long i, Lexed out);
long string_end(MacaList cs, long i);
long doubled(MacaList cs, long i, const char* c);
long interp_end(MacaList cs, long i, long depth);
long quoted_end(MacaList cs, long i);
Lexed lex_word(MacaList cs, long i, Lexed out);
long word_end(MacaList cs, long j);
Lexed lex_number(MacaList cs, long i, Lexed out);
long based(MacaList cs, long i);
long is_base_digit(const char* c);
Lexed lex_based(MacaList cs, long i, Lexed out);
long from_base(MacaList cs, long i, long j, long base, long acc);
long digit_value(const char* c);
Lexed lex_float(MacaList cs, long i, long dot, Lexed out);
const char* at_or_blank(MacaList cs, long i);
Lexed lex_punct(MacaList cs, long i, Lexed out, const char* c);
Kind one_char_kind(MacaList cs, long i, const char* c);
long is_two_char(const char* s);
Kind two_char_kind(const char* s);
Kind punct_kind(const char* c);
Lexed unterminated(Lexed out, MacaList cs, long j);
Lexed unknown(Lexed out, const char* c, long i);
Expr e_int(long n);
Expr e_ident(const char* name);
Expr e_bad(const char* text);
Expr e_param(const char* name, const char* ty);
Expr e_typed(const char* name, const char* ty);
Expr with_child(Expr e, Expr c);
Expr e_str(const char* s);
Expr e_bool(const char* text);
Expr e_float(const char* text);
Expr e_call(const char* callee, MacaList args);
Expr e_binary(const char* op, Expr lhs, Expr rhs);
Expr e_ternary(Expr cond, Expr then, Expr els);
Expr e_if(Expr cond, Expr then, Expr els);
Expr e_unary(const char* op, Expr operand);
Expr e_record(const char* tyname, MacaList fields);
Expr e_with(Expr base, MacaList fields);
Expr e_field(Expr base, const char* name);
Expr e_match(MacaList children);
Expr e_method(Expr recv, const char* name, MacaList args);
Expr e_attr(const char* name, Expr value);
Expr e_list(MacaList elems);
Stmt s_expr(Expr value);
Stmt s_bind(const char* name, Expr value);
Stmt s_bind_typed(const char* name, const char* ty, Expr value);
Stmt s_fn(const char* name, const char* ret, MacaList params, MacaList body);
Stmt s_record(const char* name, MacaList fields);
long is_impl_block(Stmt s);
long foreign_type(const char* ty, MacaList own);
const char* head_type(const char* ty);
MacaList declared_types(MacaList items, long i, MacaList acc);
long upper_word(const char* w);
long all_lambda_fields(MacaList fs, long i);
Stmt s_sum(const char* name, MacaList variants);
const char* show(Expr e);
const char* show_args(MacaList xs, long i);
Expr e_guard(Expr pat, Expr when);
Expr e_while(Expr cond, MacaList body);
Expr e_lambda(MacaList params, Expr body);
Expr lambda_body(Expr e);
MacaList lambda_params(Expr e);
Expr e_for(const char* binder, Expr over, MacaList body);
Expr e_jump(const char* word);
Expr e_leave(MacaList value);
Expr e_block(MacaList stmts, Expr last);
Stmt at_pos(Stmt s, long p);
const char* map_type_key(const char* ty);
const char* map_type_val(const char* ty);
const char* map_type_rest(const char* ty);
PExpr mk_pexpr(Expr node, long next);
PStmt mk_pstmt(Stmt snode, long snext);
PParams mk_pparams(MacaList params, long pnext);
PBlock mk_pblock(MacaList bstmts, long bnext);
PArgs mk_pargs(MacaList aitems, long anext);
PStmt parse_fn(MacaList ts, long i);
PType mk_ptype(const char* tname, long tnext);
PType scan_type(MacaList ts, long i);
PType fn_type(MacaList ts, long i, long shut);
const char* fn_type_args(MacaList ts, long i, long shut, const char* acc);
PType type_apply(MacaList ts, long i, const char* head);
long type_arg_at(MacaList ts, long i);
long is_prim_name(const char* w);
long is_tyvar_name(const char* w);
PType type_post(MacaList ts, PType t);
const char* return_type(MacaList ts, long at);
long after_return_type(MacaList ts, long at);
PStmt mk_arrow_fn(MacaList ts, const char* name, const char* ret, MacaList params, long at);
PExpr parse_list_expr(MacaList ts, long i);
PExpr more_elems(MacaList ts, long i, MacaList acc);
PStmt mk_block_fn(MacaList ts, const char* name, const char* ret, MacaList params, long at);
PBlock parse_block(MacaList ts, long i, MacaList acc);
long starts_local_fn(MacaList ts, long i);
PBlock parse_local_fn(MacaList ts, long i, MacaList acc);
Expr body_expr(MacaList stmts);
PBlock parse_bind_stmt(MacaList ts, long at, MacaList acc);
PBlock parse_expr_stmt(MacaList ts, long i, MacaList acc);
PBlock parse_store_stmt(MacaList ts, PExpr target, MacaList acc);
Module parse_module(MacaList ts, long i, MacaList acc);
Module parse_items(MacaList ts, long i, MacaList acc, MacaList bad);
Module stamped(Module m, long at, long p);
Module parse_const_item(MacaList ts, long i, MacaList acc, MacaList bad);
Module parse_path_item(MacaList ts, long i, MacaList acc, MacaList bad);
long binds_a_path(MacaList ts, long i);
long path_end(MacaList ts, long i);
const char* dotted_name(MacaList ts, long i, long end, const char* acc);
const char* bind_type(MacaList ts, long i);
const char* skipped(MacaList ts, long i);
long starts_fn(MacaList ts, long i);
long bind_eq_at(MacaList ts, long i);
long binds_a_name(MacaList ts, long i);
long is_const_word(MacaList ts, long i);
long says_as_const(MacaList ts, long i);
long past_as_const(MacaList ts, long i);
long is_record_decl(MacaList ts, long i);
long typed_fields(MacaList ts, long i);
long is_sum_decl(MacaList ts, long i);
long is_upper(const char* w);
long import_end(MacaList ts, long i);
long selection_end(MacaList ts, long i);
Module parse_sum_decl(MacaList ts, long i, MacaList acc, MacaList bad);
PParams parse_variants(MacaList ts, long i, MacaList acc);
PParams one_variant(MacaList ts, long i, MacaList acc);
PExpr parse_variant(MacaList ts, long i);
PExpr parse_payload(MacaList ts, long i, Expr v);
Module parse_fn_item(MacaList ts, long i, MacaList acc, MacaList bad);
Module parse_record_decl(MacaList ts, long i, MacaList acc, MacaList bad);
PParams parse_fields(MacaList ts, long i, MacaList acc);
PParams parse_one_field(MacaList ts, long i, MacaList acc);
long skip_comma(MacaList ts, long i);
PParams parse_params(MacaList ts, long i, MacaList acc);
PParams parse_one_param(MacaList ts, long i, MacaList acc);
Expr rest_param(MacaList ts, long i);
const char* param_type(MacaList ts, long i);
long after_param_type(MacaList ts, long i);
long prec_of(Kind k);
PExpr parse_expr(MacaList ts, long i);
PExpr parse_ternary(MacaList ts, PExpr cond);
PExpr parse_bin(MacaList ts, PExpr lhs, long min_prec);
PExpr climb(MacaList ts, PExpr lhs, long p, long min_prec);
Expr piped(Expr lhs, Expr rhs);
PExpr parse_primary(MacaList ts, long i);
PExpr parse_postfix(MacaList ts, PExpr e);
PExpr parse_index(MacaList ts, PExpr e);
PExpr parse_with(MacaList ts, PExpr base);
PExpr parse_dot(MacaList ts, PExpr e);
PExpr parse_method(MacaList ts, Expr recv, const char* name, long lparen);
Expr str_node(const char* raw);
long has_interp(MacaList cs, long i);
const char* plain_braces(MacaList cs, long i, const char* acc);
long escaped_brace(MacaList cs, long i);
Expr interp_node(const char* raw);
MacaList split_interp(MacaList cs, long i, long depth, const char* cur, MacaList acc);
MacaList interp_step(MacaList cs, long i, long depth, const char* cur, MacaList acc);
long pair_at(MacaList cs, long i, const char* c);
Expr join_parts(MacaList parts, long i, Expr acc);
Expr add_part(Expr acc, const char* piece, long i);
Expr parse_fragment(const char* src);
Expr formatted(const char* piece);
long spec_start(MacaList cs, long i);
long is_spec_char(const char* c);
long is_align(const char* c);
Expr spec_applied(Expr e, const char* spec);
Expr shown_part(Expr e, const char* prec);
const char* pad_how(const char* align);
PExpr parse_atom(MacaList ts, long i);
PExpr parse_return(MacaList ts, long i);
PExpr parse_try(MacaList ts, long i);
PExpr parse_fail(MacaList ts, long i);
PExpr parse_list(MacaList ts, long i);
PArgs parse_list_elems(MacaList ts, long i, MacaList acc);
PArgs parse_one_elem(MacaList ts, long i, MacaList acc);
long header_brace(MacaList ts, long i, long depth);
Token end_tok();
MacaList cond_tokens(MacaList ts, long i, long b);
PExpr parse_cond(MacaList ts, long i);
PExpr parse_while(MacaList ts, long i);
PExpr parse_for(MacaList ts, long i);
PExpr parse_if(MacaList ts, long i);
PExpr parse_branch(MacaList ts, long brace);
PExpr parse_else(MacaList ts, long i);
PExpr parse_match(MacaList ts, long i);
PArgs parse_arms(MacaList ts, long i, MacaList acc);
PArgs parse_one_arm(MacaList ts, long i, MacaList acc);
PExpr parse_arm_body(MacaList ts, long i);
PExpr parse_commas(MacaList ts, PExpr p);
PExpr gathered(MacaList ts, PExpr p);
PExpr parse_alts(MacaList ts, PExpr p);
PExpr parse_guarded(MacaList ts, PExpr p);
PExpr parse_pattern(MacaList ts, long i);
PExpr parse_cells_pattern(MacaList ts, long i, Expr p);
PExpr parse_fields_pattern(MacaList ts, long i, Expr p);
PExpr parse_binders(MacaList ts, long i, Expr p);
PExpr parse_neg(MacaList ts, long i);
PExpr parse_not(MacaList ts, long i);
PExpr parse_task(MacaList ts, long i, const char* word);
long paren_end(MacaList ts, long i, long depth);
PExpr parse_paren(MacaList ts, long i);
PExpr parse_lambda(MacaList ts, long i);
PExpr parse_lambda_body(MacaList ts, long at);
PExpr lambda_setter(MacaList ts, PExpr lhs);
PExpr parse_call_or_ident(MacaList ts, long i);
PExpr parse_one_lambda(MacaList ts, long i, const char* name);
long opens_record_lit(MacaList ts, long i);
long block_head(MacaList ts, long i);
PExpr parse_record_lit(MacaList ts, long i, const char* name);
PExpr parse_anon_record(MacaList ts, long i);
PArgs parse_lit_fields(MacaList ts, long i, MacaList acc);
PArgs parse_one_lit_field(MacaList ts, long i, MacaList acc);
PExpr lit_value(MacaList ts, long i);
PExpr more_lit_elems(MacaList ts, long i, MacaList acc);
long ends_lit_value(MacaList ts, long i);
PExpr parse_call(MacaList ts, long i, const char* name);
PArgs parse_args(MacaList ts, long i, MacaList acc);
PArgs parse_one_arg(MacaList ts, long i, MacaList acc);
long attr_name_end(MacaList ts, long i);
long name_run_end(MacaList ts, long i);
const char* attr_name(MacaList ts, long i, long end, const char* acc);
Ty bare(TyKind k);
Ty t_int();
Ty t_float();
Ty t_str();
Ty t_bool();
Ty t_bytes();
Ty t_unit();
Ty t_any();
Ty t_error();
long absorbing(Ty t);
Ty t_var(long slot);
Ty t_con(const char* name, MacaList args);
Ty t_array(Ty el);
Ty t_fn(MacaList params, Ty ret);
Ty t_rec(MacaList labels, MacaList types, long open_mc);
Ty t_opt(Ty inner);
MacaList fn_params(Ty t);
Ty fn_ret(Ty t);
Infer new_infer();
Instance fresh(Infer inf);
long unbound(Infer inf, long slot);
Ty resolve(Infer inf, Ty t);
Infer set_slot(Infer inf, long slot, Ty t);
long occurs(Infer inf, long slot, Ty t);
long occurs_in(Infer inf, long slot, MacaList ts, long i);
Infer bind_var(Infer inf, long slot, Ty t);
Unify united(Infer inf);
Unify clashed(Infer inf, Ty x, Ty y);
Unify refused(Infer inf, const char* why);
long shape_disagrees(Ty x, Ty y);
Unify unify(Infer inf, Ty a, Ty b);
Unify unify_all(Infer inf, MacaList xs, MacaList ys, long i);
Unify unify_rows(Infer inf, Ty x, Ty y);
Unify unify_shared(Infer inf, Ty x, Ty y, long i);
const char* unexpected_label(Ty x, Ty y, long i);
Scheme mono(Ty t);
Scheme generalize(Infer inf, Ty t);
MacaList free_slots(Infer inf, Ty t, MacaList seen);
MacaList free_slots_in(Infer inf, MacaList ts, long i, MacaList seen);
Batch fresh_many(Infer inf, long n, MacaList acc);
Instance instantiate(Infer inf, Scheme s);
Ty substitute(MacaList slots, MacaList tos, Ty t);
MacaList substitute_all(MacaList slots, MacaList tos, MacaList ts, long i);
const char* show_ty(Ty t);
const char* show_con(Ty t);
const char* show_fn(Ty t);
const char* show_rec(Ty t);
const char* show_fields(Ty t, long i);
const char* show_joined(MacaList ts, const char* sep, long i);
Env empty_env();
const char* diag_explain(const char* code);
const char* diag_message(Diagnostic d);
Diagnostic diag_at(Stmt s, const char* code, const char* why);
Typed typed(Env env, Ty ty);
Env bind_mc(Env env, const char* name, Ty ty);
Env complain(Env env, const char* why);
Env complain_as(Env env, const char* code, const char* why);
Env note(Env env, const char* why);
Env with_infer(Env env, Infer inf);
Env joined(Env env, Ty want, Ty got);
long is_array_name(const char* name);
Ty ty_named(const char* name);
Ty ty_fn_named(const char* name);
MacaList fn_arg_tys(const char* list, MacaList acc);
long is_type_var(const char* name);
long starts_lower(const char* name);
long sized_number(const char* name);
Typed declared_type(Env env, const char* decl);
Signature param_types(Env env, MacaList ps, long i, MacaList acc);
Env env_of_module(Module m);
Env collect_items(Env env, MacaList items, long i);
Env collect_item(Env env, Stmt s);
Env add_const(Env env, Stmt s);
Env add_fn(Env env, Stmt s);
long rest_before_the_end(MacaList ps, long i);
MacaList rest_taker(Stmt s);
Env add_fields(Env env, const char* rec, MacaList fs, long i);
Env add_ctors(Env env, const char* sum, MacaList vs, long i);
Typed type_in(Env env, Expr e);
Typed lambda_type(Env env, Expr e);
Typed while_type(Env env, Expr e);
Typed for_type(Env env, Expr e);
Env check_stmts(Env env, MacaList stmts, long i);
Typed range_type(Env env, Ty a, Ty b);
Typed shift_type(Env env, Ty a, Ty b);
Typed unary_type(Env env, Expr e);
Typed ident_type(Env env, Expr e);
Typed call_type(Env env, Expr e);
Ty closure_sig(Env env, Expr e);
Env unknown_call(Env env, const char* name);
long is_host_builtin(const char* name);
long is_register_builtin(const char* name);
long callable(Env env, Ty held);
Typed call_local(Env env, Expr e, Ty held);
Signature arg_types(Env env, MacaList args, long i, MacaList acc);
Typed call_declared(Env env, Expr e, Ty sig);
Env noted_shapes(Env env, MacaList want, MacaList got, long i);
Env noted_shape(Env env, Ty want, Ty got);
Ty solved_ty(Env env, Ty t);
MacaList solved_all(Env env, MacaList ts, long i);
long takes_rest(Env env, Expr e, long wanted);
Env wrong_arity(Env env, Expr e, long wanted);
Env unify_args(Env env, MacaList args, MacaList params, long i);
Env walk_args(Env env, MacaList args, long i);
Ty builtin_type(const char* name);
long is_element_tag(const char* name);
long is_prelude_call(const char* name);
long is_str_builtin(const char* name);
long is_float_builtin(const char* name);
long is_int_builtin(const char* name);
long is_bool_builtin(const char* name);
long is_io_builtin(const char* name);
long is_compare(const char* op);
long is_logic(const char* op);
long is_shift(const char* op);
Typed binop_type(Env env, Expr e);
Typed operator_type(Env env, Expr e);
long is_list(Ty t);
long joins(Env env, const char* op, Ty a, Ty b);
Ty concat_type(Env env, Ty a, Ty b);
long numeric(Ty t);
const char* overload_name(const char* op);
long declares_field(Env env, const char* owner, long i);
long is_nominal(Env env, Ty t);
long overload_at(Env env, const char* op, Ty left);
Typed arith_type(Env env, const char* op, Ty a, Ty b);
Typed numeric_type(Env env, Ty a, Ty b);
Typed ternary_type(Env env, Expr e);
Typed block_type(Env env, Expr e);
Typed method_type(Env env, Expr e);
Ty field_fn_ty(Env env, Ty recv, const char* name);
long is_ufcs_call(Env env, Expr e, Ty recv);
long own_method(Ty t);
Typed mapped_type(Env env, Ty recv, Expr f);
Typed sifted_type(Env env, Ty recv, Expr f);
Ty method_result(const char* name, Ty recv);
long is_map_ty(Ty t);
Ty map_method_result(const char* name, Ty recv);
long is_reshaping_method(const char* name);
long is_picking_method(const char* name);
long is_text_method(const char* name);
long is_asking_method(const char* name);
Ty element_of(Ty t);
Typed with_type(Env env, Expr e);
Env check_fields(Env env, const char* rec, MacaList fs, long i);
Env field_set(Env env, const char* rec, Expr f);
Env check_literal(Env env, const char* rec, MacaList fs);
Env missing_fields(Env env, const char* rec, MacaList fs);
MacaList absent_fields(Env env, const char* rec, MacaList fs, long i, MacaList acc);
const char* field_tail(const char* key, const char* rec);
long names_field(MacaList fs, const char* name, long i);
const char* lit_field_name(Expr f);
Env unknown_fields(Env env, const char* rec, MacaList fs, long i);
Typed field_type(Env env, Expr e);
const char* slot_types(Expr v);
const char* binder_type(Expr b);
Typed match_type(Env env, Expr e);
Env check_arms(Env env, Expr e, Ty scrut);
long catches_all(Env env, Expr e, long i);
long wide_arm(Env env, Expr p);
MacaList uncovered(Env env, Expr e, const char* sum, long i, MacaList acc);
long arm_names(Expr e, const char* ctor, long i);
long pat_names(Expr p, const char* ctor);
Env unify_arms(Env env, MacaList cs, Ty want, long i);
Env bound_arm(Env env, Expr pat);
Env named_pattern(Env env, Expr pat);
long starts_upper(const char* name);
const char* ctor_hint(Env env, const char* name);
long hint_span(const char* name);
const char* nearest_ctor(const char* want, MacaList names, long i, const char* found, long best);
long edits_apart(const char* a, const char* b, long cap);
long edit_rows(const char* a, const char* b, MacaList prev, long i);
MacaList edit_cells(const char* a, const char* b, MacaList prev, MacaList cur, long i, long j);
MacaList edit_row(long n, long at, MacaList acc);
long least_of(long a, long b, long c);
Env bound_cells(Env env, Expr pat, Ty el);
Env bind_cell_names(Env env, Expr pat, Ty el, long i);
Env bind_slots_of(Env env, MacaList bs, MacaList tys, long i);
Typed list_type(Env env, Expr e);
Env unify_from(Env env, MacaList cs, Ty want, long i, long step);
Env check_fn_in(Env env, Stmt s);
Ty signature_ret(Env env, const char* name);
MacaList signature_params(Env env, const char* name);
Env bind_params(Env env, MacaList ps, MacaList tys, long i);
Typed check_body(Env env, MacaList stmts, long i, Ty last);
Typed stmt_type(Env env, Stmt s, long is_last);
long branches_for_effect(Expr e);
Env effect_walk(Env env, Expr e);
Env effect_arms(Env env, MacaList cs, long i);
Env extend(Env env, Stmt s, Ty ty);
Env sealed(Env env, Stmt s);
Env return_check(Env env, const char* declared, Ty actual);
Env checked_module(Module m);
long check_module(Module m);
Env check_items(Env env, MacaList items, long i);
Env check_item(Env env, Stmt s);
MacaList check_diagnostics(Module m);
MacaList check_errors(Module m);
MacaList config_errors(MacaList items);
long writes_an_option(MacaList items, long i);
MacaList config_refusals(MacaList items, long i, MacaList acc);
MacaList config_refusal(Stmt s);
MacaList impure_value(Stmt s);
const char* impure_config(const char* name);
const char* effect_of(const char* name);
long is_async_effect(const char* word);
const char* effectful_call(Expr e);
const char* effectful_in(MacaList xs, long i);
MacaList misspelt_option(Stmt s);
const char* option_root(const char* name);
long known_root(const char* root);
MacaList nixos_roots();
MacaList clashing_names(MacaList items, long i, MacaList seen, MacaList acc);
Diagnostic clash_of(Stmt s);
const char* surface_of(Ty t);
const char* con_surface(Ty t);
const char* fn_surface(Ty t);
const char* surface_joined(MacaList ts, long i);
Module annotated(Module m);
MacaList annotate_items(Env env, MacaList items, long i, MacaList acc);
Stmt annotate_item(Env env, Stmt s);
Expr annotated_methods(Env env, Expr e);
MacaList annotated_each(Env env, MacaList fs, long i, MacaList acc);
Expr annotated_method(Env env, Expr f);
const char* item_ret(Env env, Stmt s);
const char* erased_ret(const char* ret, long keep);
MacaList annotate_params(Env env, MacaList ps, MacaList tys, long i, long keep);
Expr annotate_param(Env env, Expr p, MacaList tys, long i, long keep);
Ty grounded(Infer inf, Ty t);
MacaList grounded_all(Infer inf, MacaList ts, long i);
MacaList annotate_body(Env env, MacaList stmts, long i, MacaList acc);
Stmt reassigned(Env env, Stmt s);
Expr annotate_expr(Env env, Expr e);
long holds_fn(Env env, const char* name);
long is_bound_name(Expr e);
long is_field_call(Env env, Expr e);
long is_rest_call(Env env, Expr e);
Expr gathered_rest(Env env, Expr e);
long fixed_arity(Env env, const char* name);
Expr annotated_node(Env env, Expr e);
long is_element_call(Env env, Expr e);
long tag_wins(Env env, Expr e, long declared);
Expr lowered_element(Env env, Expr e);
Expr element_attrs(Env env, MacaList cs, long i, Expr acc);
Expr one_attribute(Env env, Expr a);
Expr element_kids(Env env, MacaList cs, long i, Expr acc);
Expr as_html_text(Env env, Expr v);
long is_str_list(Infer inf, Ty t);
Env looped(Env env, Expr e);
MacaList annotate_children(Env env, Expr e, long i, MacaList acc);
Env child_scope(Env env, Expr e, long i);
Env arm_scope(Env env, Expr scrut, Expr pat);
Env bound_whole(Env env, Expr pat, Ty t);
long whole_binder(const char* name);
Env lambda_scope(Env env, Expr e, long i);
Env bind_each(Env env, MacaList ps, Ty t, long i);
const char* type_of(Expr e);
long count_errors(Expr e);
Module lifted(Module m);
MacaList top_names(MacaList items, long i, MacaList acc);
Module desugared(Module m, const char* src);
const char* embed_dir(const char* path);
long embed_sep(MacaList cs, long i);
long is_text_const(Stmt s);
MacaList text_const_names(MacaList items, long i, MacaList acc);
MacaList text_const_texts(MacaList items, long i, MacaList acc);
long is_data_call(Expr e);
const char* asset_spec(Expr e, Assets at);
const char* data_file(const char* base, const char* spec);
const char* local_spec(const char* spec);
long last_dot(MacaList cs, long i);
const char* asset_file(Expr e, Assets at);
long asset_ok(Expr e, Assets at);
const char* asset_text(Expr e, Assets at);
const char* one_line_text(const char* text);
const char* embed_char(const char* c);
MacaList embed_items(MacaList items, Assets at, long i, MacaList acc);
Stmt embed_stmt(Stmt s, Assets at);
Expr embed_bound(Expr e, const char* want, Assets at);
Expr embed_expr(Expr e, Assets at);
MacaList embed_kids(MacaList xs, Assets at, long i, MacaList acc);
MacaList embed_faults(MacaList items, Assets at, long i, MacaList acc);
MacaList expr_faults(Expr e, Assets at);
MacaList kid_faults(MacaList xs, Assets at, long i, MacaList acc);
MacaList data_faults(Expr e, Assets at);
Lift lift_items(MacaList items, long i, Lift at);
LiftedBody lift_stmt(Stmt s, Lift at);
LiftedBody lift_body(MacaList stmts, long i, MacaList acc, Lift at);
Lifted lift_expr(Expr e, Lift at);
LiftedAll lift_all(MacaList cs, long i, MacaList acc, Lift at, Expr owner);
long held_inline(Expr owner, Expr kid);
long is_attribute_call(Expr owner);
long inlines_lambda(const char* name);
Lifted lift_one(Expr e, Lift at);
Lifted lift_closure(Expr e, Lift at);
MacaList cap_binds(MacaList caps, long i);
MacaList captures_of(Expr e);
MacaList lambda_names(MacaList ps, long i, MacaList acc);
MacaList free_idents(Expr e, MacaList bound, MacaList acc);
MacaList stmt_names(MacaList ss, long i, MacaList acc);
MacaList free_bound(Expr e, MacaList bound);
MacaList free_added(Expr e, MacaList bound, MacaList acc);
MacaList free_names(MacaList acc, long i, MacaList out);
MacaList free_kids(MacaList cs, long i, MacaList bound, MacaList acc);
MacaList free_stmts(MacaList ss, long i, MacaList bound, MacaList acc);
Module named_anons(Module m);
MacaList anon_stmts(MacaList ss, long i);
Stmt anon_stmt(Stmt s);
Expr anon_methods(Expr e);
Expr anon_expr(Expr e);
MacaList anon_kids(MacaList cs, long i);
const char* anon_text(Expr e);
long anon_named_fields(MacaList fs, long i);
long anon_field_named(Expr f);
long anon_word(const char* name);
MacaList anon_tags(MacaList fs, long i, MacaList acc);
const char* anon_tag(Expr f);
long is_anon_lit(Expr e);
MacaList anon_in_stmts(MacaList ss, long i);
MacaList anon_in_stmt(Stmt s);
MacaList anon_in_expr(Expr e);
MacaList anon_in_kids(MacaList cs, long i);
MacaList anon_decls(MacaList es, long i, MacaList seen);
MacaList anon_fields(MacaList fs, long i);
Module monomorphic(Module m);
MacaList generic_items(MacaList items, long i, MacaList acc);
long is_generic_fn(Stmt s);
long any_type_var(MacaList ps, long i);
long has_type_var(const char* name);
long var_in_each(const char* list);
PolyBody mono_items(MacaList items, long i, MacaList acc, Poly at);
PolyBody mono_stmt(Stmt s, Poly at);
PolyBody mono_body(MacaList stmts, long i, MacaList acc, Poly at);
PolyNode mono_expr(Expr e, Poly at);
PolyAll mono_all(MacaList cs, long i, MacaList acc, Poly at);
PolyNode mono_named(Expr e, Poly at);
PolyNode mono_call(Expr e, Stmt g, Poly at);
long generic_at(MacaList gens, const char* name, long i);
MacaList arg_type_names(MacaList cs, long i, MacaList acc);
const char* mangled(Want w);
const char* type_tag(const char* ty);
PolyBody expanded(Env env, Poly at, long i, MacaList acc);
Stmt specialised(Stmt g, Want w, const char* name);
MacaList ground_body(Stmt g, MacaList tys, MacaList ss, long i);
Stmt ground_stmt(Stmt g, MacaList tys, Stmt s);
Expr ground_expr(Stmt g, MacaList tys, Expr e);
MacaList ground_kids(Stmt g, MacaList tys, MacaList cs, long i);
MacaList specialised_params(Stmt g, MacaList ps, MacaList tys, long i);
Expr specialised_param(Stmt g, Expr p, MacaList tys, long i);
const char* specialised_ty(Stmt g, MacaList tys, const char* ty);
const char* specialised_fn(Stmt g, MacaList tys, const char* ty);
const char* specialised_each(Stmt g, MacaList tys, const char* list);
const char* var_type(MacaList ps, MacaList tys, long i, const char* v);
const char* matched_var(const char* declared, const char* concrete, const char* v);
MacaList variadic_errors(MacaList items, long i, MacaList acc);
MacaList rest_misuse(Stmt s);
const char* rest_misuse_why(Stmt s);
MacaList effect_list();
const char* eff_none();
MacaList eff_words(const char* set);
long eff_has(const char* set, const char* name);
const char* eff_add(const char* set, const char* name);
const char* eff_union(const char* a, const char* b);
const char* eff_merge(const char* a, MacaList xs, long i);
long is_known_target(const char* name);
MacaList target_effects(const char* target);
MacaList eff_outside(const char* set, MacaList allowed);
MacaList eff_refused(MacaList all, const char* set, MacaList allowed, long i, MacaList acc);
const char* eff_of_call(const char* name);
const char* eff_of_word(const char* word);
const char* eff_of_method(Expr e);
long is_io_method(const char* name);
const char* node_effects(Expr e, MacaList names, MacaList sets);
const char* expr_effects(Expr e, MacaList names, MacaList sets);
const char* kids_effects(MacaList xs, MacaList names, MacaList sets, long i, const char* acc);
const char* body_effects(MacaList body, MacaList names, MacaList sets, long i, const char* acc);
MacaList effect_fn_names(MacaList items, long i, MacaList acc);
MacaList blank_effects(long n, long i, MacaList acc);
MacaList settled_effects(MacaList items);
MacaList settle_effects(MacaList items, MacaList names, MacaList sets, long fuel);
long same_effects(MacaList a, MacaList b, long i);
MacaList effect_pass(MacaList items, MacaList names, MacaList sets, long i, MacaList acc);
MacaList target_errors(Module m, const char* target);
MacaList kept_borrow_notes(Module m);
MacaList kept_borrow_errors(Module m, const char* target);
MacaList borrow_errors(MacaList items, MacaList own, long i, MacaList acc);
MacaList kept_borrows(Stmt s, MacaList fs, MacaList own, long i, MacaList acc);
MacaList kept_params(Stmt s, const char* method, Expr lam, MacaList ps, MacaList own, long i, MacaList acc);
const char* kept_message(const char* p, const char* method);
long escaping(Expr body, const char* name);
long answered_by(Expr body, const char* name);
long stored_away(Expr e, const char* name);
long field_holds(MacaList fs, const char* name, long i);
long any_mentions(MacaList cs, const char* name, long i);
long any_stores(MacaList cs, const char* name, long i);
long body_stores(MacaList ss, const char* name, long i);
long mentions(Expr e, const char* name);
MacaList effect_errors(Module m, const char* target);
MacaList effect_refusals(MacaList items, MacaList sets, MacaList allowed, const char* target, long i, long k, MacaList acc);
MacaList effect_refusal(Stmt s, const char* set, MacaList allowed, const char* target);
Diagnostic effect_diag(Stmt s, const char* what, const char* target);
const char* target_phrase(const char* target);
MacaList check_diagnostics_on(Module m, const char* target);
MacaList check_errors_on(Module m, const char* target);
const char* tw_look(const char* table, const char* key);
long tw_starts(const char* c, const char* p);
const char* tw_after(const char* c, const char* p);
const char* tw_decl(const char* prop, const char* v);
const char* tw_pair_decl(const char* a, const char* b, const char* v);
long tw_digits(MacaList cs, long i, long acc);
long tw_int(const char* s);
const char* tw_signed(const char* s);
long tw_pow10(long k);
long tw_thou(const char* s);
long tw_thou_parts(long whole, const char* frac);
const char* tw_zeros(const char* s);
const char* tw_pad(long n, long width);
const char* tw_dec(long n);
const char* tw_space(const char* v);
const char* tw_ratio(const char* v);
const char* tw_size(const char* v);
const char* tw_border(MacaList sides, long i, const char* w);
const char* tw_trim_dash(const char* s);
long tw_last_dash(MacaList cs, long i, long at);
const char* tw_arbitrary(const char* c);
const char* tw_bracket(const char* pre, const char* raw);
const char* tw_edges(const char* c);
const char* tw_offsets(const char* c);
const char* tw_track(const char* c);
const char* tw_axis(const char* head, const char* val);
const char* tw_place(const char* axis, const char* part, const char* val);
const char* tw_leading(const char* v);
const char* tw_widest(const char* v);
const char* tw_repeat(const char* prop, const char* v);
const char* tw_body(const char* c);
const char* tw_measured(const char* c);
const char* tw_pixels(const char* prop, const char* v);
const char* tw_placed(const char* c);
const char* tw_scaled(const char* c);
const char* tw_scale(const char* p, const char* v);
const char* tw_box(const char* p);
const char* tw_side_of(const char* p);
const char* tw_axes(const char* p, const char* v);
const char* tw_valued(const char* p, const char* v);
const char* tw_typed(const char* v);
const char* tw_opacity(const char* v);
MacaList tw_parts(const char* c, MacaList acc);
const char* tw_variant(const char* v);
long tw_known(MacaList parts, long i, long n);
const char* tw_selector(MacaList parts, long i, long n, const char* acc);
MacaList tw_queries(MacaList parts, long i, long n, MacaList acc);
const char* tw_escape(const char* c);
const char* tw_escaped(MacaList cs, long i, const char* acc);
const char* tw_rule(const char* c);
const char* tw_wrapped(MacaList media, const char* rule);
long tw_order(const char* c);
long tw_rank(MacaList parts, long i, long n, long r);
long tw_layer(const char* v, long r);
MacaList tw_words(const char* s);
MacaList tw_in_exprs(MacaList xs, long i, MacaList acc);
MacaList tw_in_expr(Expr e, MacaList acc);
long tw_tagged(Expr e);
MacaList tw_in_stmts(MacaList items, long i, MacaList acc);
MacaList tw_in_stmt(Stmt s, MacaList acc);
MacaList tw_unique(MacaList cs, long i, MacaList acc);
const char* tw_quoted(const char* s);
const char* tw_sheet(MacaList cs, long rank, const char* acc);
const char* tw_rules(MacaList cs, long i, long rank, const char* acc);
const char* style_sheet(MacaList items);
const char* quoted(const char* s);
const char* c_id(const char* name);
const char* emit_expr(Expr e);
const char* emit_while(Expr e);
const char* emit_for(Expr e);
long is_loop(Expr e);
const char* emit_loop(Expr e);
const char* emit_unary(Expr e);
const char* emit_spawn(Expr call);
const char* emit_jump(Expr e, const char* ret);
const char* emit_bool(Expr e);
const char* emit_ternary(Expr e);
const char* c_arm(Expr e, const char* other);
const char* emit_list(Expr e);
const char* c_cells(const char* el, MacaList cs, long i);
const char* c_cell(const char* el, Expr e);
const char* c_cell_of(const char* el, const char* code);
const char* c_cell_at(const char* recv, const char* el, const char* ix);
long c_boxed(const char* el);
const char* c_elem_of(const char* ty);
long map_method(const char* name);
const char* emit_map_method(Expr e, const char* recv);
const char* c_map_read(const char* val, const char* call);
long c_own_type(const char* ty);
long c_sized_number(const char* ty);
long c_sized_float(const char* ty);
long is_vector_type(const char* ty);
long c_vec_lanes(const char* ty);
const char* c_vec_scalar(const char* ty);
const char* c_vec_splat(const char* ty, const char* arg);
const char* c_vec_sum(const char* ty, const char* recv);
const char* emit_method(Expr e);
long c_list_method(const char* name);
const char* emit_list_method(Expr e, const char* recv, const char* el);
const char* c_over(const char* recv, const char* body);
const char* c_fold_pick(const char* held, const char* over, const char* el, const char* op);
const char* c_cell_kind(const char* el);
long c_padding(const char* name);
long c_str_method(const char* name);
const char* c_ufcs_args(const char* recv, MacaList cs);
const char* c_apply(Expr f, const char* el, const char* arg, const char* ret);
const char* emit_map(Expr e);
const char* emit_filter(Expr e);
const char* emit_reduce(Expr e);
const char* c_apply2(Expr f, const char* held, const char* el, const char* cur);
const char* c_list_find(const char* recv, const char* el, Expr want);
const char* c_at(const char* recv, long on_str, const char* ix);
const char* c_char(const char* recv, long on_str);
long is_list_type(const char* ty);
const char* emit_call(Expr e);
const char* emit_prelude(Expr e, const char* args);
long c_math_call(const char* name);
long c_picking(const char* name);
const char* c_pick2(Expr e);
const char* c_clamp(Expr e);
const char* c_gcd(Expr e);
const char* c_len(MacaList cs, const char* args);
long c_runtime_call(const char* name);
long c_char_call(const char* name);
long c_file_call(const char* name);
long c_host_call(const char* name);
const char* c_assert_eq(MacaList cs);
const char* c_shown(MacaList cs, long i);
const char* c_arg_ty(MacaList cs);
const char* c_str_of(const char* ty, const char* args);
const char* c_list_str(const char* el, const char* code);
const char* c_int_of(const char* ty, const char* args);
long c_compare(const char* op);
long c_is_str(Expr e);
long c_joins(const char* op, Expr l, Expr r);
const char* emit_binary(Expr e);
const char* c_joined(Expr e);
long c_fresh(const char* code);
const char* c_join(const char* l, const char* r);
const char* c_say(const char* f, const char* args, const char* end);
const char* c_overload(const char* op);
const char* c_store(Expr l, Expr r);
const char* emit_match(Expr e);
const char* emit_match_stmt(Expr e);
const char* stmt_arms(const char* scrut, MacaList cs, long i, long tagged, const char* el, const char* sum);
const char* emit_arms(const char* scrut, MacaList cs, long i, const char* el, const char* sum);
const char* emit_arms_at(const char* scrut, MacaList cs, long i, long tagged, const char* el, const char* sum);
long bound_pat(Expr p);
long any_bound(MacaList cs, long i);
long is_alt_pat(Expr p);
long is_cells_pat(Expr p);
const char* cells_test(const char* scrut, Expr p);
const char* literal_cells(const char* scrut, Expr p, long i);
const char* bind_cells(const char* scrut, Expr p, const char* el, long i);
const char* test(const char* scrut, Expr pat, long tagged, const char* el);
const char* one_cell_test(const char* scrut, Expr pat, const char* el);
const char* guard_test(const char* scrut, Expr pat, long tagged, const char* el);
long is_fields_pat(Expr p);
long is_bind_pat(Expr p);
long c_lower_name(const char* name);
const char* bind_one(const char* scrut, Expr p);
const char* arm_body(const char* scrut, Expr pat, Expr body, const char* arm_elem, const char* sum);
const char* bind_fields(const char* scrut, MacaList fs, long i);
const char* bind_slots(const char* scrut, MacaList bs, const char* sum, long i);
const char* emit_block(Expr e);
const char* block_stmts(MacaList body, long i);
const char* emit_with(Expr e);
const char* emit_updates(MacaList fs, long i);
const char* emit_lit_fields(MacaList fs, long i);
const char* emit_lit_field(Expr f);
const char* emit_args(MacaList xs, long i);
const char* emit_unit(Expr e);
long is_fn_type(const char* ty);
const char* c_decl(const char* ty, const char* name);
const char* c_ident_value(Expr e);
const char* c_arg_type(Expr a);
MacaList c_arg_names(long n, long i, MacaList acc);
const char* c_arg_binds(MacaList tys, MacaList vals, long i);
const char* c_call_value(const char* fnv, const char* rt, MacaList tys, MacaList vals);
const char* c_indirect(const char* fnv, const char* rt, MacaList args);
const char* c_closure(Expr e);
const char* c_cap_cells(MacaList caps, long i);
long is_closure_fn(const char* name);
const char* c_sig_params(Stmt s);
long is_map_type(const char* ty);
const char* type_c(const char* ty);
const char* emit_fn(Stmt s);
long c_shimmed(Stmt s);
const char* c_name(Stmt s);
const char* c_ret(Stmt s);
const char* c_main_shim(Stmt s);
const char* c_params(MacaList ps, long i);
const char* emit_param(Expr p);
const char* emit_stmts(MacaList body, long i, const char* ret);
const char* bind_c_decl(Stmt s);
const char* bind_c_type(Stmt s);
const char* local_c_type(Expr e);
const char* guessed_c_type(Expr e);
const char* call_c_type(Expr e);
long c_str_call(const char* name);
const char* method_c_type(Expr e);
long c_listy_method(const char* name);
long c_texty_method(const char* name);
const char* concat_c_type(Expr e);
const char* emit_stmt(Stmt s, long is_last, const char* ret);
const char* emit_if_stmt(Expr e);
long is_missing_else(Expr e);
const char* c_branch(Expr e);
long is_raise(Expr e);
const char* c_zero(const char* ty);
const char* c_preamble();
const char* c_errors(const char* code);
long c_uses(const char* code, const char* name);
const char* c_bare(const char* code);
MacaList c_unquoted(MacaList parts, long i, long live, MacaList acc);
long c_escaped(const char* part);
long c_slashes(const char* s, long i, long n);
const char* c_net_headers();
const char* c_process(const char* code);
const char* c_threads(const char* code);
const char* c_sockets(const char* code);
const char* c_http();
const char* c_mqtt(const char* protos);
const char* emit_module(Module m);
const char* c_styles(Module m, const char* code);
const char* c_ffi(const char* protos);
const char* c_sqlite(const char* protos);
const char* c_python(const char* protos);
const char* emit_consts(MacaList items, long i);
const char* emit_const(Stmt s);
const char* c_held(Stmt s);
long c_constant(Expr e);
const char* emit_starts(MacaList items);
const char* emit_start_sets(MacaList items, long i);
long is_type_item(Stmt s);
const char* emit_types(MacaList items, long i);
const char* emit_ranked(MacaList items, long i, long r);
const char* emit_rank(MacaList items, long i, long r);
long type_rank(MacaList items, Stmt s, long fuel, long i);
long node_rank(MacaList items, Expr p, long fuel);
long payload_rank(MacaList items, MacaList cs, long fuel, long i);
long named_rank(MacaList items, const char* ty, long fuel, long i);
long is_declared_only(Stmt s);
const char* emit_protos(MacaList items, long i);
const char* emit_bodies(MacaList items, long i);
const char* emit_proto(Stmt s);
const char* emit_item(Stmt s);
const char* emit_struct(Stmt s);
const char* emit_struct_field(Expr f);
const char* emit_sum(Stmt s);
const char* variant_name(Expr v);
const char* emit_tagged_sum(Stmt s);
const char* tag_name(Expr v);
long payload_arity(MacaList vs, long i);
long wider(long a, long b);
const char* emit_slots(Stmt s, long n, long i);
const char* slot_type(Stmt s, long at, long i);
const char* agreed(const char* a, const char* b);
const char* emit_ctors(Stmt s, long i);
const char* emit_ctor(Stmt s, Expr v);
const char* ctor_params(Stmt s, MacaList ps, long i);
const char* ctor_slot(Stmt s, MacaList ps, long i);
const char* ctor_assigns(Stmt s, MacaList ps, long i);
const char* rust_int(long n);
const char* rust_str(const char* s);
const char* rid(const char* name);
long rcopy(const char* ty);
const char* rowned(const char* ty);
const char* remit_expr(Expr e);
const char* rfield(Expr e);
const char* rplace(Expr e);
const char* rjump(Expr e);
const char* remit_lambda(Expr e);
const char* remit_while(Expr e);
const char* remit_for(Expr e);
const char* remit_unary(Expr e);
const char* remit_ternary(Expr e);
long rmissing_else(Expr e);
const char* rif_stmt(Expr e);
const char* rbranch(Expr e);
const char* remit_match(Expr e);
const char* remit_match_on(Expr e);
long has_str_arm(MacaList cs, long i);
const char* rchar_pred(const char* name);
long rstr_helper(const char* name);
const char* remit_str_method(Expr e, const char* recv);
const char* rmap_method_of(Expr e, const char* recv);
long rlist_method(const char* name);
const char* rlist_method_of(Expr e, const char* recv);
const char* rarg(Expr e, long i);
long rstr_index(Expr e);
const char* remit_method(Expr e);
const char* remit_call(Expr e);
const char* rlen_of(const char* ty, const char* args);
const char* rstr_of(const char* ty, const char* args);
long rmath_call(const char* name);
long rpicking(const char* name);
const char* rpick2(Expr e);
const char* rshown(MacaList cs, long i);
long rruntime_call(const char* name);
const char* rarg_ty(MacaList cs);
const char* rint_of(const char* ty, const char* args);
const char* remit_binary(Expr e);
const char* rstore(Expr l, const char* value);
long rjoins(Expr e);
long rlist(const char* ty);
const char* remit_arms(MacaList cs, long i, const char* scrut);
const char* remit_arm_pat(Expr p, Expr body);
long rfields_pat(Expr p);
const char* rbind_fields(const char* scrut, MacaList fs, long i);
const char* remit_pat(Expr p);
const char* binder_name(Expr b);
const char* remit_block(Expr e);
const char* rblock_stmts(MacaList body, long i);
long rmut(MacaList body, long i, Stmt s);
long rassigned_in(MacaList body, long i, const char* name);
long rassigned_deep(Expr e, const char* name);
long rstored_into(Expr e, const char* name);
long rassigned_any(MacaList cs, long i, const char* name);
const char* remit_with(Expr e);
const char* remit_updates(MacaList fs, long i);
const char* remit_lit_fields(MacaList fs);
const char* remit_lit_field(Expr f);
const char* remit_args(MacaList xs, long i);
const char* relem(const char* ty);
const char* rclone(const char* ty);
const char* rmap_key(const char* ty);
const char* rmap_val(const char* ty);
long r_is_fn(const char* ty);
const char* r_fn_ret(const char* ty);
const char* r_fn_params(const char* ty);
const char* r_param_types(const char* list, long at, const char* acc);
const char* rtype(const char* ty);
const char* remit_fn(Stmt s);
const char* rforeign(Stmt s);
const char* rret(Stmt s);
const char* rtail_ty(MacaList body);
const char* rust_argv(MacaList params);
const char* remit_param(Expr p);
const char* remit_stmts(MacaList body, long i);
const char* remit_stmt(Stmt s, long is_last, long movable);
const char* rmut_word(long movable);
const char* remit_module(Module m);
const char* remit_items(MacaList items, MacaList lz, MacaList own, long i);
MacaList rlazy_names(MacaList items, long i, MacaList acc);
long rscalar(Expr e);
Stmt rlazy_stmt(Stmt s, MacaList lz);
MacaList rlazy_body(MacaList body, MacaList lz, long i, MacaList acc);
Expr rlazy_expr(Expr e, MacaList lz);
MacaList rlazy_children(MacaList cs, MacaList lz, long i, MacaList acc);
const char* rust_preamble();
const char* remit_item(Stmt s, MacaList own);
const char* remit_impl(Stmt s, MacaList own);
const char* remit_methods(MacaList fs, MacaList own, long i, const char* acc);
const char* rimpl_method(Expr f, MacaList own);
MacaList rmethod_params(MacaList ps, MacaList own, long i, MacaList acc);
const char* rmethod_param(Expr p, MacaList own);
const char* rmethod_ret(Expr lam);
const char* rmethod_answer(Expr lam);
const char* remit_const(Stmt s);
const char* rconst_type(Expr e);
const char* rcell_type(MacaList cs);
const char* remit_struct(Stmt s);
const char* remit_struct_field(Expr f);
const char* remit_sum(Stmt s);
const char* remit_variant(Expr v);
const char* payload_type(Expr p);
const char* jid(const char* name);
const char* js_str(const char* s);
long js_list(const char* ty);
long js_scalar_ty(const char* ty);
long js_scalar(Expr e);
long js_own_type(const char* ty);
long js_upper(const char* name);
long js_binder(const char* name);
const char* jemit_expr(Expr e);
const char* jemit_jump(Expr e);
const char* jemit_unary(Expr e);
const char* jemit_while(Expr e);
const char* jemit_for(Expr e);
const char* jemit_match(Expr e);
const char* jemit_arms(MacaList cs, long i);
const char* jemit_arm(Expr p, Expr body);
const char* jpat_test(Expr p, const char* sv);
const char* jguard_test(Expr p, const char* sv);
const char* jcells_test(Expr p, const char* sv);
long js_cell_count(Expr p);
const char* jcell_tests(Expr p, const char* sv, long i, long n);
const char* jpat_binds(Expr p, const char* sv);
const char* jfield_binds(MacaList fs, const char* sv, long i);
const char* jpayload_binds(MacaList cs, const char* sv, long i);
const char* jcell_binds(Expr p, const char* sv, long i, long n);
const char* jemit_ternary(Expr e);
const char* jemit_else(Expr e);
const char* jemit_binary(Expr e);
const char* js_not(const char* op);
long js_joins(Expr e);
long js_deep(Expr e);
const char* jstore(Expr l, const char* value);
const char* jarg(Expr e, long i);
const char* js_math(const char* name);
const char* jemit_call(Expr e);
long js_map_method(const char* name);
const char* jemit_map_method(Expr e, const char* recv);
const char* js_zero(const char* ty);
const char* jemit_method(Expr e);
const char* jtext_method(Expr e, const char* recv);
const char* jlist_method(Expr e, const char* recv);
const char* jemit_lambda(Expr e);
const char* jemit_with(Expr e);
const char* jemit_fields(MacaList fs, long i);
const char* jemit_field(Expr f);
const char* jemit_args(MacaList xs, long i);
const char* jemit_params(MacaList ps, long i);
const char* js_binder_name(Expr p);
const char* jemit_block(Expr e);
const char* jemit_stmts(MacaList body, long i);
const char* jemit_stmt(Stmt s);
const char* jstmt_of(Expr e);
const char* jmatch_stmt(Expr e);
const char* jarms_stmt(MacaList cs, long i);
const char* jif_stmt(Expr e);
const char* jbranch(Expr e);
long js_loop(Expr e);
const char* jemit_fn(Stmt s);
const char* js_waiting(MacaList body);
long js_waits_in(MacaList body, long i);
long js_waits(Expr e);
long js_waits_any(MacaList cs, long i);
const char* jemit_body(MacaList body, long i);
const char* jemit_module(Module m);
const char* jemit_variants(MacaList items, long i);
const char* jemit_variant(Expr v);
const char* js_payload_names(MacaList cs, long i);
const char* jemit_consts(MacaList items, long i);
const char* jemit_exports(MacaList items);
const char* js_exported(MacaList items, long i);
const char* js_export_of(Stmt s);
const char* js_variant_ref(Expr v);
const char* jemit_entry(MacaList items, long i);
const char* jemit_items(MacaList items, long i);
const char* nemit_module(Module m);
const char* nix_home(MacaList ls, const char* user);
MacaList nix_binds(MacaList items, long i, long home, MacaList acc);
const char* nix_block(MacaList ls, long n, long i, const char* acc);
const char* nix_indent(const char* s, long n);
const char* nix_pad(long n, const char* acc);
const char* nix_padded(MacaList ls, const char* pad, long i, const char* acc);
long nix_at_home(Stmt s);
long nix_program(Stmt s);
const char* nemit_bind(Stmt s);
long nix_path2(MacaList p, const char* a, const char* b);
long nix_path3(MacaList p, const char* a, const char* b, const char* c);
const char* nix_enabled(Expr v);
const char* nix_body(MacaList ls, long i, const char* acc);
const char* nix_pkg_list(Expr v);
MacaList nix_elems(Expr v);
const char* nix_pkg_ref(Expr e);
const char* nvalue(Expr e);
const char* nix_attrs(MacaList fs);
MacaList nix_fields(MacaList fs, long i, MacaList acc);
const char* nix_field(Expr f);
const char* nix_unary(Expr e);
const char* nix_cond(Expr e);
const char* nix_binary(Expr e);
long nix_infix(const char* op);
const char* nix_xdg_dirs(Expr v);
const char* nix_dir_lines(MacaList xs, long i, const char* acc);
const char* nix_dir_line(const char* name);
const char* nix_xdg_key(const char* name);
const char* nix_string(const char* s);
const char* nix_escaped(MacaList cs, long i, const char* acc);
const char* nix_escape(const char* c);
Mcu emb_mcu(const char* name);
const char* emb_linker_script(Mcu m);
const char* eemit_module(Module m);
MacaList eemit_errors(Module m);
const char* emb_consts(MacaList items, long i, const char* acc);
const char* emb_const(Stmt s);
const char* emb_fns(MacaList items, long i, const char* acc);
const char* emb_fn(Stmt s);
const char* emb_params(MacaList ps, long i, const char* acc);
const char* emb_block(MacaList body, long wants_value, long ind);
const char* emb_stmts(MacaList body, long i, long wants_value, long ind, const char* acc);
const char* emb_stmt(Stmt s, long last, long wants_value, long ind);
const char* emb_while(Expr e, long ind);
const char* emb_for(Expr e, long ind);
const char* emb_if(Expr e, long ind);
long emb_no_else(Expr e);
const char* emb_branch(Expr e, long ind);
const char* emb_jump(Expr e, const char* pad);
const char* emb_pad(long n, const char* acc);
long emb_pure(Expr e);
const char* emb_expr(Expr e);
const char* emb_int(long n);
const char* emb_bool(Expr e);
const char* emb_ternary(Expr e);
const char* emb_unary(Expr e);
const char* emb_binary(Expr e);
long emb_prefix(const char* op);
long emb_infix(const char* op);
const char* emb_call(Expr e);
const char* emb_reg(const char* addr);
const char* emb_arg(Expr e, long i);
const char* emb_args(MacaList xs, long i, const char* acc);
MacaList emb_item_errors(MacaList items, long i, MacaList acc);
MacaList emb_item_error(Stmt s, MacaList acc);
const char* emb_sum_refusal(const char* name);
const char* emb_record_refusal(const char* name);
MacaList emb_stmt_errors(MacaList body, long i, MacaList acc);
MacaList emb_one_errors(Stmt s, MacaList acc);
MacaList emb_if_errors(Expr e, MacaList acc);
MacaList emb_branch_errors(Expr e, MacaList acc);
MacaList emb_jump_errors(Expr e, MacaList acc);
MacaList emb_value_errors(Expr e, MacaList acc);
MacaList emb_child_errors(MacaList xs, long i, MacaList acc);
const char* emb_refusal(Expr e);
const char* emb_named(Expr e);
const char* emb_effect_named(const char* op);
long emb_lowered(Expr e);
const char* jv_id(const char* name);
const char* jvmemit_module(Module m, const char* name);
const char* jvm_helpers();
const char* jv_types(MacaList items, long i, const char* acc);
const char* jv_enum(Stmt s);
const char* jv_variant_name(Expr v);
const char* jv_class(Stmt s);
const char* jv_fields(MacaList fs, long i, const char* acc);
const char* jv_setters(const char* owner, MacaList fs, long i, const char* acc);
const char* jv_members(MacaList items, MacaList fns, long i, const char* acc);
const char* jv_bind(Stmt s, MacaList items, MacaList fns);
long jv_is_impl(Stmt s, MacaList fns);
long jv_all_fns(MacaList fs, MacaList fns, long i);
const char* jv_impl(Stmt s, MacaList items);
const char* jv_impl_methods(MacaList fs, MacaList items, long i, const char* acc);
Stmt jv_named_fn(MacaList items, long i, const char* name);
const char* jv_names(MacaList ps, long i, const char* acc);
const char* jv_const(Stmt s);
const char* jv_const_type(Stmt s);
const char* jv_method(Stmt s);
const char* jv_ret(Stmt s);
const char* jv_params(MacaList ps, long i, const char* acc);
const char* jv_main(Stmt s);
const char* jv_argv(MacaList ps);
const char* jv_body(MacaList body, const char* ret, long ind);
const char* jv_stmts(MacaList body, long i, const char* ret, long ind, const char* acc);
const char* jv_stmt(Stmt s, long last, const char* ret, long ind);
const char* jv_fallback(long wants, const char* ret, const char* pad);
const char* jv_local(Stmt s);
const char* jv_while(Expr e, long ind);
const char* jv_for(Expr e, long ind);
const char* jv_jump(Expr e, const char* pad);
const char* jv_if_stmt(Expr e, long ind);
long jv_no_else(Expr e);
const char* jv_branch(Expr e, long ind);
const char* jv_pad(long n, const char* acc);
long jv_pure(Expr e);
const char* jv_zero(const char* ret);
const char* jv_expr(Expr e);
const char* jv_string(const char* s);
const char* jv_name(Expr e);
long jv_upper(const char* w);
long jv_user_type(const char* ty);
const char* jv_jump_value(Expr e);
const char* jv_ternary(Expr e);
const char* jv_else(Expr e);
const char* jv_unary(Expr e);
const char* jv_binary(Expr e);
long jv_by_value(Expr l, Expr r);
long jv_listy(const char* ty);
const char* jv_new(Expr e);
const char* jv_writes(MacaList fs, long i, const char* acc);
const char* jv_args(MacaList xs, long i, const char* acc);
const char* jv_arg(Expr e, long i);
const char* jv_arg_ty(Expr e);
const char* jv_call(Expr e);
const char* jv_stream(const char* name);
long jv_math_call(const char* name);
const char* jv_to_int(Expr e, const char* a);
const char* jv_size(const char* recv, const char* ty);
const char* jv_method_call(Expr e);
const char* jv_char_test(const char* name);
const char* jv_scrutinee(Expr e);
const char* jv_match(Expr e);
const char* jv_arms(MacaList cs, long i);
const char* jv_match_stmt(Expr e, long ind);
const char* jv_stmt_arms(MacaList cs, long i);
const char* jv_action(Expr e);
const char* jv_label(Expr p);
const char* jv_type(const char* ty);
const char* jv_boxed(const char* ty);
MacaList jvmemit_errors(Module m);
MacaList jv_fn_names(MacaList items, long i, MacaList acc);
MacaList jv_item_errors(MacaList items, MacaList fns, long i, MacaList acc);
MacaList jv_one_item(Stmt s, MacaList fns, MacaList acc);
long jv_carries(MacaList vs, long i);
const char* jv_payload_refusal(const char* name);
MacaList jv_body_errors(MacaList body, MacaList fns, long i, MacaList acc);
MacaList jv_value_errors(Expr e, MacaList fns, MacaList acc);
MacaList jv_if_errors(Expr e, MacaList fns, MacaList acc);
MacaList jv_branch_errors(Expr e, MacaList fns, MacaList acc);
MacaList jv_kid_errors(MacaList xs, MacaList fns, long i, MacaList acc);
const char* jv_refusal(Expr e, MacaList fns);
const char* jv_effect_refusal(const char* op);
const char* jv_store_refusal(Expr target);
const char* jv_method_refusal(Expr e);
const char* jv_match_refusal(Expr e);
const char* jv_arm_refusal(MacaList cs, long i);
const char* jv_shape_pat(Expr p);
long jv_known_call(const char* name, MacaList fns);
long jv_builtin(const char* name);
const char* print_module(Module m);
const char* print_marked(Module m, MacaList marks);
const char* print_source(const char* src);
MacaList imports_of(const char* src, MacaList ts, MacaList marks, long i, MacaList acc);
Token one_import(const char* src, MacaList ts, MacaList marks, long i);
long import_text_end(const char* src, MacaList ts, MacaList marks, long shut, long from);
MacaList in_order(MacaList a, MacaList b, long i, long j, MacaList acc);
const char* print_items(MacaList items, MacaList marks, long i, long seen, const char* acc);
const char* marks_before(MacaList marks, long from, long upto, const char* acc);
const char* trailing_marks(MacaList marks, long from, const char* acc);
const char* written(MacaList marks, long from, long more);
long past(MacaList marks, long from, long upto);
const char* print_item(Stmt s);
const char* print_fields(MacaList fs, long i, const char* acc);
const char* print_variants(MacaList vs, long i, const char* acc);
const char* print_variant(Expr v);
const char* print_payloads(MacaList ps, long i, const char* acc);
const char* print_fn(Stmt s);
const char* print_ret(const char* ret);
const char* print_params(MacaList ps, long i, const char* acc);
const char* print_param(Expr p);
const char* indent_of(long d);
const char* print_body(MacaList body, long i, const char* acc, long d);
const char* print_stmt(Stmt s, long d);
const char* print_expr(Expr e, long d);
const char* print_str(const char* s);
const char* doubled_braces(MacaList cs, long i, const char* acc);
const char* print_jump(Expr e, long d);
const char* print_method(Expr e, long d);
long loose(Expr e);
const char* held(Expr e, long d);
const char* print_unary(Expr e, long d);
const char* print_with(Expr e, long d);
const char* print_lambda(Expr e, long d);
const char* print_inner(Expr e, long d);
const char* print_wrapped(Expr e, long d);
long needs_block(Expr e);
const char* print_if(Expr e, long d);
const char* print_guards(Expr e, long d);
const char* print_otherwise(Expr e, long d);
const char* print_while(Expr e, long d);
const char* print_for(Expr e, long d);
const char* print_match(Expr e, long d);
const char* print_arms(MacaList xs, long i, const char* acc, long d);
const char* print_arm(Expr e, long d);
const char* print_pattern(Expr p, long d);
const char* print_rest(MacaList xs, long d);
const char* print_pieces(MacaList xs, long i, const char* acc, long d);
long op_power(const char* op);
const char* print_binary(Expr e, long d);
const char* print_operand(Expr c, long mine, long side, long d);
const char* print_ternary(Expr e, long d);
const char* print_args(MacaList xs, long i, const char* acc, long d);
MacaList lines(const char* s);
MacaList words(const char* s);
MacaList keep_nonempty(MacaList xs, long i, MacaList acc);
MacaList split_once(const char* s, const char* sep);
const char* strip_prefix(const char* s, const char* p);
const char* strip_suffix(const char* s, const char* p);
long index_of_from(const char* s, const char* pat, long start);
long last_index_of(const char* s, const char* pat);
long scan_last(const char* s, const char* pat, long at, long best);
const char* between(const char* s, const char* open_mc, const char* close_mc);
const char* escape_html(const char* s);
long count(const char* s, const char* pat);
long count_from(const char* s, const char* pat, long at, long n);
const char* title_case(const char* s);
const char* capitalize(const char* w);
const char* join_words(MacaList ws, long i, const char* acc);
const char* indent(const char* s, const char* pad);
const char* pad_unless_empty(const char* line, const char* pad);
const char* dedent(const char* s);
const char* drop_indent(const char* line, long cut);
long common_indent(MacaList ls, long i, long best);
long narrower(long best, const char* line);
long leading_spaces(MacaList cs, long i);
const char* wrap(const char* s, long width);
const char* fill(MacaList ws, long i, long width, const char* cur, const char* out);
const char* next_word(MacaList ws, long i, long width, const char* cur, const char* out);
const char* flush(const char* out, const char* cur);
const char* encode(long value);
long decode(const char* text);
const char* quote(const char* s);
const char* array_of_str(MacaList xs);
const char* array_of_int(MacaList xs);
const char* object_of(MacaList keys, MacaList values);
MacaList pairs(MacaList ks, MacaList vs, long i, MacaList acc);
const char* get(const char* src, const char* key);
long get_int(const char* src, const char* key, long dflt);
long get_bool(const char* src, const char* key);
MacaList items(const char* src);
MacaList split_items(MacaList cs, long i, long depth, const char* cur, MacaList acc);
MacaList next_item(MacaList cs, long i, long depth, const char* cur, MacaList acc);
long nesting(const char* c);
MacaList copy_string(MacaList cs, long i, long depth, const char* cur, MacaList acc);
MacaList split_items_after_escape(MacaList cs, long i, long depth, const char* cur, MacaList acc);
const char* value_at(const char* src, long at);
long balanced_end(MacaList cs, long i, long depth);
long balanced_step(MacaList cs, long i, long depth);
long skip_string(MacaList cs, long i);
long quote_end(MacaList cs, long i);
long bare_end(MacaList cs, long i);
const char* unwrap(const char* s);
const char* unescape(const char* s);
Trace trace(const char* label, MacaList spans);
Span region(const char* name, long start, long end);
long duration(Span s);
long span_count(Trace t);
long wall(Trace t);
long origin(Trace t);
long first_start(MacaList xs, long i, long best);
long last_end(MacaList xs, long i, long best);
MacaList roots(Trace t);
MacaList children(Trace t, long i);
MacaList kids_of(MacaList xs, long parent, long i, MacaList acc);
long held_by(MacaList xs, long i);
long levels(Trace t);
long deepest(MacaList xs, long i, long best);
MacaList level(Trace t, long d);
MacaList at_depth(MacaList xs, long d, long i, MacaList acc);
long child_time(Trace t, long i);
long sum_kids(Trace t, MacaList ids, long i, long acc);
long self_time(Trace t, long i);
MacaList leaked(Trace t);
MacaList unclosed(MacaList xs, long i, MacaList acc);
long find_span(Trace t, const char* name);
long scan_name(MacaList xs, const char* name, long i);
const char* to_json(Trace t);
MacaList span_objects(MacaList xs, long i, MacaList acc);
const char* span_json(Span s);
Trace from_json(const char* src);
MacaList read_spans(MacaList objs, long i, MacaList acc);
Span read_span(const char* o);
long enabled();
const char* paint(const char* code, const char* s);
long width(const char* s);
long utf8_len(long b);
long codepoint(const char* s, long i, long len);
long lead_base(long len);
long tail(const char* s, long i);
long columns(long cp);
long skip_escape(const char* s, long from);
const char* pad(const char* s, long w);
const char* pad_left(const char* s, long w);
const char* plain(const char* s);
const char* bold(const char* s);
const char* dim(const char* s);
const char* italic(const char* s);
const char* underline(const char* s);
const char* red(const char* s);
const char* green(const char* s);
const char* yellow(const char* s);
const char* blue(const char* s);
const char* magenta(const char* s);
const char* cyan(const char* s);
const char* grey(const char* s);
const char* ok(const char* s);
const char* warn(const char* s);
const char* bad(const char* s);
const char* note__20(const char* s);
Scale scale_of(Trace t, long cols);
long column(Scale sc, long at);
const char* flame(Trace t, long cols);
const char* chart_head(Trace t);
const char* plural(long n, const char* noun);
const char* leak_note(Trace t);
MacaList chart_rows(Trace t, Scale sc, long d, MacaList acc);
const char* lay_out(Trace t, Scale sc, MacaList ids, long i, long cursor, const char* acc);
long bar_start(Scale sc, Span s, long cursor);
long bar_width(Scale sc, Span s, long from);
const char* bar_text(Span s, long w);
const char* inside(const char* name, long ms, long room);
const char* fit(const char* s, long n);
long widest_prefix(const char* s, long n, long i, long best);
long on_boundary(const char* s, long i);
const char* tint(long i, const char* s);
const char* sgr(long i);
const char* flame_svg(Trace t, long px);
const char* flame_svg_in(Trace t, long px, const char* unit);
long svg_head_h();
long svg_row();
const char* svg_open(long px, long high);
const char* svg_backdrop(Trace t, long px, long high, long inset, const char* unit);
const char* svg_frames(Trace t, Scale sc, long inset, long i, const char* acc, const char* unit);
const char* svg_frame(Trace t, Scale sc, long inset, long i, const char* unit);
long frame_width(Scale sc, Span s, long inset, long left);
const char* svg_rect(long x, long y, long w, long i);
const char* svg_label(Span s, long x, long y, long w);
const char* svg_tip(Trace t, long i, const char* unit);
double percent(long part, long whole);
const char* swatch(long i);
long maca_main(MacaList args);
long version_asked(const char* a);
long help_asked(const char* a);
long usage();
long unknown_cmd(const char* name);
long run_script(const char* cmd);
long check_only(const char* src, const char* target);
long init_project(const char* root);
long write_absent(const char* path, const char* text);
const char* leaf_of(const char* root);
const char* indent_unit(MacaList chain);
const char* space_run(long n);
const char* reindent(const char* text, const char* unit);
MacaList reindent_lines(MacaList lines, const char* unit, long i, MacaList acc);
const char* reindent_line(const char* line, const char* unit);
const char* unit_times(const char* unit, long n);
long indent_width(const char* line, long i);
long format_file(const char* src, long only_check);
long fmt_cmd(MacaList args);
long fmt_each(MacaList files, long i, long only_check, long acc);
long fix_cmd(MacaList args);
MacaList fix_files(MacaList args, long i, MacaList acc);
long fix_each(MacaList files, long i, long dry, long bad);
long fix_one(const char* path, long dry);
long fix_lexed(const char* path, const char* src, long dry);
const char* fix_said(long dry);
long phantom_at(MacaList ts, long i);
long phantom_word(Token t);
long phantom_spelling(const char* w);
long phantom_count(MacaList ts, long i, long n);
MacaList cut_all(MacaList cs, MacaList ts, long i);
MacaList cut_one(MacaList cs, Token t);
long space_end(MacaList cs, long i);
long watch_cmd(MacaList args);
long watched(const char* src, MacaList rest);
long newest_ms(MacaList files, long i, long best);
long module_asked(const char* a);
long module_cmd(MacaList args);
long module_spec(const char* spec, MacaList rest);
const char* spec_refusal(const char* spec);
long empty_segment(MacaList ps, long i);
Entry module_entry(const char* spec);
Entry whole_entry(const char* whole);
const char* module_path(const char* name);
long module_found(Entry at, MacaList rest);
long module_entered(Entry at, MacaList rest);
const char* entry_fn(MacaList items, const char* own);
long fn_at(MacaList items, const char* want, long i);
long module_calling(Entry at, Stmt def, const char* want, MacaList rest);
const char* call_shape(Stmt def);
const char* param_list(MacaList ps, long i, const char* acc);
long module_shimmed(Entry at, const char* call, const char* ret, MacaList rest);
const char* entry_source(const char* module, const char* call, const char* ret);
const char* entry_body(const char* call, const char* ret);
const char* flag_after(MacaList args, const char* name);
long build_classes(const char* src, const char* out, const char* cp);
const char* jvm_class(const char* src);
const char* java_imports(const char* src);
const char* java_named(MacaList files, long i, const char* acc);
const char* java_in(MacaList ts, long i, const char* acc);
MacaList javac_line(const char* java, const char* out, const char* cp);
long javac_here();
long on_path(const char* cmd);
long build_rust(const char* src, const char* out);
long cargo_built(const char* src, const char* rs, const char* out, MacaList deps);
long cargo_built_at(const char* proj, const char* out);
const char* cargo_toml(MacaList chain, MacaList deps, MacaList patch);
const char* cargo_patch(MacaList chain, MacaList patch);
const char* cargo_entries(MacaList chain, const char* table, MacaList ks, long i, const char* acc);
const char* cargo_value(const char* v);
MacaList manifest_keys(MacaList chain, const char* table, long i);
long tooled(const char* name);
const char* tool_path(const char* name);
long build_out(const char* src, const char* out, const char* target, const char* mcu, const char* cp);
const char* default_out(const char* src, const char* target);
MacaList page_keys();
const char* stray_page_key(MacaList chain, long i);
const char* page_setting(MacaList chain, const char* key, const char* dflt);
long build_page(const char* src, const char* dir);
long page_written(const char* src, const char* dir, MacaList chain);
MacaList page_assets(MacaList files, long i, MacaList acc);
MacaList assets_in(const char* src, MacaList ts, long i, MacaList acc);
MacaList asset_at(const char* src, MacaList ts, long i);
long tagged_block(const char* src, MacaList ts, long i);
const char* raw_block(const char* src, long at);
MacaList asked_names(MacaList ts, long i, MacaList acc);
const char* asset_kind(const char* spec);
MacaList resolved_assets(const char* src, MacaList xs, long i, MacaList acc);
MacaList asset_errors(MacaList xs, long i, MacaList acc);
Found resolved_asset(const char* src, PageAsset a);
Found package_found(const char* src, const char* spec, const char* kind);
Found package_entry(const char* dir, const char* name, const char* said);
MacaList entry_keys();
const char* first_entry(const char* text, MacaList keys, long i);
const char* pkg_entry(const char* text, const char* key);
long next_quote(MacaList cs, long i);
const char* quoted_upto(MacaList cs, long i, MacaList acc);
const char* installed_at(const char* dir, const char* name);
const char* package_name(const char* spec);
const char* package_sub(const char* spec);
const char* unscoped(const char* spec);
const char* page_html(const char* src, MacaList chain, MacaList assets, MacaList found, const char* js);
const char* inlined(MacaList assets, MacaList found, long i, MacaList acc, long head);
const char* one_inlined(PageAsset a, Found f);
const char* script_open(PageAsset a, Found f);
long es_module(const char* path);
long module_package(const char* dir);
const char* close_safe(const char* text);
const char* rejoined(MacaList parts, long i, MacaList acc);
const char* html_text(const char* s);
const char* named_bindings(MacaList names, long i, MacaList acc);
const char* first_there(MacaList ss, long i, MacaList acc);
MacaList spellings(const char* name);
const char* camel_of(MacaList cs, long i, MacaList acc, long up);
const char* base64_of(const char* file);
long build_wasm(const char* src, const char* out);
long build_firmware(const char* src, const char* dir, const char* mcu);
long cross_compile(Mcu m, const char* dir);
long build_tauri(const char* src, const char* dir);
long tauri_shell(const char* src, const char* dir);
long tauri_backend(const char* src, const char* st);
long tauri_scaffolded(const char* src, const char* dir, const char* st);
const char* tauri_bridge();
const char* tauri_cargo(const char* name);
const char* tauri_conf(const char* name, const char* title);
const char* tauri_main_rs();
const char* crate_ident(const char* stem);
const char* ident_chars(MacaList cs, long i, MacaList acc);
long build_binary(const char* src, const char* out);
const char* cache_dir();
const char* cache_key(const char* src);
const char* cache_probe();
long cache_take(const char* from, const char* out);
long cache_store(const char* key, const char* out);
MacaList link_flags(const char* code);
MacaList sqlite_flags();
const char* nix_path(const char* attr);
MacaList python_flags();
long build_fresh(const char* src, const char* out);
MacaList cc_flags();
long build_cmd(MacaList args);
const char* declared_cp(MacaList args, MacaList chain);
const char* build_setting(MacaList args, MacaList chain, const char* flag, const char* key);
const char* declared_out(MacaList chain, long i);
MacaList build_keys();
const char* stray_build_key(MacaList chain, long i);
const char* stray_of(MacaList keys, MacaList known, long i);
const char* build_src(MacaList args, long i, const char* held);
long takes_value(const char* a);
const char* sniffed(const char* src, const char* asked);
const char* detected_target(const char* src);
const char* why_target(const char* found);
long imports_nixpkgs(MacaList paths, long i);
long answers_element(MacaList ts, long i);
long run_cmd(MacaList args);
long test_cmd(MacaList args);
long test_package(MacaList chain);
const char* package_heading(MacaList chain);
const char* tests_dir(MacaList chain);
MacaList suite_files(const char* dir, MacaList names, long i, MacaList acc);
long no_suites(const char* dir);
long ran_suites(MacaList suites, long i, long failed);
long check_cmd(MacaList args);
long check_json(const char* path, const char* target);
MacaList unread(MacaList xs, long i, MacaList acc);
MacaList unparsed(MacaList xs, long i, MacaList acc);
MacaList diag_list(const char* path, const char* text, MacaList ds, long i, MacaList acc);
const char* diag_json(const char* path, const char* text, Diagnostic d);
const char* note_json(const char* note);
const char* fix_suggestions(const char* text, Diagnostic d, long lo, long hi);
const char* spot_json(const char* text, long lo, long hi);
const char* json_text(const char* s);
long line_at(const char* text, long at);
long col_at(const char* text, long at);
const char* quoted_name(const char* msg);
long anchor_at(const char* text, const char* name, long from);
long at_or_zero(const char* text, long at);
long word_from(const char* text, const char* name, long from);
long word_here(const char* text, const char* name, long at, long n);
long word_char(const char* c);
const char* entry_of(const char* cmd, MacaList args);
const char* first_named(MacaList args);
const char* stem_of(const char* src);
MacaList manifest_chain(const char* from);
const char* rooted(const char* from);
MacaList manifest_dirs(const char* dir, MacaList acc);
long workspace_at(MacaList dirs, long i);
long declares_workspace(const char* toml);
long heads(MacaList lines, long i, const char* want);
MacaList upto(MacaList dirs, long stop, long i, MacaList acc);
MacaList here_chain();
MacaList chain_of(const char* src);
const char* chain_value(MacaList chain, long i, const char* table, const char* key);
long workspace_ok(MacaList chain);
const char* workspace_problem(MacaList chain);
const char* nested_workspace(MacaList chain, const char* root, long i);
const char* members_problem(const char* root, MacaList ms);
const char* member_refusal(const char* root, MacaList ms, long i);
long named_package(const char* file);
MacaList parents_of(MacaList ms, long i, MacaList acc);
const char* stray_in(const char* root, MacaList ms, MacaList parents, long i);
const char* stray_at(const char* root, MacaList ms, const char* parent, MacaList names, long i);
MacaList members_of(const char* toml);
long key_line(MacaList lines, long i, const char* at, const char* table, const char* key);
const char* list_body(MacaList lines, long i, const char* acc);
const char* bracketed(const char* s);
MacaList cleaned(MacaList xs, long i, MacaList acc);
const char* declared_bin(const char* cmd, const char* want);
const char* chosen_bin(const char* cmd, MacaList chain, const char* want);
const char* bin_file(const char* cmd, const char* dir, Bin b);
long bin_pick(MacaList bins, const char* want);
long bin_at(MacaList bins, const char* want, long i);
const char* bin_names(MacaList bins, long i, const char* acc);
const char* package_of(MacaList chain);
MacaList bins_of(MacaList lines, long i, MacaList acc);
MacaList with_bin(MacaList acc, Bin b);
Bin one_bin(MacaList lines, long i);
const char* block_value(MacaList lines, long i, const char* key);
MacaList table_keys(const char* toml, const char* table);
MacaList keys_in(MacaList lines, long i, const char* at, const char* table, MacaList acc);
const char* toml_value(const char* toml, const char* table, const char* key);
const char* table_value(MacaList lines, long i, const char* at, const char* table, const char* key);
const char* toml_head(const char* line);
const char* toml_key(const char* line);
const char* toml_val(const char* line);
const char* unquoted(const char* v);
MacaList nonempty(MacaList xs, long i, MacaList acc);
const char* shell_pid();
const char* scratch_path(const char* kind);
long run_file(const char* src, MacaList rest);
long test_file(const char* src);
MacaList test_names(MacaList items, long i, MacaList acc);
long run_tests(const char* src, MacaList names);
const char* without_main(const char* src);
long main_item(MacaList items, long i);
long item_start(MacaList items, long i);
const char* cut_out(const char* src, long from, long upto);
const char* test_main(MacaList names, long i, const char* acc);
long compile_file(MacaList args);
long refused_here(const char* asked, Module m);
MacaList ported_errors(MacaList items, long i, const char* target, MacaList acc);
MacaList ported_error(Stmt s, const char* target);
MacaList fn_fields(const char* rec, MacaList fs, long i, const char* target, MacaList acc);
MacaList foreign_errors(const char* asked, const char* src, Unit u);
MacaList browser_errors(const char* asked, Unit u);
const char* browser_error(const char* asked, const char* name);
const char* target_named(const char* asked);
long js_import_in(MacaList ts, long i);
long foreign_at(MacaList ts, long i, const char* lang);
const char* browser_file(MacaList files, long i);
const char* module_named(const char* path);
long last_root(MacaList ps, long i, long acc);
MacaList rust_import_errors(MacaList ts, long i, MacaList deps, MacaList acc);
MacaList rust_import_error(const char* lang, const char* spec, MacaList deps);
long rust_builtin(const char* name);
const char* crate_of(const char* spec);
const char* unprefixed(const char* name);
long path_like(MacaList cs, long i);
long path_char(const char* c);
Unit unit_of(const char* entry);
MacaList asks_of(const char* entry);
Asked walk_asks(const char* path, Asked a);
Asked asks_in(const char* by, MacaList ts, long i, Asked a);
Asked asks_at(const char* by, MacaList ts, long i, Asked a);
MacaList selected(MacaList ts, long i, const char* at, MacaList acc);
const char* nix_valued(const char* path, const char* src);
const char* nix_bound(const char* dir, MacaList ts, long i, const char* acc);
const char* nix_binding(const char* dir, const char* spec);
const char* nix_name(const char* spec);
Unit load_unit(const char* path, Unit u);
Unit spliced(Unit deps, const char* path, MacaList whole, MacaList errs);
MacaList holding(Unit u, const char* path, MacaList clash, long i, MacaList acc);
long keeps(Unit u, const char* path, const char* name);
long wanted(MacaList asks, const char* path, const char* name);
const char* owner_of(Unit u, const char* name, long i, const char* held);
MacaList without_names(MacaList xs, MacaList drop, long i, MacaList acc);
MacaList owned_by(MacaList mine, const char* path, long i, MacaList acc);
MacaList fn_names(MacaList items, long i, MacaList acc);
MacaList clashing(MacaList mine, MacaList taken, long i, MacaList acc);
MacaList renamed(MacaList ts, MacaList taken, const char* tag, long i);
Token one_renamed(Token t, MacaList taken, const char* tag);
MacaList without_eof(MacaList ts);
long live_end(MacaList ts, long n);
Unit load_deps(const char* by, MacaList wants, long i, Unit u);
Unit one_dep(const char* by, const char* want, Unit u);
MacaList imports_in(MacaList ts, long i, MacaList acc);
MacaList import_names(MacaList ts, long i);
const char* import_path(MacaList ts, long i, const char* acc);
const char* resolved(const char* by, const char* want);
const char* search_up(const char* dir, const char* want);
const char* in_base(const char* dir, const char* want);
const char* found(const char* cand);
const char* dir_of(const char* path);
const char* parent_of(const char* dir);
long sep_after(MacaList cs, long i);
long report_all(const char* stage, MacaList msgs, long i);
long spec_cmd(MacaList args);
long spec_asked(const char* root, const char* pkg);
const char* spec_root();
const char* root_above(const char* dir);
long printed_spec(const char* text);
long package_index(const char* root, const char* name);
const char* llm_spec(const char* root);
const char* cheatsheet(const char* spec);
const char* examples(const char* root);
const char* example(const char* root, const char* what, const char* name);
const char* indexed(const char* dir, const char* pkg, long full);
const char* index_files(const char* dir, const char* pkg, MacaList names, long i, long full, const char* acc);
const char* index_file(const char* dir, const char* pkg, const char* name, long full);
const char* documented(MacaList lines, long i, const char* doc, long full, const char* acc);
const char* summary_of(const char* line);
const char* signature_of(const char* line);
const char* item_line(const char* sig, const char* doc, long full);
const char* other_packages(const char* root);
const char* package_lines(const char* root, MacaList names, long i, const char* acc);
const char* blurb_of(const char* root, const char* pkg);
const char* first_maca(MacaList names, long i);
const char* blurb(const char* src);
const char* mistakes();
const char* targets_table();
const char* builtin_methods();
long profile_cmd(MacaList args);
long profile_built(const char* src, const char* svg);
long profile_measured(const char* src, const char* bin, const char* svg);
long profile_reported(const char* src, Dump d, const char* svg);
Dump costs_in(const char* text);
Scan one_line(MacaList table, const char* line, Scan s);
Dump charged(Dump d, const char* key, long ir);
Dump owned(Dump d, const char* name, long ir);
long is_cost_line(const char* line);
long cost_in(const char* line);
MacaList name_table(MacaList lines);
MacaList with_spec(MacaList acc, const char* spec);
const char* name_spec(const char* line);
const char* fn_spec(const char* line);
const char* spec_name(MacaList table, const char* spec);
const char* table_name(MacaList table, const char* id, long i);
const char* cost_table(Dump d);
MacaList cost_rows(Dump d, MacaList order, long i, MacaList acc);
const char* cost_row(Dump d, long at);
MacaList own_order(Dump d, long i, MacaList acc);
MacaList ranked(Dump d, MacaList acc, long at);
long rank_in(Dump d, MacaList acc, long own, long i);
MacaList frames_of(Dump d);
MacaList frames_at(Dump d, Call f, Frame at, MacaList path, MacaList acc);
MacaList laid_out(Dump d, MacaList kids, long i, long stop, Frame at, MacaList path, MacaList acc);
const char* root_name(Dump d);
long heaviest(Dump d, long i, long best);
long inclusive_of(Dump d, const char* name);
long own_of(Dump d, const char* name);
long edge_sum(Dump d, const char* from, long i, long acc);
MacaList calls_of(Dump d, const char* from);
MacaList calls_in(Dump d, const char* from, long i, MacaList acc);
Call one_call(Dump d, const char* from, long i);
MacaList heavy_first(MacaList cs, long i, MacaList acc);
MacaList call_ranked(MacaList acc, Call c);
long call_rank(MacaList acc, long ir, long i);
long dev_cmd(MacaList args);
long dev_written(const char* src, const char* out);
long dev_flake_out(Module m, const char* out);
const char* dev_flake(Module m);
long dev_at(MacaList items, const char* name, long i);
const char* dev_name(MacaList items);
const char* dev_packages(MacaList items);
const char* dev_env(MacaList items);
const char* dev_lines(MacaList ls, long i, const char* acc);
const char* dev_hook(MacaList items);
const char* flake_text(const char* name, const char* shell);
long add_cmd(MacaList args);
long added(MacaList specs, long i, const char* registry, long bad);
long add_one(const char* spec, const char* registry);
long install_cmd();
long install_all(MacaList deps, long i, const char* registry, long bad);
long install_one(Dep d, const char* registry);
long installed(Dep d, Pin p);
long update_cmd();
long update_all(MacaList deps, long i, const char* registry, long bad);
long update_one(Dep d, const char* registry);
Pin brought(Spec s, const char* spec, const char* registry, Pin want);
Pin landed(Spec s, const char* spec, Pin p);
long upgrade_cmd();
const char* asset_url(const char* doc, const char* want);
const char* asset_pick(const char* rest, const char* want);
long replaced(const char* want, const char* url);
long swapped(const char* exe, const char* url, const char* want);
const char* host_triple();
const char* json_str(const char* src, const char* key);
const char* quoted_at(MacaList cs, long i);
const char* upto_quote(MacaList cs, long i, const char* acc);
MacaList nonflags(MacaList args, long i, MacaList acc);
Spec parse_spec(const char* spec);
Spec npm_spec(const char* rest);
Spec git_spec(const char* rest);
Spec reg_spec(const char* s);
long version_cut(const char* s);
long last_of(MacaList cs, const char* want, long i);
const char* seg_after(const char* s, const char* sep);
const char* git_name(const char* url);
long bad_name(const char* name);
Ver ver_of(const char* text);
Ver bad_ver();
long core_end(MacaList cs, long i);
Ver ver_parts(MacaList parts);
long digits(const char* s);
long all_digits(MacaList cs, long i);
long ver_cmp(Ver a, Ver b);
long satisfies(const char* version, const char* range);
long any_clause(Ver v, MacaList clauses, long i);
long clause_holds(Ver v, const char* clause);
long every_part(Ver v, MacaList parts, long i);
long comparator(Ver v, const char* part);
long caret(Ver v, Ver lo);
Ver caret_hi(Ver lo);
long tilde(Ver v, Ver lo);
long cmp_op(Ver v, const char* text, const char* op);
long exact_ver(Ver v, Ver w);
long wild(Ver v, MacaList parts);
long wild_part(const char* part, long have);
const char* registry_url();
Pin resolve_dep(Spec s, const char* registry);
Pin git_pin(Spec s);
const char* first_sha(const char* out);
Pin registry_pin(const char* base, const char* pkg, const char* req);
Pin pin_of(const char* doc);
const char* http_get(const char* url);
Pin failed(const char* why);
Pin unpinned();
const char* fetch(Spec s, Pin p);
const char* git_into(const char* url, const char* sha, const char* dir);
const char* tgz_into(const char* url, const char* integrity, const char* dir);
const char* unpacked(const char* tgz, const char* integrity, const char* dir);
const char* mismatch(const char* file, const char* want);
const char* digested(const char* file, const char* want);
const char* digest_of(const char* file);
const char* hex_of(const char* b64);
MacaList manifest_deps();
MacaList deps_in(MacaList lines, long i, long end, MacaList acc);
long manifest_put(const char* name, const char* spec);
MacaList put_line(MacaList lines, const char* name, const char* line);
MacaList started(MacaList lines);
MacaList replaced_line(MacaList lines, long from, const char* name, const char* line);
long table_at(MacaList lines, long i, const char* table);
long block_end(MacaList lines, long i);
long tight_end(MacaList lines, long from, long end);
long key_at(MacaList lines, long i, long end, const char* name);
MacaList lock_read();
MacaList lock_lines(MacaList lines, long i, const char* name, const char* block, MacaList acc);
MacaList flushed(MacaList acc, const char* name, const char* block);
long noise(const char* line);
const char* lock_name(const char* line, const char* held);
Pin pinned(MacaList entries, const char* name, const char* spec);
long pkg_at(MacaList entries, const char* name, long i);
long lock_put(Spec s, const char* spec, Pin p);
const char* lock_block(Spec s, const char* spec, Pin p);
const char* lock_source(Spec s);
const char* lock_line(const char* key, const char* value);
const char* lock_text(MacaList entries, long i, const char* acc);
MacaList inserted(MacaList xs, Pkg p, long i, MacaList acc);
long demo();
__attribute__((constructor)) static void maca_module_init(void) { NixosRoots = maca_cat_own(maca_cat_own(maca_cat(" networking system services users user environment programs", " fonts boot hardware security nix nixpkgs virtualisation systemd"), " i18n time sound xdg home imports console powerManagement", 1), " documentation location ", 1); StarterDev = maca_cat_own(maca_cat("    import nixpkgs\n\n    dev.name     = \"myapp\"\n", "    dev.packages = rustc, cargo\n"), "    dev.env      = { RUST_BACKTRACE = \"1\" }\n", 1); }
Kind keyword_kind(const char* w) { return ((strcmp(w, "let") == 0) ? KwLet : ((strcmp(w, "if") == 0) ? KwIf : ((strcmp(w, "else") == 0) ? KwElse : ((strcmp(w, "for") == 0) ? KwFor : ((strcmp(w, "in") == 0) ? KwIn : ((strcmp(w, "while") == 0) ? KwWhile : ((strcmp(w, "break") == 0) ? KwBreak : ((strcmp(w, "continue") == 0) ? KwContinue : ((strcmp(w, "return") == 0) ? KwReturn : ((strcmp(w, "match") == 0) ? KwMatch : ((strcmp(w, "import") == 0) ? KwImport : ((strcmp(w, "with") == 0) ? KwWith : ((strcmp(w, "fail") == 0) ? KwFail : ((strcmp(w, "try") == 0) ? KwTry : ((strcmp(w, "alias") == 0) ? KwAlias : ((strcmp(w, "true") == 0) ? KwTrue : ((strcmp(w, "false") == 0) ? KwFalse : TIdent)))))))))))))))));  }
Token mk_token(Kind kind, const char* text, long pos) { return (Token){ .kind = kind, .text = text, .pos = pos, .fresh = 0 };  }
long is_space(const char* c) { return (isspace((unsigned char)(c)[0]) != 0);  }
long is_digit(const char* c) { return (isdigit((unsigned char)(c)[0]) != 0);  }
long is_alpha(const char* c) { return ((isalpha((unsigned char)(c)[0]) != 0) || (strcmp(c, "_") == 0));  }
long is_alnum(const char* c) { return (is_alpha(c) || is_digit(c));  }
long run_end(MacaList cs, long i, MacaFn pred) { return (((i < (cs.len)) && ({ MacaFn _c = pred; const char* _a0 = ((const char*)cs.data[i]); _c.env.len ? ((long(*)(MacaList, const char*))_c.fn)(_c.env, _a0) : ((long(*)(const char*))_c.fn)(_a0); })) ? run_end(cs, (i + 1), pred) : i);  }
const char* span(MacaList cs, long i, long j) { return maca_list_join(maca_list_slice(cs, i, j), "");  }
Lexed lexed(MacaList tokens, MacaList errors) { return (Lexed){ .tokens = tokens, .marks = maca_listv(0), .errors = errors, .pos = 0, .broke = 0 };  }
Lexed keep(Lexed acc, Token t) { return ({ __typeof__(acc) _w = acc; _w.tokens = maca_list_pushed(acc.tokens, maca_box(sizeof(Token), (Token[]){ ({ __typeof__(t) _w = t; _w.fresh = acc.broke; _w; }) })); _w.broke = 0; _w; });  }
Lexed mark(Lexed acc, Token t) { return ({ __typeof__(acc) _w = acc; _w.marks = maca_list_pushed(acc.marks, maca_box(sizeof(Token), (Token[]){ t })); _w; });  }
Lexed crossed(Lexed acc, const char* c) { return ({ __typeof__(acc) _w = acc; _w.broke = (acc.broke || (strcmp(c, "\n") == 0)); _w; });  }
Lexed moved(Lexed acc, long i) { return ({ __typeof__(acc) _w = acc; _w.pos = i; _w; });  }
Lexed note_error(Lexed acc, const char* msg) { return ({ __typeof__(acc) _w = acc; _w.errors = maca_list_cat(acc.errors, maca_listv(1, (long)(msg))); _w; });  }
MacaList lex(const char* src) { return lex_all(src).tokens;  }
MacaList lex_marked(const char* src) { return lex_all(src).marks;  }
Lexed lex_all(const char* src) { MacaList cs = maca_chars(src); Lexed scanned = scan(cs, lexed(maca_listv(0), maca_listv(0)), (cs.len)); return end_run(scanned, (cs.len), 0);  }
Lexed scan(MacaList cs, Lexed out, long hi) { return ((out.pos >= hi) ? out : (((hi - out.pos) == 1) ? step(cs, out) : halves(cs, out, hi, (out.pos + ((hi - out.pos) / 2)))));  }
Lexed halves(MacaList cs, Lexed out, long hi, long mid) { Lexed left = scan(cs, emptied(out), mid); Lexed right = scan(cs, emptied(left), hi); return merged_runs(out, left, right);  }
Lexed emptied(Lexed acc) { return (Lexed){ .tokens = maca_listv(0), .marks = maca_listv(0), .errors = maca_listv(0), .pos = acc.pos, .broke = acc.broke };  }
Lexed merged_runs(Lexed out, Lexed left, Lexed right) { return (Lexed){ .tokens = maca_list_cat(maca_list_cat(out.tokens, left.tokens), right.tokens), .marks = maca_list_cat(maca_list_cat(out.marks, left.marks), right.marks), .errors = maca_list_cat(maca_list_cat(out.errors, left.errors), right.errors), .pos = right.pos, .broke = right.broke };  }
Lexed end_run(Lexed out, long i, long n) { return ((n >= 4) ? out : end_run(keep(out, mk_token(Eof, "", i)), i, (n + 1)));  }
Lexed step(MacaList cs, Lexed out) { long i = out.pos; const char* c = ((const char*)cs.data[i]); return (is_space(c) ? moved(crossed(out, c), (i + 1)) : (comments(cs, i) ? lex_comment(cs, i, out) : (triple(cs, i) ? lex_raw(cs, i, out) : ((strcmp(c, "\"") == 0) ? lex_string(cs, i, out) : (is_digit(c) ? lex_number(cs, i, out) : (is_alpha(c) ? lex_word(cs, i, out) : lex_punct(cs, i, out, c)))))));  }
Lexed lex_comment(MacaList cs, long i, Lexed out) { long j = line_end(cs, i); return moved(mark(out, mk_token(Comment, span(cs, i, j), i)), j);  }
long comments(MacaList cs, long i) { return (((strcmp(((const char*)cs.data[i]), "/") == 0) && ((i + 1) < (cs.len))) && (strcmp(((const char*)cs.data[(i + 1)]), "/") == 0));  }
long line_end(MacaList cs, long i) { return (((i < (cs.len)) && (strcmp(((const char*)cs.data[i]), "\n") != 0)) ? line_end(cs, (i + 1)) : i);  }
long triple(MacaList cs, long i) { return (((strcmp(((const char*)cs.data[i]), "\"") == 0) && (strcmp(at_or_blank(cs, (i + 1)), "\"") == 0)) && (strcmp(at_or_blank(cs, (i + 2)), "\"") == 0));  }
long raw_end(MacaList cs, long i) { return (((i + 2) >= (cs.len)) ? (cs.len) : (triple(cs, i) ? i : raw_end(cs, (i + 1))));  }
Lexed lex_raw(MacaList cs, long i, Lexed out) { long j = raw_end(cs, (i + 3)); const char* text = escape_raw(maca_chars(span(cs, (i + 3), j)), 0, ""); Lexed held = keep(unterminated(out, cs, j), mk_token(TStr, text, i)); return moved(held, (j + 3));  }
const char* escape_raw(MacaList cs, long i, const char* acc) { return ((i >= (cs.len)) ? acc : escape_raw(cs, (i + 1), maca_cat(acc, raw_char(((const char*)cs.data[i])))));  }
const char* raw_char(const char* c) { return ((strcmp(c, "\\") == 0) ? "\\\\" : ((strcmp(c, "\"") == 0) ? "\\\"" : ((strcmp(c, "\n") == 0) ? "\\n" : ((strcmp(c, "\r") == 0) ? "\\r" : ((strcmp(c, "\t") == 0) ? "\\t" : ((strcmp(c, "{") == 0) ? "{{" : ((strcmp(c, "}") == 0) ? "}}" : c)))))));  }
Lexed lex_string(MacaList cs, long i, Lexed out) { long j = string_end(cs, (i + 1)); const char* text = span(cs, (i + 1), j); Lexed held = keep(unterminated(out, cs, j), mk_token(TStr, text, i)); return moved(held, (j + 1));  }
long string_end(MacaList cs, long i) { return ((i >= (cs.len)) ? i : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? string_end(cs, (i + 2)) : ((doubled(cs, i, "{") || doubled(cs, i, "}")) ? string_end(cs, (i + 2)) : ((strcmp(((const char*)cs.data[i]), "{") == 0) ? string_end(cs, interp_end(cs, (i + 1), 1)) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? i : string_end(cs, (i + 1)))))));  }
long doubled(MacaList cs, long i, const char* c) { return (((strcmp(((const char*)cs.data[i]), c) == 0) && ((i + 1) < (cs.len))) && (strcmp(((const char*)cs.data[(i + 1)]), c) == 0));  }
long interp_end(MacaList cs, long i, long depth) { return ((i >= (cs.len)) ? i : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? interp_end(cs, (i + 2), depth) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? interp_end(cs, (quoted_end(cs, (i + 1)) + 1), depth) : ((strcmp(((const char*)cs.data[i]), "{") == 0) ? interp_end(cs, (i + 1), (depth + 1)) : (((strcmp(((const char*)cs.data[i]), "}") == 0) && (depth == 1)) ? (i + 1) : ((strcmp(((const char*)cs.data[i]), "}") == 0) ? interp_end(cs, (i + 1), (depth - 1)) : interp_end(cs, (i + 1), depth)))))));  }
long quoted_end(MacaList cs, long i) { return ((i >= (cs.len)) ? i : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? quoted_end(cs, (i + 2)) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? i : quoted_end(cs, (i + 1)))));  }
Lexed lex_word(MacaList cs, long i, Lexed out) { long j = word_end(cs, run_end(cs, i, (MacaFn){ (void*)is_alnum, (MacaList){0, 0} })); const char* text = span(cs, i, j); return moved(keep(out, mk_token(keyword_kind(text), text, i)), j);  }
long word_end(MacaList cs, long j) { return (((((j + 1) < (cs.len)) && (strcmp(((const char*)cs.data[j]), "-") == 0)) && is_alpha(((const char*)cs.data[(j + 1)]))) ? word_end(cs, run_end(cs, (j + 1), (MacaFn){ (void*)is_alnum, (MacaList){0, 0} })) : j);  }
Lexed lex_number(MacaList cs, long i, Lexed out) { long j = run_end(cs, i, (MacaFn){ (void*)is_digit, (MacaList){0, 0} }); long has_fraction = ((((j + 1) < (cs.len)) && (strcmp(((const char*)cs.data[j]), ".") == 0)) && is_digit(((const char*)cs.data[(j + 1)]))); return (based(cs, i) ? lex_based(cs, i, out) : (has_fraction ? lex_float(cs, i, j, out) : moved(keep(out, mk_token(TInt, span(cs, i, j), i)), j)));  }
long based(MacaList cs, long i) { return (((strcmp(((const char*)cs.data[i]), "0") != 0) || ((i + 2) >= (cs.len))) ? 0 : ({ const char* marker = ((const char*)cs.data[(i + 1)]); (((strcmp(marker, "x") == 0) || (strcmp(marker, "b") == 0)) && is_base_digit(((const char*)cs.data[(i + 2)]))); }));  }
long is_base_digit(const char* c) { long v = maca_ord(c); return ((is_digit(c) || ((v >= 97) && (v <= 102))) || ((v >= 65) && (v <= 70)));  }
Lexed lex_based(MacaList cs, long i, Lexed out) { long j = run_end(cs, (i + 2), (MacaFn){ (void*)is_base_digit, (MacaList){0, 0} }); long n = from_base(cs, (i + 2), j, ((strcmp(((const char*)cs.data[(i + 1)]), "x") == 0) ? 16 : 2), 0); return moved(keep(out, mk_token(TInt, maca_int_to_str(n), i)), j);  }
long from_base(MacaList cs, long i, long j, long base, long acc) { return ((i >= j) ? acc : from_base(cs, (i + 1), j, base, ((acc * base) + digit_value(((const char*)cs.data[i])))));  }
long digit_value(const char* c) { long v = maca_ord(c); return (is_digit(c) ? atol(c) : ((v >= 97) ? ((v - 97) + 10) : ((v - 65) + 10)));  }
Lexed lex_float(MacaList cs, long i, long dot, Lexed out) { long k = run_end(cs, (dot + 1), (MacaFn){ (void*)is_digit, (MacaList){0, 0} }); return moved(keep(out, mk_token(TFloat, span(cs, i, k), i)), k);  }
const char* at_or_blank(MacaList cs, long i) { return ((i < (cs.len)) ? ((const char*)cs.data[i]) : "");  }
Lexed lex_punct(MacaList cs, long i, Lexed out, const char* c) { const char* two = maca_cat(c, at_or_blank(cs, (i + 1))); const char* three = maca_cat(two, at_or_blank(cs, (i + 2))); return ((strcmp(three, "...") == 0) ? moved(keep(out, mk_token(Ellipsis, three, i)), (i + 3)) : (is_two_char(two) ? moved(keep(out, mk_token(two_char_kind(two), two, i)), (i + 2)) : moved(keep(unknown(out, c, i), mk_token(one_char_kind(cs, i, c), c, i)), (i + 1))));  }
Kind one_char_kind(MacaList cs, long i, const char* c) { return ((((strcmp(c, "?") == 0) && (i > 0)) && (!is_space(((const char*)cs.data[(i - 1)])))) ? QuestionPost : punct_kind(c));  }
long is_two_char(const char* s) { return (((((((((((((strcmp(s, "->") == 0) || (strcmp(s, "=>") == 0)) || (strcmp(s, "==") == 0)) || (strcmp(s, "!=") == 0)) || (strcmp(s, "<=") == 0)) || (strcmp(s, ">=") == 0)) || (strcmp(s, "++") == 0)) || (strcmp(s, "&&") == 0)) || (strcmp(s, "||") == 0)) || (strcmp(s, "<<") == 0)) || (strcmp(s, ">>") == 0)) || (strcmp(s, "..") == 0)) || (strcmp(s, "|>") == 0));  }
Kind two_char_kind(const char* s) { return ((strcmp(s, "..") == 0) ? DotDot : ((strcmp(s, "->") == 0) ? Arrow : ((strcmp(s, "=>") == 0) ? FatArrow : ((strcmp(s, "==") == 0) ? EqEq : ((strcmp(s, "!=") == 0) ? NotEq : ((strcmp(s, "<=") == 0) ? Le : ((strcmp(s, ">=") == 0) ? Ge : ((strcmp(s, "<<") == 0) ? Shl : ((strcmp(s, ">>") == 0) ? Shr : ((strcmp(s, "++") == 0) ? PlusPlus : ((strcmp(s, "&&") == 0) ? AmpAmp : ((strcmp(s, "||") == 0) ? PipePipe : ((strcmp(s, "|>") == 0) ? PipeGt : TIdent)))))))))))));  }
Kind punct_kind(const char* c) { return ((strcmp(c, "(") == 0) ? LParen : ((strcmp(c, ")") == 0) ? RParen : ((strcmp(c, "[") == 0) ? LBracket : ((strcmp(c, "]") == 0) ? RBracket : ((strcmp(c, "{") == 0) ? LBrace : ((strcmp(c, "}") == 0) ? RBrace : ((strcmp(c, ",") == 0) ? Comma : ((strcmp(c, ":") == 0) ? Colon : ((strcmp(c, ".") == 0) ? Dot : ((strcmp(c, "=") == 0) ? Eq : ((strcmp(c, "+") == 0) ? Plus : ((strcmp(c, "-") == 0) ? Minus : ((strcmp(c, "*") == 0) ? Star : ((strcmp(c, "/") == 0) ? Slash : ((strcmp(c, "%") == 0) ? Percent : ((strcmp(c, "|") == 0) ? Bar : ((strcmp(c, "?") == 0) ? Question : ((strcmp(c, "!") == 0) ? Bang : ((strcmp(c, "<") == 0) ? Lt : ((strcmp(c, ">") == 0) ? Gt : TIdent))))))))))))))))))));  }
Lexed unterminated(Lexed out, MacaList cs, long j) { return ((j >= (cs.len)) ? note_error(out, "unterminated string") : out);  }
Lexed unknown(Lexed out, const char* c, long i) { return ((punct_kind(c) == TIdent) ? note_error(out, maca_cat_own(maca_cat_own(maca_cat("unexpected character `", c), "` at ", 1), maca_int_to_str(i), 3)) : out);  }
Expr e_int(long n) { return (Expr){ .kind = EInt, .text = "", .ival = n, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_ident(const char* name) { return (Expr){ .kind = EIdent, .text = name, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_bad(const char* text) { return (Expr){ .kind = EBad, .text = text, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_param(const char* name, const char* ty) { return (Expr){ .kind = EIdent, .text = name, .ival = 0, .ty = ty, .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_typed(const char* name, const char* ty) { return (Expr){ .kind = EIdent, .text = name, .ival = 0, .ty = ty, .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr with_child(Expr e, Expr c) { return (Expr){ .kind = e.kind, .text = e.text, .ival = e.ival, .ty = e.ty, .children = maca_list_cat(e.children, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ c }))), .stmts = e.stmts };  }
Expr e_str(const char* s) { return (Expr){ .kind = EStr, .text = s, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_bool(const char* text) { return (Expr){ .kind = EBool, .text = text, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_float(const char* text) { return (Expr){ .kind = EFloat, .text = text, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_call(const char* callee, MacaList args) { return (Expr){ .kind = ECall, .text = callee, .ival = 0, .ty = "", .children = args, .stmts = maca_listv(0) };  }
Expr e_binary(const char* op, Expr lhs, Expr rhs) { return (Expr){ .kind = EBinary, .text = op, .ival = 0, .ty = "", .children = maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ lhs }), maca_box(sizeof(Expr), (Expr[]){ rhs })), .stmts = maca_listv(0) };  }
Expr e_ternary(Expr cond, Expr then, Expr els) { return (Expr){ .kind = ETernary, .text = "", .ival = 0, .ty = "", .children = maca_listv(3, maca_box(sizeof(Expr), (Expr[]){ cond }), maca_box(sizeof(Expr), (Expr[]){ then }), maca_box(sizeof(Expr), (Expr[]){ els })), .stmts = maca_listv(0) };  }
Expr e_if(Expr cond, Expr then, Expr els) { return (Expr){ .kind = EIf, .text = "", .ival = 0, .ty = "", .children = maca_listv(3, maca_box(sizeof(Expr), (Expr[]){ cond }), maca_box(sizeof(Expr), (Expr[]){ then }), maca_box(sizeof(Expr), (Expr[]){ els })), .stmts = maca_listv(0) };  }
Expr e_unary(const char* op, Expr operand) { return (Expr){ .kind = EUnary, .text = op, .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ operand })), .stmts = maca_listv(0) };  }
Expr e_record(const char* tyname, MacaList fields) { return (Expr){ .kind = ERecord, .text = tyname, .ival = 0, .ty = "", .children = fields, .stmts = maca_listv(0) };  }
Expr e_with(Expr base, MacaList fields) { return (Expr){ .kind = EWith, .text = "", .ival = 0, .ty = "", .children = maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ base })), fields), .stmts = maca_listv(0) };  }
Expr e_field(Expr base, const char* name) { return (Expr){ .kind = EField, .text = name, .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ base })), .stmts = maca_listv(0) };  }
Expr e_match(MacaList children) { return (Expr){ .kind = EMatch, .text = "", .ival = 0, .ty = "", .children = children, .stmts = maca_listv(0) };  }
Expr e_method(Expr recv, const char* name, MacaList args) { return (Expr){ .kind = EMethod, .text = name, .ival = 0, .ty = "", .children = maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ recv })), args), .stmts = maca_listv(0) };  }
Expr e_attr(const char* name, Expr value) { return (Expr){ .kind = EAttr, .text = name, .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ value })), .stmts = maca_listv(0) };  }
Expr e_list(MacaList elems) { return (Expr){ .kind = EList, .text = "", .ival = 0, .ty = "", .children = elems, .stmts = maca_listv(0) };  }
Stmt s_expr(Expr value) { return (Stmt){ .kind = SExpr, .name = "", .value = value, .ret = "", .pos = 0, .params = maca_listv(0), .body = maca_listv(0), .frozen = 0 };  }
Stmt s_bind(const char* name, Expr value) { return (Stmt){ .kind = SBind, .name = name, .value = value, .ret = "", .pos = 0, .params = maca_listv(0), .body = maca_listv(0), .frozen = 0 };  }
Stmt s_bind_typed(const char* name, const char* ty, Expr value) { return (Stmt){ .kind = SBind, .name = name, .value = value, .ret = ty, .pos = 0, .params = maca_listv(0), .body = maca_listv(0), .frozen = 0 };  }
Stmt s_fn(const char* name, const char* ret, MacaList params, MacaList body) { return (Stmt){ .kind = SFn, .name = name, .value = e_ident(name), .ret = ret, .pos = 0, .params = params, .body = body, .frozen = 0 };  }
Stmt s_record(const char* name, MacaList fields) { return (Stmt){ .kind = SRecord, .name = name, .value = e_ident(name), .ret = "", .pos = 0, .params = fields, .body = maca_listv(0), .frozen = 0 };  }
long is_impl_block(Stmt s) { return (((((s.kind == SBind) && upper_word(s.ret)) && (s.value.kind == ERecord)) && ((s.value.children.len) > 0)) && all_lambda_fields(s.value.children, 0));  }
long foreign_type(const char* ty, MacaList own) { return ((((maca_str_index_of(ty, "[]") < 0) && upper_word(ty)) && (maca_list_index_of_str(own, head_type(ty)) < 0)) && upper_word(head_type(ty)));  }
const char* head_type(const char* ty) { long cut = maca_str_index_of(ty, " "); return ((cut < 0) ? ty : maca_str_slice(ty, 0, cut));  }
MacaList declared_types(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : ((((*(Stmt*)items.data[i]).kind == SRecord) || ((*(Stmt*)items.data[i]).kind == SSum)) ? declared_types(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : declared_types(items, (i + 1), acc)));  }
long upper_word(const char* w) { return (((((int)strlen(w)) > 0) && (isalpha((unsigned char)(maca_str_at(w, 0))[0]) != 0)) && (strcmp(maca_upper(maca_str_at(w, 0)), maca_str_at(w, 0)) == 0));  }
long all_lambda_fields(MacaList fs, long i) { return ((i >= (fs.len)) ? 1 : (((((*(Expr*)fs.data[i]).children.len) < 2) || ((*(Expr*)(*(Expr*)fs.data[i]).children.data[1]).kind != ELambda)) ? 0 : all_lambda_fields(fs, (i + 1))));  }
Stmt s_sum(const char* name, MacaList variants) { return (Stmt){ .kind = SSum, .name = name, .value = e_ident(name), .ret = "", .pos = 0, .params = variants, .body = maca_listv(0), .frozen = 0 };  }
const char* show(Expr e) { return (e.kind == EInt ? maca_int_to_str(e.ival) : (e.kind == EFloat ? e.text : (e.kind == EStr ? e.text : (e.kind == EBool ? e.text : (e.kind == EIdent ? e.text : (e.kind == ECall ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), "(", 1), show_args(e.children, 0), 1), ")", 1) : (e.kind == EBinary ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", show((*(Expr*)e.children.data[0]))), " ", 1), e.text, 1), " ", 1), show((*(Expr*)e.children.data[1])), 1), ")", 1) : (e.kind == ETernary ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", show((*(Expr*)e.children.data[0]))), " ? ", 1), show((*(Expr*)e.children.data[1])), 1), " : ", 1), show((*(Expr*)e.children.data[2])), 1), ")", 1) : (e.kind == EIf ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(if ", show((*(Expr*)e.children.data[0]))), " then ", 1), show((*(Expr*)e.children.data[1])), 1), " else ", 1), show((*(Expr*)e.children.data[2])), 1), ")", 1) : (e.kind == EUnary ? maca_cat_own(maca_cat_own(maca_cat("(", e.text), show((*(Expr*)e.children.data[0])), 1), ")", 1) : (e.kind == ERecord ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), " { ", 1), show_args(e.children, 0), 1), " }", 1) : (e.kind == EWith ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", show((*(Expr*)e.children.data[0]))), " with { ", 1), show_args(e.children, 1), 1), " }", 1) : (e.kind == EField ? maca_cat_own(maca_cat_own(maca_cat("", show((*(Expr*)e.children.data[0]))), ".", 1), e.text, 1) : (e.kind == EMatch ? maca_cat_own(maca_cat("match ", show((*(Expr*)e.children.data[0]))), " { ... }", 1) : (e.kind == EMethod ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", show((*(Expr*)e.children.data[0]))), ".", 1), e.text, 1), "(", 1), show_args(e.children, 1), 1), ")", 1) : (e.kind == EList ? maca_cat_own(maca_cat("[", show_args(e.children, 0)), "]", 1) : (e.kind == EAttr ? maca_cat_own(maca_cat_own(maca_cat("", e.text), "=", 1), show((*(Expr*)e.children.data[0])), 1) : (e.kind == EJump ? e.text : (e.kind == EWhile ? maca_cat_own(maca_cat("while ", show((*(Expr*)e.children.data[0]))), " { ... }", 1) : (e.kind == EFor ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("for ", e.text), " in ", 1), show((*(Expr*)e.children.data[0])), 1), " { ... }", 1) : "?"))))))))))))))))))));  }
const char* show_args(MacaList xs, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(xs, i, (xs.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(show((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
Expr e_guard(Expr pat, Expr when) { return (Expr){ .kind = EGuard, .text = pat.text, .ival = 0, .ty = "", .children = maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ pat }), maca_box(sizeof(Expr), (Expr[]){ when })), .stmts = maca_listv(0) };  }
Expr e_while(Expr cond, MacaList body) { return (Expr){ .kind = EWhile, .text = "", .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ cond })), .stmts = body };  }
Expr e_lambda(MacaList params, Expr body) { return (Expr){ .kind = ELambda, .text = "", .ival = 0, .ty = "", .children = maca_list_cat(params, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ body }))), .stmts = maca_listv(0) };  }
Expr lambda_body(Expr e) { return (*(Expr*)e.children.data[((e.children.len) - 1)]);  }
MacaList lambda_params(Expr e) { return maca_list_slice(e.children, 0, ((e.children.len) - 1));  }
Expr e_for(const char* binder, Expr over, MacaList body) { return (Expr){ .kind = EFor, .text = binder, .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ over })), .stmts = body };  }
Expr e_jump(const char* word) { return (Expr){ .kind = EJump, .text = word, .ival = 0, .ty = "", .children = maca_listv(0), .stmts = maca_listv(0) };  }
Expr e_leave(MacaList value) { return (Expr){ .kind = EJump, .text = "return", .ival = 0, .ty = "", .children = value, .stmts = maca_listv(0) };  }
Expr e_block(MacaList stmts, Expr last) { return (Expr){ .kind = EBlock, .text = "", .ival = 0, .ty = "", .children = maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ last })), .stmts = stmts };  }
Stmt at_pos(Stmt s, long p) { return ({ __typeof__(s) _w = s; _w.pos = p; _w; });  }
const char* map_type_key(const char* ty) { const char* rest = map_type_rest(ty); long cut = maca_str_index_of(rest, " "); return ((cut < 0) ? "" : maca_str_slice(rest, 0, cut));  }
const char* map_type_val(const char* ty) { const char* rest = map_type_rest(ty); long cut = maca_str_index_of(rest, " "); return ((cut < 0) ? "" : maca_str_slice(rest, (cut + 1), ((int)strlen(rest))));  }
const char* map_type_rest(const char* ty) { return (((((int)strlen(ty)) > 4) && (strcmp(maca_str_slice(ty, 0, 4), "Map ") == 0)) ? maca_str_slice(ty, 4, ((int)strlen(ty))) : "");  }
PExpr mk_pexpr(Expr node, long next) { return (PExpr){ .node = node, .next = next };  }
PStmt mk_pstmt(Stmt snode, long snext) { return (PStmt){ .snode = snode, .snext = snext };  }
PParams mk_pparams(MacaList params, long pnext) { return (PParams){ .params = params, .pnext = pnext };  }
PBlock mk_pblock(MacaList bstmts, long bnext) { return (PBlock){ .bstmts = bstmts, .bnext = bnext };  }
PArgs mk_pargs(MacaList aitems, long anext) { return (PArgs){ .aitems = aitems, .anext = anext };  }
PStmt parse_fn(MacaList ts, long i) { const char* name = (*(Token*)ts.data[i]).text; PParams pp = parse_params(ts, (i + 2), maca_listv(0)); long afterp = (pp.pnext + 1); long body_at = after_return_type(ts, afterp); return (((*(Token*)ts.data[body_at]).kind == LBrace) ? mk_block_fn(ts, name, return_type(ts, afterp), pp.params, body_at) : (((*(Token*)ts.data[body_at]).kind == FatArrow) ? mk_arrow_fn(ts, name, return_type(ts, afterp), pp.params, body_at) : mk_pstmt(s_fn(name, return_type(ts, afterp), pp.params, maca_listv(0)), body_at)));  }
PType mk_ptype(const char* tname, long tnext) { return (PType){ .tname = tname, .tnext = tnext };  }
PType scan_type(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == LParen) ? fn_type(ts, i, paren_end(ts, (i + 1), 0)) : type_post(ts, type_apply(ts, (i + 1), (*(Token*)ts.data[i]).text)));  }
PType fn_type(MacaList ts, long i, long shut) { const char* args = fn_type_args(ts, (i + 1), shut, ""); return (((*(Token*)ts.data[(shut + 1)]).kind != Arrow) ? mk_ptype("", (shut + 1)) : ({ PType ret = scan_type(ts, (shut + 2)); mk_ptype(maca_cat_own(maca_cat_own(maca_cat("(", args), ") -> ", 1), ret.tname, 1), ret.tnext); }));  }
const char* fn_type_args(MacaList ts, long i, long shut, const char* acc) { return ((i >= shut) ? acc : ({ PType one = scan_type(ts, i); const char* sep = ((strcmp(acc, "") == 0) ? "" : ", "); fn_type_args(ts, skip_comma(ts, one.tnext), shut, maca_cat_own(maca_cat(acc, sep), one.tname, 1)); }));  }
PType type_apply(MacaList ts, long i, const char* head) { return ((is_upper(head) && type_arg_at(ts, i)) ? type_apply(ts, (i + 1), maca_cat_own(maca_cat(head, " "), (*(Token*)ts.data[i]).text, 1)) : mk_ptype(head, i));  }
long type_arg_at(MacaList ts, long i) { const char* w = (*(Token*)ts.data[i]).text; long shaped = ((is_prim_name(w) || is_upper(w)) || is_tyvar_name(w)); return ((((*(Token*)ts.data[i]).kind == TIdent) && ((*(Token*)ts.data[(i + 1)]).kind != Colon)) && shaped);  }
long is_prim_name(const char* w) { return (((((strcmp(w, "int") == 0) || (strcmp(w, "float") == 0)) || (strcmp(w, "str") == 0)) || (strcmp(w, "bool") == 0)) || (strcmp(w, "bytes") == 0));  }
long is_tyvar_name(const char* w) { return ((((int)strlen(w)) == 1) && (!is_upper(w)));  }
PType type_post(MacaList ts, PType t) { long listed = (((*(Token*)ts.data[t.tnext]).kind == LBracket) && ((*(Token*)ts.data[(t.tnext + 1)]).kind == RBracket)); long dotted = (((*(Token*)ts.data[t.tnext]).kind == Dot) && ((*(Token*)ts.data[(t.tnext + 1)]).kind == TIdent)); return (listed ? type_post(ts, mk_ptype(maca_cat(t.tname, "[]"), (t.tnext + 2))) : (dotted ? type_post(ts, mk_ptype(maca_cat_own(maca_cat_own(maca_cat("", t.tname), ".", 1), (*(Token*)ts.data[(t.tnext + 1)]).text, 1), (t.tnext + 2))) : t));  }
const char* return_type(MacaList ts, long at) { return (((*(Token*)ts.data[at]).kind != Arrow) ? "" : scan_type(ts, (at + 1)).tname);  }
long after_return_type(MacaList ts, long at) { return (((*(Token*)ts.data[at]).kind != Arrow) ? at : scan_type(ts, (at + 1)).tnext);  }
PStmt mk_arrow_fn(MacaList ts, const char* name, const char* ret, MacaList params, long at) { PExpr be = parse_list_expr(ts, (at + 1)); return mk_pstmt(s_fn(name, ret, params, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(be.node) }))), be.next);  }
PExpr parse_list_expr(MacaList ts, long i) { PExpr e = parse_expr(ts, i); return (((*(Token*)ts.data[e.next]).kind != Comma) ? e : more_elems(ts, (e.next + 1), maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e.node }))));  }
PExpr more_elems(MacaList ts, long i, MacaList acc) { PExpr e = parse_expr(ts, i); MacaList seen = maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e.node }))); return (((*(Token*)ts.data[e.next]).kind == Comma) ? more_elems(ts, (e.next + 1), seen) : mk_pexpr(e_list(seen), e.next));  }
PStmt mk_block_fn(MacaList ts, const char* name, const char* ret, MacaList params, long at) { PBlock pb = parse_block(ts, (at + 1), maca_listv(0)); return mk_pstmt(s_fn(name, ret, params, pb.bstmts), pb.bnext);  }
PBlock parse_block(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? mk_pblock(acc, (i + 1)) : (starts_local_fn(ts, i) ? parse_local_fn(ts, i, acc) : ((binds_a_name(ts, i) || is_const_word(ts, i)) ? parse_bind_stmt(ts, i, acc) : parse_expr_stmt(ts, i, acc))));  }
long starts_local_fn(MacaList ts, long i) { return ((!starts_fn(ts, i)) ? 0 : ({ long at = after_return_type(ts, (paren_end(ts, (i + 2), 0) + 1)); (((*(Token*)ts.data[at]).kind == FatArrow) || ((*(Token*)ts.data[at]).kind == LBrace)); }));  }
PBlock parse_local_fn(MacaList ts, long i, MacaList acc) { PStmt made = parse_fn(ts, i); Stmt held = made.snode; return parse_block(ts, made.snext, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_bind(held.name, e_lambda(held.params, body_expr(held.body))) }))));  }
Expr body_expr(MacaList stmts) { long n = (stmts.len); Stmt last = (*(Stmt*)stmts.data[(n - 1)]); return (((n == 1) && (last.kind == SExpr)) ? last.value : ((last.kind == SExpr) ? e_block(maca_list_slice(stmts, 0, (n - 1)), last.value) : e_block(stmts, e_ident(last.name))));  }
PBlock parse_bind_stmt(MacaList ts, long at, MacaList acc) { long i = (is_const_word(ts, at) ? (at + 1) : at); PExpr v = parse_list_expr(ts, (bind_eq_at(ts, (i + 1)) + 1)); Stmt made = s_bind_typed((*(Token*)ts.data[i]).text, bind_type(ts, (i + 1)), v.node); Stmt kept = ({ __typeof__(made) _w = made; _w.frozen = ((i > at) || says_as_const(ts, v.next)); _w; }); return parse_block(ts, past_as_const(ts, v.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ kept }))));  }
PBlock parse_expr_stmt(MacaList ts, long i, MacaList acc) { PExpr e = parse_expr(ts, i); return (((*(Token*)ts.data[e.next]).kind == Eq) ? parse_store_stmt(ts, e, acc) : parse_block(ts, e.next, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(e.node) })))));  }
PBlock parse_store_stmt(MacaList ts, PExpr target, MacaList acc) { PExpr v = parse_expr(ts, (target.next + 1)); Stmt set = s_expr(e_binary("=", target.node, v.node)); return parse_block(ts, v.next, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ set }))));  }
Module parse_module(MacaList ts, long i, MacaList acc) { return parse_items(ts, i, acc, maca_listv(0));  }
Module parse_items(MacaList ts, long i, MacaList acc, MacaList bad) { return (((*(Token*)ts.data[i]).kind == Eof) ? (Module){ .items = acc, .errors = bad } : (((*(Token*)ts.data[i]).kind == KwImport) ? parse_items(ts, import_end(ts, (i + 1)), acc, bad) : (((*(Token*)ts.data[i]).kind == KwAlias) ? parse_items(ts, parse_expr(ts, (i + 3)).next, acc, bad) : (is_const_word(ts, i) ? parse_items(ts, (i + 1), acc, bad) : (is_record_decl(ts, i) ? stamped(parse_record_decl(ts, i, acc, bad), (acc.len), (*(Token*)ts.data[i]).pos) : (is_sum_decl(ts, i) ? stamped(parse_sum_decl(ts, i, acc, bad), (acc.len), (*(Token*)ts.data[i]).pos) : (starts_fn(ts, i) ? stamped(parse_fn_item(ts, i, acc, bad), (acc.len), (*(Token*)ts.data[i]).pos) : (binds_a_name(ts, i) ? stamped(parse_const_item(ts, i, acc, bad), (acc.len), (*(Token*)ts.data[i]).pos) : (binds_a_path(ts, i) ? stamped(parse_path_item(ts, i, acc, bad), (acc.len), (*(Token*)ts.data[i]).pos) : parse_items(ts, (i + 1), acc, maca_list_cat(bad, maca_listv(1, (long)(skipped(ts, i))))))))))))));  }
Module stamped(Module m, long at, long p) { return ((at >= (m.items.len)) ? m : ({ __typeof__(m) _w = m; _w.items = maca_list_set(m.items, at, maca_box(sizeof(Stmt), (Stmt[]){ at_pos((*(Stmt*)m.items.data[at]), p) })); _w; }));  }
Module parse_const_item(MacaList ts, long i, MacaList acc, MacaList bad) { long at = bind_eq_at(ts, (i + 1)); PExpr got = parse_list_expr(ts, (at + 1)); Stmt made = s_bind_typed((*(Token*)ts.data[i]).text, bind_type(ts, (i + 1)), got.node); return parse_items(ts, past_as_const(ts, got.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ made }))), bad);  }
Module parse_path_item(MacaList ts, long i, MacaList acc, MacaList bad) { long end = path_end(ts, (i + 1)); PExpr got = parse_list_expr(ts, (bind_eq_at(ts, end) + 1)); Stmt made = s_bind_typed(dotted_name(ts, i, end, ""), bind_type(ts, end), got.node); return parse_items(ts, got.next, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ made }))), bad);  }
long binds_a_path(MacaList ts, long i) { long end = path_end(ts, (i + 1)); return ((((*(Token*)ts.data[i]).kind == TIdent) && (end > (i + 1))) && ((*(Token*)ts.data[bind_eq_at(ts, end)]).kind == Eq));  }
long path_end(MacaList ts, long i) { return ((((*(Token*)ts.data[i]).kind == Dot) && ((*(Token*)ts.data[(i + 1)]).kind == TIdent)) ? path_end(ts, (i + 2)) : i);  }
const char* dotted_name(MacaList ts, long i, long end, const char* acc) { return ((i >= end) ? acc : dotted_name(ts, (i + 1), end, maca_cat(acc, (*(Token*)ts.data[i]).text)));  }
const char* bind_type(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind != Colon) ? "" : scan_type(ts, (i + 1)).tname);  }
const char* skipped(MacaList ts, long i) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("no declaration starts at `", (*(Token*)ts.data[i]).text), "` (offset ", 1), maca_int_to_str((*(Token*)ts.data[i]).pos), 3), ")", 1);  }
long starts_fn(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == TIdent) && ((*(Token*)ts.data[(i + 1)]).kind == LParen));  }
long bind_eq_at(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind != Colon) ? i : bind_eq_at(ts, scan_type(ts, (i + 1)).tnext));  }
long binds_a_name(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == TIdent) && ((*(Token*)ts.data[bind_eq_at(ts, (i + 1))]).kind == Eq));  }
long is_const_word(MacaList ts, long i) { return ((((*(Token*)ts.data[i]).kind == TIdent) && (strcmp((*(Token*)ts.data[i]).text, "const") == 0)) && binds_a_name(ts, (i + 1)));  }
long says_as_const(MacaList ts, long i) { return ((strcmp((*(Token*)ts.data[i]).text, "as") == 0) && (strcmp((*(Token*)ts.data[(i + 1)]).text, "const") == 0));  }
long past_as_const(MacaList ts, long i) { return (says_as_const(ts, i) ? (i + 2) : i);  }
long is_record_decl(MacaList ts, long i) { long at = bind_eq_at(ts, (i + 1)); return ((binds_a_name(ts, i) && ((*(Token*)ts.data[(at + 1)]).kind == LBrace)) && typed_fields(ts, (at + 2)));  }
long typed_fields(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == RBrace) || (((*(Token*)ts.data[i]).kind == TIdent) && ((*(Token*)ts.data[(i + 1)]).kind == Colon)));  }
long is_sum_decl(MacaList ts, long i) { long at = bind_eq_at(ts, (i + 1)); return (((!binds_a_name(ts, i)) || ((*(Token*)ts.data[(at + 1)]).kind != TIdent)) ? 0 : ((!is_upper((*(Token*)ts.data[(at + 1)]).text)) ? 0 : (((*(Token*)ts.data[(at + 2)]).kind == Bar) || ((*(Token*)ts.data[(at + 2)]).kind == LParen))));  }
long is_upper(const char* w) { return (((((int)strlen(w)) > 0) && (isalpha((unsigned char)(maca_str_at(w, 0))[0]) != 0)) && (strcmp(maca_upper(maca_str_at(w, 0)), maca_str_at(w, 0)) == 0));  }
long import_end(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == LBrace) ? import_end(ts, (selection_end(ts, (i + 1)) + 2)) : (((*(Token*)ts.data[i]).kind == TStr) ? (i + 1) : (((*(Token*)ts.data[(i + 1)]).kind == TStr) ? (i + 2) : (((*(Token*)ts.data[(i + 1)]).kind == Slash) ? import_end(ts, (i + 2)) : (i + 1)))));  }
long selection_end(MacaList ts, long i) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? i : selection_end(ts, (i + 1)));  }
Module parse_sum_decl(MacaList ts, long i, MacaList acc, MacaList bad) { const char* name = (*(Token*)ts.data[i]).text; PParams sv = parse_variants(ts, (bind_eq_at(ts, (i + 1)) + 1), maca_listv(0)); return parse_items(ts, sv.pnext, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_sum(name, sv.params) }))), bad);  }
PParams parse_variants(MacaList ts, long i, MacaList acc) { return (((*(Token*)ts.data[i]).kind != TIdent) ? mk_pparams(acc, i) : one_variant(ts, i, acc));  }
PParams one_variant(MacaList ts, long i, MacaList acc) { PExpr v = parse_variant(ts, i); MacaList seen = maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ v.node }))); return (((*(Token*)ts.data[v.next]).kind == Bar) ? parse_variants(ts, (v.next + 1), seen) : mk_pparams(seen, v.next));  }
PExpr parse_variant(MacaList ts, long i) { Expr named = e_ident((*(Token*)ts.data[i]).text); return (((*(Token*)ts.data[(i + 1)]).kind == LParen) ? parse_payload(ts, (i + 2), named) : mk_pexpr(named, (i + 1)));  }
PExpr parse_payload(MacaList ts, long i, Expr v) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RParen)) ? mk_pexpr(v, (i + 1)) : (((*(Token*)ts.data[i]).kind == Comma) ? parse_payload(ts, (i + 1), v) : ({ Expr ty = e_typed((*(Token*)ts.data[i]).text, (*(Token*)ts.data[i]).text); parse_payload(ts, (i + 1), with_child(v, ty)); })));  }
Module parse_fn_item(MacaList ts, long i, MacaList acc, MacaList bad) { PStmt ps = parse_fn(ts, i); return parse_items(ts, ps.snext, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ps.snode }))), bad);  }
Module parse_record_decl(MacaList ts, long i, MacaList acc, MacaList bad) { const char* name = (*(Token*)ts.data[i]).text; PParams rf = parse_fields(ts, (bind_eq_at(ts, (i + 1)) + 2), maca_listv(0)); return parse_items(ts, (rf.pnext + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_record(name, rf.params) }))), bad);  }
PParams parse_fields(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? mk_pparams(acc, i) : parse_one_field(ts, i, acc));  }
PParams parse_one_field(MacaList ts, long i, MacaList acc) { PType t = scan_type(ts, (i + 2)); MacaList seen = maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_param((*(Token*)ts.data[i]).text, t.tname) }))); return parse_fields(ts, skip_comma(ts, t.tnext), seen);  }
long skip_comma(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == Comma) ? (i + 1) : i);  }
PParams parse_params(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RParen)) ? mk_pparams(acc, i) : parse_one_param(ts, i, acc));  }
PParams parse_one_param(MacaList ts, long i, MacaList acc) { return (((*(Token*)ts.data[i]).kind == Ellipsis) ? parse_params(ts, skip_comma(ts, after_param_type(ts, (i + 1))), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ rest_param(ts, (i + 1)) })))) : parse_params(ts, skip_comma(ts, after_param_type(ts, i)), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_param((*(Token*)ts.data[i]).text, param_type(ts, i)) })))));  }
Expr rest_param(MacaList ts, long i) { return ({ __typeof__(e_param((*(Token*)ts.data[i]).text, maca_cat(param_type(ts, i), "[]"))) _w = e_param((*(Token*)ts.data[i]).text, maca_cat(param_type(ts, i), "[]")); _w.ival = 1; _w; });  }
const char* param_type(MacaList ts, long i) { return (((*(Token*)ts.data[(i + 1)]).kind != Colon) ? "" : scan_type(ts, (i + 2)).tname);  }
long after_param_type(MacaList ts, long i) { return (((*(Token*)ts.data[(i + 1)]).kind != Colon) ? (i + 1) : scan_type(ts, (i + 2)).tnext);  }
long prec_of(Kind k) { long is_cmp = ((((((k == EqEq) || (k == NotEq)) || (k == Lt)) || (k == Gt)) || (k == Le)) || (k == Ge)); return ((((k == Star) || (k == Slash)) || (k == Percent)) ? 8 : ((((k == Plus) || (k == Minus)) || (k == PlusPlus)) ? 7 : (((k == Shl) || (k == Shr)) ? 6 : ((k == DotDot) ? 5 : (is_cmp ? 4 : ((k == PipeGt) ? 3 : ((k == AmpAmp) ? 2 : ((k == PipePipe) ? 1 : 0))))))));  }
PExpr parse_expr(MacaList ts, long i) { PExpr lhs = parse_bin(ts, parse_primary(ts, i), 1); return (((*(Token*)ts.data[lhs.next]).kind == Question) ? parse_ternary(ts, lhs) : lhs);  }
PExpr parse_ternary(MacaList ts, PExpr cond) { PExpr then = parse_expr(ts, (cond.next + 1)); PExpr els = parse_expr(ts, (then.next + 1)); return mk_pexpr(e_ternary(cond.node, then.node, els.node), els.next);  }
PExpr parse_bin(MacaList ts, PExpr lhs, long min_prec) { long p = prec_of((*(Token*)ts.data[lhs.next]).kind); return ((p < min_prec) ? lhs : climb(ts, lhs, p, min_prec));  }
PExpr climb(MacaList ts, PExpr lhs, long p, long min_prec) { const char* op = (*(Token*)ts.data[lhs.next]).text; PExpr rhs = parse_bin(ts, parse_primary(ts, (lhs.next + 1)), (p + 1)); Expr node = ((strcmp(op, "|>") == 0) ? piped(lhs.node, rhs.node) : e_binary(op, lhs.node, rhs.node)); return parse_bin(ts, mk_pexpr(node, rhs.next), min_prec);  }
Expr piped(Expr lhs, Expr rhs) { return ((rhs.kind == ECall) ? e_call(rhs.text, maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ lhs })), rhs.children)) : ((rhs.kind == EMethod) ? ({ __typeof__(rhs) _w = rhs; _w.children = maca_list_cat(maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ (*(Expr*)rhs.children.data[0]) }), maca_box(sizeof(Expr), (Expr[]){ lhs })), maca_list_slice(rhs.children, 1, (rhs.children.len))); _w; }) : e_call(rhs.text, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ lhs })))));  }
PExpr parse_primary(MacaList ts, long i) { return parse_postfix(ts, parse_atom(ts, i));  }
PExpr parse_postfix(MacaList ts, PExpr e) { return ((((*(Token*)ts.data[e.next]).kind == LBracket) && (*(Token*)ts.data[e.next]).fresh) ? e : (((*(Token*)ts.data[e.next]).kind == QuestionPost) ? parse_postfix(ts, mk_pexpr(e.node, (e.next + 1))) : (((*(Token*)ts.data[e.next]).kind == Dot) ? parse_dot(ts, e) : (((*(Token*)ts.data[e.next]).kind == LBracket) ? parse_index(ts, e) : (((*(Token*)ts.data[e.next]).kind == KwWith) ? parse_with(ts, e) : e)))));  }
PExpr parse_index(MacaList ts, PExpr e) { PExpr ix = parse_expr(ts, (e.next + 1)); Expr got = e_method(e.node, "get", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ ix.node }))); return parse_postfix(ts, mk_pexpr(got, (ix.next + 1)));  }
PExpr parse_with(MacaList ts, PExpr base) { PArgs rf = parse_lit_fields(ts, (base.next + 2), maca_listv(0)); return parse_postfix(ts, mk_pexpr(e_with(base.node, rf.aitems), rf.anext));  }
PExpr parse_dot(MacaList ts, PExpr e) { const char* name = (*(Token*)ts.data[(e.next + 1)]).text; long after = (e.next + 2); return (((*(Token*)ts.data[after]).kind == LParen) ? parse_method(ts, e.node, name, after) : parse_postfix(ts, mk_pexpr(e_field(e.node, name), after)));  }
PExpr parse_method(MacaList ts, Expr recv, const char* name, long lparen) { PArgs a = parse_args(ts, (lparen + 1), maca_listv(0)); return parse_postfix(ts, mk_pexpr(e_method(recv, name, a.aitems), a.anext));  }
Expr str_node(const char* raw) { return (has_interp(maca_chars(raw), 0) ? interp_node(raw) : e_str(plain_braces(maca_chars(raw), 0, "")));  }
long has_interp(MacaList cs, long i) { return ((i >= (cs.len)) ? 0 : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? has_interp(cs, (i + 2)) : (pair_at(cs, i, "{") ? has_interp(cs, (i + 2)) : ((strcmp(((const char*)cs.data[i]), "{") == 0) ? 1 : has_interp(cs, (i + 1))))));  }
const char* plain_braces(MacaList cs, long i, const char* acc) { return ((i >= (cs.len)) ? acc : (escaped_brace(cs, i) ? plain_braces(cs, (i + 2), maca_cat(acc, ((const char*)cs.data[(i + 1)]))) : ((pair_at(cs, i, "{") || pair_at(cs, i, "}")) ? plain_braces(cs, (i + 2), maca_cat(acc, ((const char*)cs.data[i]))) : plain_braces(cs, (i + 1), maca_cat(acc, ((const char*)cs.data[i]))))));  }
long escaped_brace(MacaList cs, long i) { return (((strcmp(((const char*)cs.data[i]), "\\") != 0) || ((i + 1) >= (cs.len))) ? 0 : ((strcmp(((const char*)cs.data[(i + 1)]), "{") == 0) || (strcmp(((const char*)cs.data[(i + 1)]), "}") == 0)));  }
Expr interp_node(const char* raw) { return join_parts(split_interp(maca_chars(raw), 0, 0, "", maca_listv(0)), 0, e_str(""));  }
MacaList split_interp(MacaList cs, long i, long depth, const char* cur, MacaList acc) { return ((i >= (cs.len)) ? maca_list_cat(acc, maca_listv(1, (long)(cur))) : interp_step(cs, i, depth, cur, acc));  }
MacaList interp_step(MacaList cs, long i, long depth, const char* cur, MacaList acc) { const char* c = ((const char*)cs.data[i]); long opens = ((strcmp(c, "{") == 0) && (depth == 0)); long closes = ((strcmp(c, "}") == 0) && (depth == 1)); return (escaped_brace(cs, i) ? split_interp(cs, (i + 2), depth, maca_cat(cur, ((const char*)cs.data[(i + 1)])), acc) : (((depth == 0) && (pair_at(cs, i, "{") || pair_at(cs, i, "}"))) ? split_interp(cs, (i + 2), depth, maca_cat(cur, c), acc) : ((opens || closes) ? split_interp(cs, (i + 1), (opens ? 1 : 0), "", maca_list_cat(acc, maca_listv(1, (long)(cur)))) : (((strcmp(c, "{") == 0) && (depth > 0)) ? split_interp(cs, (i + 1), (depth + 1), maca_cat(cur, c), acc) : (((strcmp(c, "}") == 0) && (depth > 1)) ? split_interp(cs, (i + 1), (depth - 1), maca_cat(cur, c), acc) : split_interp(cs, (i + 1), depth, maca_cat(cur, c), acc))))));  }
long pair_at(MacaList cs, long i, const char* c) { return (((strcmp(((const char*)cs.data[i]), c) == 0) && ((i + 1) < (cs.len))) && (strcmp(((const char*)cs.data[(i + 1)]), c) == 0));  }
Expr join_parts(MacaList parts, long i, Expr acc) { return ((i >= (parts.len)) ? acc : join_parts(parts, (i + 1), add_part(acc, ((const char*)parts.data[i]), i)));  }
Expr add_part(Expr acc, const char* piece, long i) { return ((i == 0) ? e_str(piece) : (((i % 2) == 1) ? e_binary("++", acc, formatted(piece)) : ((strcmp(piece, "") == 0) ? acc : e_binary("++", acc, e_str(piece)))));  }
Expr parse_fragment(const char* src) { return parse_expr(lex(src), 0).node;  }
Expr formatted(const char* piece) { MacaList cs = maca_chars(piece); long at = spec_start(cs, ((cs.len) - 1)); return ((at < 0) ? e_call("str", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ parse_fragment(piece) }))) : spec_applied(parse_fragment(maca_str_slice(piece, 0, at)), maca_str_slice(piece, (at + 1), ((int)strlen(piece)))));  }
long spec_start(MacaList cs, long i) { return (((i >= 0) && is_spec_char(((const char*)cs.data[i]))) ? spec_start(cs, (i - 1)) : ((((i < 1) || (i == ((cs.len) - 1))) || (strcmp(((const char*)cs.data[i]), ":") != 0)) ? (-1) : (((strcmp(((const char*)cs.data[(i - 1)]), " ") == 0) || (strcmp(((const char*)cs.data[(i - 1)]), "\t") == 0)) ? (-1) : i)));  }
long is_spec_char(const char* c) { return ((is_align(c) || (strcmp(c, ".") == 0)) || (isdigit((unsigned char)(c)[0]) != 0));  }
long is_align(const char* c) { return (((strcmp(c, "<") == 0) || (strcmp(c, ">") == 0)) || (strcmp(c, "^") == 0));  }
Expr spec_applied(Expr e, const char* spec) { const char* rest = (is_align(maca_str_at(spec, 0)) ? maca_str_slice(spec, 1, ((int)strlen(spec))) : spec); long dot = maca_str_index_of(rest, "."); const char* width = ((dot < 0) ? rest : maca_str_slice(rest, 0, dot)); const char* prec = ((dot < 0) ? "" : maca_str_slice(rest, (dot + 1), ((int)strlen(rest)))); Expr shown = shown_part(e, prec); const char* fill = (((((int)strlen(rest)) > 1) && (strcmp(maca_str_at(rest, 0), "0") == 0)) ? "0" : " "); return ((strcmp(width, "") == 0) ? shown : e_method(shown, pad_how(maca_str_at(spec, 0)), maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ e_int(atol(width)) }), maca_box(sizeof(Expr), (Expr[]){ e_str(fill) }))));  }
Expr shown_part(Expr e, const char* prec) { return ((strcmp(prec, "") == 0) ? e_call("str", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e }))) : e_method(e, "fixed", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_int(atol(prec)) }))));  }
const char* pad_how(const char* align) { return ((strcmp(align, "<") == 0) ? "pad_end" : ((strcmp(align, "^") == 0) ? "pad_center" : "pad_start"));  }
PExpr parse_atom(MacaList ts, long i) { Token t = (*(Token*)ts.data[i]); return (t.kind == Minus ? parse_neg(ts, i) : (t.kind == Bang ? parse_not(ts, i) : (t.kind == TInt ? mk_pexpr(e_int(atol(t.text)), (i + 1)) : (t.kind == TFloat ? mk_pexpr(e_float(t.text), (i + 1)) : (t.kind == TStr ? mk_pexpr(str_node(t.text), (i + 1)) : (t.kind == KwTrue ? mk_pexpr(e_bool("true"), (i + 1)) : (t.kind == KwFalse ? mk_pexpr(e_bool("false"), (i + 1)) : (t.kind == KwMatch ? parse_match(ts, i) : (t.kind == KwIf ? parse_if(ts, i) : (t.kind == KwWhile ? parse_while(ts, i) : (t.kind == KwFor ? parse_for(ts, i) : (t.kind == KwBreak ? mk_pexpr(e_jump("break"), (i + 1)) : (t.kind == KwContinue ? mk_pexpr(e_jump("continue"), (i + 1)) : (t.kind == KwReturn ? parse_return(ts, i) : (t.kind == KwFail ? parse_fail(ts, i) : (t.kind == KwTry ? parse_try(ts, i) : (t.kind == LBracket ? parse_list(ts, i) : (t.kind == LBrace ? parse_anon_record(ts, i) : (t.kind == TIdent ? parse_call_or_ident(ts, i) : (t.kind == LParen ? parse_paren(ts, i) : mk_pexpr(e_bad((*(Token*)ts.data[i]).text), (i + 1))))))))))))))))))))));  }
PExpr parse_return(MacaList ts, long i) { return ((((*(Token*)ts.data[(i + 1)]).kind == RBrace) || ((*(Token*)ts.data[(i + 1)]).kind == Eof)) ? mk_pexpr(e_leave(maca_listv(0)), (i + 1)) : ({ PExpr left = parse_expr(ts, (i + 1)); mk_pexpr(e_leave(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ left.node }))), left.next); }));  }
PExpr parse_try(MacaList ts, long i) { PExpr guarded = parse_expr(ts, (i + 1)); return mk_pexpr(e_unary("try", guarded.node), guarded.next);  }
PExpr parse_fail(MacaList ts, long i) { PExpr raised = parse_expr(ts, (i + 1)); return mk_pexpr(e_unary("fail", raised.node), raised.next);  }
PExpr parse_list(MacaList ts, long i) { PArgs el = parse_list_elems(ts, (i + 1), maca_listv(0)); return mk_pexpr(e_list(el.aitems), el.anext);  }
PArgs parse_list_elems(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBracket)) ? mk_pargs(acc, (i + 1)) : parse_one_elem(ts, i, acc));  }
PArgs parse_one_elem(MacaList ts, long i, MacaList acc) { PExpr e = parse_expr(ts, i); return parse_list_elems(ts, skip_comma(ts, e.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e.node }))));  }
long header_brace(MacaList ts, long i, long depth) { Kind k = (*(Token*)ts.data[i]).kind; return (((k == Eof) || ((k == LBrace) && (depth == 0))) ? i : (((k == LParen) || (k == LBracket)) ? header_brace(ts, (i + 1), (depth + 1)) : (((k == RParen) || (k == RBracket)) ? header_brace(ts, (i + 1), (depth - 1)) : header_brace(ts, (i + 1), depth))));  }
Token end_tok() { return mk_token(Eof, "", 0);  }
MacaList cond_tokens(MacaList ts, long i, long b) { return maca_list_cat(maca_list_slice(ts, i, b), maca_listv(4, maca_box(sizeof(Token), (Token[]){ end_tok() }), maca_box(sizeof(Token), (Token[]){ end_tok() }), maca_box(sizeof(Token), (Token[]){ end_tok() }), maca_box(sizeof(Token), (Token[]){ end_tok() })));  }
PExpr parse_cond(MacaList ts, long i) { long b = header_brace(ts, i, 0); return mk_pexpr(parse_expr(cond_tokens(ts, i, b), 0).node, b);  }
PExpr parse_while(MacaList ts, long i) { PExpr cond = parse_cond(ts, (i + 1)); PBlock pb = parse_block(ts, (cond.next + 1), maca_listv(0)); return mk_pexpr(e_while(cond.node, pb.bstmts), pb.bnext);  }
PExpr parse_for(MacaList ts, long i) { PExpr over = parse_cond(ts, (i + 3)); PBlock pb = parse_block(ts, (over.next + 1), maca_listv(0)); return mk_pexpr(e_for((*(Token*)ts.data[(i + 1)]).text, over.node, pb.bstmts), pb.bnext);  }
PExpr parse_if(MacaList ts, long i) { PExpr cond = parse_cond(ts, (i + 1)); PExpr then = parse_branch(ts, cond.next); PExpr els = parse_else(ts, then.next); return mk_pexpr(e_if(cond.node, then.node, els.node), els.next);  }
PExpr parse_branch(MacaList ts, long brace) { PBlock pb = parse_block(ts, (brace + 1), maca_listv(0)); return mk_pexpr(body_expr(pb.bstmts), pb.bnext);  }
PExpr parse_else(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind != KwElse) ? mk_pexpr(e_ident("?"), i) : (((*(Token*)ts.data[(i + 1)]).kind == KwIf) ? parse_if(ts, (i + 1)) : parse_branch(ts, (i + 1))));  }
PExpr parse_match(MacaList ts, long i) { PExpr scrut = parse_expr(ts, (i + 1)); PArgs arms = parse_arms(ts, (scrut.next + 1), maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ scrut.node }))); return mk_pexpr(e_match(arms.aitems), arms.anext);  }
PArgs parse_arms(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? mk_pargs(acc, (i + 1)) : parse_one_arm(ts, i, acc));  }
PArgs parse_one_arm(MacaList ts, long i, MacaList acc) { PExpr p = parse_guarded(ts, parse_alts(ts, parse_commas(ts, parse_pattern(ts, i)))); PExpr body = parse_arm_body(ts, (p.next + 1)); return parse_arms(ts, body.next, maca_list_cat(acc, maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ p.node }), maca_box(sizeof(Expr), (Expr[]){ body.node }))));  }
PExpr parse_arm_body(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == LBrace) ? parse_branch(ts, i) : parse_expr(ts, i));  }
PExpr parse_commas(MacaList ts, PExpr p) { return (((*(Token*)ts.data[p.next]).kind != Comma) ? p : gathered(ts, mk_pexpr(with_child(e_ident("[]"), p.node), p.next)));  }
PExpr gathered(MacaList ts, PExpr p) { return (((*(Token*)ts.data[p.next]).kind != Comma) ? p : (((*(Token*)ts.data[(p.next + 1)]).kind == DotDot) ? mk_pexpr(with_child(({ __typeof__(p.node) _w = p.node; _w.text = "[..]"; _w; }), e_ident((*(Token*)ts.data[(p.next + 2)]).text)), (p.next + 3)) : ({ PExpr next = parse_pattern(ts, (p.next + 1)); gathered(ts, mk_pexpr(with_child(p.node, next.node), next.next)); })));  }
PExpr parse_alts(MacaList ts, PExpr p) { return (((*(Token*)ts.data[p.next]).kind != Bar) ? p : ({ PExpr next = parse_pattern(ts, (p.next + 1)); parse_alts(ts, mk_pexpr(e_binary("|", p.node, next.node), next.next)); }));  }
PExpr parse_guarded(MacaList ts, PExpr p) { return (((*(Token*)ts.data[p.next]).kind != KwIf) ? p : ({ PExpr when = parse_expr(ts, (p.next + 1)); mk_pexpr(e_guard(p.node, when.node), when.next); }));  }
PExpr parse_pattern(MacaList ts, long i) { Expr named = (((*(Token*)ts.data[i]).kind == TStr) ? str_node((*(Token*)ts.data[i]).text) : e_ident((*(Token*)ts.data[i]).text)); return (((*(Token*)ts.data[i]).kind == LBrace) ? parse_fields_pattern(ts, (i + 1), e_ident("{}")) : (((*(Token*)ts.data[i]).kind == LBracket) ? parse_cells_pattern(ts, (i + 1), e_ident("[]")) : (((*(Token*)ts.data[(i + 1)]).kind == LParen) ? parse_binders(ts, (i + 2), named) : mk_pexpr(named, (i + 1)))));  }
PExpr parse_cells_pattern(MacaList ts, long i, Expr p) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBracket)) ? mk_pexpr(p, (i + 1)) : (((*(Token*)ts.data[i]).kind == Comma) ? parse_cells_pattern(ts, (i + 1), p) : ((((*(Token*)ts.data[i]).kind == DotDot) || ((*(Token*)ts.data[i]).kind == Ellipsis)) ? parse_cells_pattern(ts, (i + 2), with_child(({ __typeof__(p) _w = p; _w.text = "[..]"; _w; }), e_ident((*(Token*)ts.data[(i + 1)]).text))) : parse_cells_pattern(ts, (i + 1), with_child(p, e_ident((*(Token*)ts.data[i]).text))))));  }
PExpr parse_fields_pattern(MacaList ts, long i, Expr p) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? mk_pexpr(p, (i + 1)) : (((*(Token*)ts.data[i]).kind == Comma) ? parse_fields_pattern(ts, (i + 1), p) : parse_fields_pattern(ts, (i + 1), with_child(p, e_ident((*(Token*)ts.data[i]).text)))));  }
PExpr parse_binders(MacaList ts, long i, Expr p) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RParen)) ? mk_pexpr(p, (i + 1)) : (((*(Token*)ts.data[i]).kind == Comma) ? parse_binders(ts, (i + 1), p) : parse_binders(ts, (i + 1), with_child(p, e_ident((*(Token*)ts.data[i]).text)))));  }
PExpr parse_neg(MacaList ts, long i) { PExpr operand = parse_primary(ts, (i + 1)); return mk_pexpr(e_unary("-", operand.node), operand.next);  }
PExpr parse_not(MacaList ts, long i) { PExpr operand = parse_primary(ts, (i + 1)); return mk_pexpr(e_unary("!", operand.node), operand.next);  }
PExpr parse_task(MacaList ts, long i, const char* word) { PExpr operand = parse_primary(ts, (i + 1)); return mk_pexpr(e_unary(word, operand.node), operand.next);  }
long paren_end(MacaList ts, long i, long depth) { Kind k = (*(Token*)ts.data[i]).kind; return (((k == Eof) || ((k == RParen) && (depth == 0))) ? i : ((((k == LParen) || (k == LBracket)) || (k == LBrace)) ? paren_end(ts, (i + 1), (depth + 1)) : ((((k == RParen) || (k == RBracket)) || (k == RBrace)) ? paren_end(ts, (i + 1), (depth - 1)) : paren_end(ts, (i + 1), depth))));  }
PExpr parse_paren(MacaList ts, long i) { long shut = paren_end(ts, (i + 1), 0); return (((*(Token*)ts.data[after_return_type(ts, (shut + 1))]).kind == FatArrow) ? parse_lambda(ts, i) : mk_pexpr(parse_expr(ts, (i + 1)).node, (shut + 1)));  }
PExpr parse_lambda(MacaList ts, long i) { PParams pp = parse_params(ts, (i + 1), maca_listv(0)); PExpr body = parse_lambda_body(ts, (after_return_type(ts, (pp.pnext + 1)) + 1)); Expr made = e_lambda(pp.params, body.node); return mk_pexpr(({ __typeof__(made) _w = made; _w.ty = return_type(ts, (pp.pnext + 1)); _w; }), body.next);  }
PExpr parse_lambda_body(MacaList ts, long at) { return (((*(Token*)ts.data[at]).kind == LBrace) ? parse_branch(ts, at) : lambda_setter(ts, parse_expr(ts, at)));  }
PExpr lambda_setter(MacaList ts, PExpr lhs) { return (((*(Token*)ts.data[lhs.next]).kind != Eq) ? lhs : ({ PExpr v = parse_lambda_body(ts, (lhs.next + 1)); mk_pexpr(e_binary("=", lhs.node, v.node), v.next); }));  }
PExpr parse_call_or_ident(MacaList ts, long i) { const char* name = (*(Token*)ts.data[i]).text; return (((strcmp(name, "spawn") == 0) || (strcmp(name, "await") == 0)) ? parse_task(ts, i, name) : (((*(Token*)ts.data[(i + 1)]).kind == FatArrow) ? parse_one_lambda(ts, i, name) : (((*(Token*)ts.data[(i + 1)]).kind == LParen) ? parse_call(ts, i, name) : (opens_record_lit(ts, i) ? parse_record_lit(ts, i, name) : mk_pexpr(e_ident(name), (i + 1))))));  }
PExpr parse_one_lambda(MacaList ts, long i, const char* name) { PExpr body = parse_lambda_body(ts, (i + 2)); return mk_pexpr(e_lambda(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_param(name, "") })), body.node), body.next);  }
long opens_record_lit(MacaList ts, long i) { long opens = ((*(Token*)ts.data[(i + 1)]).kind == LBrace); long named = ((*(Token*)ts.data[(i + 2)]).kind == TIdent); Kind after = (*(Token*)ts.data[(i + 3)]).kind; return (((opens && named) && ((after == Eq) || (after == Comma))) && (!block_head(ts, i)));  }
long block_head(MacaList ts, long i) { Kind kind = (*(Token*)ts.data[i]).kind; return ((((((kind == KwIf) || (kind == KwElse)) || (kind == KwWhile)) || (kind == KwFor)) || (kind == KwMatch)) ? 1 : (((((i == 0) || (*(Token*)ts.data[i]).fresh) || (kind == LBrace)) || (kind == RBrace)) ? 0 : block_head(ts, (i - 1))));  }
PExpr parse_record_lit(MacaList ts, long i, const char* name) { PArgs rf = parse_lit_fields(ts, (i + 2), maca_listv(0)); return mk_pexpr(e_record(name, rf.aitems), rf.anext);  }
PExpr parse_anon_record(MacaList ts, long i) { PArgs rf = parse_lit_fields(ts, (i + 1), maca_listv(0)); return mk_pexpr(e_record("", rf.aitems), rf.anext);  }
PArgs parse_lit_fields(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RBrace)) ? mk_pargs(acc, (i + 1)) : parse_one_lit_field(ts, i, acc));  }
PArgs parse_one_lit_field(MacaList ts, long i, MacaList acc) { const char* fname = (*(Token*)ts.data[i]).text; return (((*(Token*)ts.data[(i + 1)]).kind != Eq) ? parse_lit_fields(ts, skip_comma(ts, (i + 1)), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_binary("=", e_ident(fname), e_ident(fname)) })))) : ({ PExpr v = lit_value(ts, (i + 2)); parse_lit_fields(ts, skip_comma(ts, v.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_binary("=", e_ident(fname), v.node) })))); }));  }
PExpr lit_value(MacaList ts, long i) { PExpr v = parse_expr(ts, i); return ((((*(Token*)ts.data[v.next]).kind != Comma) || ends_lit_value(ts, (v.next + 1))) ? v : more_lit_elems(ts, (v.next + 1), maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ v.node }))));  }
PExpr more_lit_elems(MacaList ts, long i, MacaList acc) { PExpr e = parse_expr(ts, i); MacaList seen = maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e.node }))); return ((((*(Token*)ts.data[e.next]).kind == Comma) && (!ends_lit_value(ts, (e.next + 1)))) ? more_lit_elems(ts, (e.next + 1), seen) : mk_pexpr(e_list(seen), e.next));  }
long ends_lit_value(MacaList ts, long i) { Kind k = (*(Token*)ts.data[i]).kind; Kind after = (*(Token*)ts.data[(i + 1)]).kind; return (((k == RBrace) || (k == Eof)) || ((k == TIdent) && (((after == Eq) || (after == Comma)) || (after == RBrace))));  }
PExpr parse_call(MacaList ts, long i, const char* name) { PArgs a = parse_args(ts, (i + 2), maca_listv(0)); return mk_pexpr(e_call(name, a.aitems), a.anext);  }
PArgs parse_args(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == Eof) || ((*(Token*)ts.data[i]).kind == RParen)) ? mk_pargs(acc, (i + 1)) : parse_one_arg(ts, i, acc));  }
PArgs parse_one_arg(MacaList ts, long i, MacaList acc) { long named = attr_name_end(ts, i); return ((named > i) ? ({ PExpr v = parse_expr(ts, (named + 1)); parse_args(ts, skip_comma(ts, v.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_attr(attr_name(ts, i, named, ""), v.node) })))); }) : ({ PExpr a = parse_expr(ts, i); parse_args(ts, skip_comma(ts, a.next), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ a.node })))); }));  }
long attr_name_end(MacaList ts, long i) { long end = name_run_end(ts, i); return (((*(Token*)ts.data[end]).kind == Eq) ? end : i);  }
long name_run_end(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == Eof) ? i : ((((*(Token*)ts.data[(i + 1)]).kind == Minus) && ((*(Token*)ts.data[(i + 2)]).kind == TIdent)) ? name_run_end(ts, (i + 2)) : (i + 1)));  }
const char* attr_name(MacaList ts, long i, long end, const char* acc) { return ((i >= end) ? acc : attr_name(ts, (i + 1), end, maca_cat(acc, (*(Token*)ts.data[i]).text)));  }
Ty bare(TyKind k) { return (Ty){ .kind = k, .name = "", .slot = 0, .args = maca_listv(0), .labels = maca_listv(0), .open_mc = 0 };  }
Ty t_int() { return bare(KInt);  }
Ty t_float() { return bare(KFloat);  }
Ty t_str() { return bare(KStr);  }
Ty t_bool() { return bare(KBool);  }
Ty t_bytes() { return bare(KBytes);  }
Ty t_unit() { return bare(KUnit);  }
Ty t_any() { return bare(KAny);  }
Ty t_error() { return bare(KError);  }
long absorbing(Ty t) { return ((t.kind == KAny) || (t.kind == KError));  }
Ty t_var(long slot) { return ({ __typeof__(bare(KVar)) _w = bare(KVar); _w.slot = slot; _w; });  }
Ty t_con(const char* name, MacaList args) { return ({ __typeof__(bare(KCon)) _w = bare(KCon); _w.name = name; _w.args = args; _w; });  }
Ty t_array(Ty el) { return t_con("Array", maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ el })));  }
Ty t_fn(MacaList params, Ty ret) { return ({ __typeof__(bare(KFn)) _w = bare(KFn); _w.args = maca_list_cat(params, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ ret }))); _w; });  }
Ty t_rec(MacaList labels, MacaList types, long open_mc) { return ({ __typeof__(bare(KRec)) _w = bare(KRec); _w.labels = labels; _w.args = types; _w.open_mc = open_mc; _w; });  }
Ty t_opt(Ty inner) { return ({ __typeof__(bare(KOpt)) _w = bare(KOpt); _w.args = maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ inner })); _w; });  }
MacaList fn_params(Ty t) { return maca_list_slice(t.args, 0, ((t.args.len) - 1));  }
Ty fn_ret(Ty t) { return (*(Ty*)t.args.data[((t.args.len) - 1)]);  }
Infer new_infer() { return (Infer){ .subst = maca_listv(0) };  }
Instance fresh(Infer inf) { Ty made = t_var((inf.subst.len)); return (Instance){ .infer = (Infer){ .subst = maca_list_cat(inf.subst, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ made }))) }, .ty = made };  }
long unbound(Infer inf, long slot) { Ty held = (*(Ty*)inf.subst.data[slot]); return ((held.kind == KVar) && (held.slot == slot));  }
Ty resolve(Infer inf, Ty t) { return ((t.kind != KVar) ? t : (unbound(inf, t.slot) ? t : resolve(inf, (*(Ty*)inf.subst.data[t.slot]))));  }
Infer set_slot(Infer inf, long slot, Ty t) { return (Infer){ .subst = maca_list_cat(maca_list_cat(maca_list_slice(inf.subst, 0, slot), maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ t }))), maca_list_slice(inf.subst, (slot + 1), (inf.subst.len))) };  }
long occurs(Infer inf, long slot, Ty t) { Ty seen = resolve(inf, t); return ((seen.kind == KVar) ? (seen.slot == slot) : occurs_in(inf, slot, seen.args, 0));  }
long occurs_in(Infer inf, long slot, MacaList ts, long i) { return ((i >= (ts.len)) ? 0 : (occurs(inf, slot, (*(Ty*)ts.data[i])) ? 1 : occurs_in(inf, slot, ts, (i + 1))));  }
Infer bind_var(Infer inf, long slot, Ty t) { return (occurs(inf, slot, t) ? set_slot(inf, slot, t_any()) : set_slot(inf, slot, t));  }
Unify united(Infer inf) { return (Unify){ .infer = inf, .error = "" };  }
Unify clashed(Infer inf, Ty x, Ty y) { return (Unify){ .infer = inf, .error = maca_cat_own(maca_cat_own(maca_cat("type mismatch: expected ", show_ty(x)), ", found ", 1), show_ty(y), 1) };  }
Unify refused(Infer inf, const char* why) { return (Unify){ .infer = inf, .error = why };  }
long shape_disagrees(Ty x, Ty y) { return ((x.kind != y.kind) ? 1 : ((x.kind == KCon) ? ((strcmp(x.name, y.name) != 0) || ((x.args.len) != (y.args.len))) : ((x.kind == KFn) ? ((x.args.len) != (y.args.len)) : 0)));  }
Unify unify(Infer inf, Ty a, Ty b) { Ty x = resolve(inf, a); Ty y = resolve(inf, b); return ((absorbing(x) || absorbing(y)) ? united(inf) : ((((x.kind == KVar) && (y.kind == KVar)) && (x.slot == y.slot)) ? united(inf) : ((x.kind == KVar) ? united(bind_var(inf, x.slot, y)) : ((y.kind == KVar) ? united(bind_var(inf, y.slot, x)) : (((x.kind == KOpt) && (y.kind != KOpt)) ? unify(inf, (*(Ty*)x.args.data[0]), y) : (((y.kind == KOpt) && (x.kind != KOpt)) ? unify(inf, x, (*(Ty*)y.args.data[0])) : (((x.kind == KRec) && (y.kind == KRec)) ? unify_rows(inf, x, y) : (shape_disagrees(x, y) ? clashed(inf, x, y) : unify_all(inf, x.args, y.args, 0)))))))));  }
Unify unify_all(Infer inf, MacaList xs, MacaList ys, long i) { return ((i >= (xs.len)) ? united(inf) : ({ Unify step = unify(inf, (*(Ty*)xs.data[i]), (*(Ty*)ys.data[i])); ((strcmp(step.error, "") != 0) ? step : unify_all(step.infer, xs, ys, (i + 1))); }));  }
Unify unify_rows(Infer inf, Ty x, Ty y) { Unify shared = unify_shared(inf, x, y, 0); const char* extra = unexpected_label(x, y, 0); return ((strcmp(shared.error, "") != 0) ? shared : ((strcmp(extra, "") != 0) ? refused(shared.infer, maca_cat_own(maca_cat("record has unexpected field `", extra), "`", 1)) : united(shared.infer)));  }
Unify unify_shared(Infer inf, Ty x, Ty y, long i) { return ((i >= (x.labels.len)) ? united(inf) : ({ const char* label = ((const char*)x.labels.data[i]); long at = maca_list_index_of_str(y.labels, label); (((at < 0) && y.open_mc) ? unify_shared(inf, x, y, (i + 1)) : ((at < 0) ? refused(inf, maca_cat_own(maca_cat("record is missing field `", label), "`", 1)) : ({ Unify step = unify(inf, (*(Ty*)x.args.data[i]), (*(Ty*)y.args.data[at])); ((strcmp(step.error, "") != 0) ? step : unify_shared(step.infer, x, y, (i + 1))); }))); }));  }
const char* unexpected_label(Ty x, Ty y, long i) { return ((i >= (y.labels.len)) ? "" : (((maca_list_index_of_str(x.labels, ((const char*)y.labels.data[i])) < 0) && (!x.open_mc)) ? ((const char*)y.labels.data[i]) : unexpected_label(x, y, (i + 1))));  }
Scheme mono(Ty t) { return (Scheme){ .slots = maca_listv(0), .ty = t };  }
Scheme generalize(Infer inf, Ty t) { return (Scheme){ .slots = free_slots(inf, t, maca_listv(0)), .ty = t };  }
MacaList free_slots(Infer inf, Ty t, MacaList seen) { Ty known = resolve(inf, t); return (((known.kind == KVar) && (maca_list_index_of(seen, (long)(known.slot)) < 0)) ? maca_list_cat(seen, maca_listv(1, (long)(known.slot))) : ((known.kind == KVar) ? seen : free_slots_in(inf, known.args, 0, seen)));  }
MacaList free_slots_in(Infer inf, MacaList ts, long i, MacaList seen) { return ((i >= (ts.len)) ? seen : free_slots_in(inf, ts, (i + 1), free_slots(inf, (*(Ty*)ts.data[i]), seen)));  }
Batch fresh_many(Infer inf, long n, MacaList acc) { return ((n <= 0) ? (Batch){ .infer = inf, .tys = acc } : ({ Instance made = fresh(inf); fresh_many(made.infer, (n - 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ made.ty })))); }));  }
Instance instantiate(Infer inf, Scheme s) { Batch made = fresh_many(inf, (s.slots.len), maca_listv(0)); return (Instance){ .infer = made.infer, .ty = substitute(s.slots, made.tys, s.ty) };  }
Ty substitute(MacaList slots, MacaList tos, Ty t) { return ((t.kind == KVar) ? ({ long at = maca_list_index_of(slots, (long)(t.slot)); ((at < 0) ? t : (*(Ty*)tos.data[at])); }) : ({ __typeof__(t) _w = t; _w.args = substitute_all(slots, tos, t.args, 0); _w; }));  }
MacaList substitute_all(MacaList slots, MacaList tos, MacaList ts, long i) { return ((i >= (ts.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ substitute(slots, tos, (*(Ty*)ts.data[i])) })), substitute_all(slots, tos, ts, (i + 1))));  }
const char* show_ty(Ty t) { return (t.kind == KInt ? "int" : (t.kind == KFloat ? "float" : (t.kind == KStr ? "str" : (t.kind == KBool ? "bool" : (t.kind == KBytes ? "bytes" : (t.kind == KUnit ? "()" : (t.kind == KAny ? "any" : (t.kind == KError ? "error" : (t.kind == KVar ? maca_cat_own("t", maca_int_to_str(t.slot), 2) : (t.kind == KCon ? show_con(t) : (t.kind == KFn ? show_fn(t) : (t.kind == KRec ? show_rec(t) : maca_cat_own(maca_cat("", show_ty((*(Ty*)t.args.data[0]))), "?", 1)))))))))))));  }
const char* show_con(Ty t) { return (((t.args.len) == 0) ? t.name : maca_cat_own(maca_cat_own(maca_cat("", t.name), " ", 1), show_joined(t.args, " ", 0), 1));  }
const char* show_fn(Ty t) { return maca_cat_own(maca_cat_own(maca_cat("(", show_joined(fn_params(t), ", ", 0)), ") -> ", 1), show_ty(fn_ret(t)), 1);  }
const char* show_rec(Ty t) { return maca_cat_own(maca_cat("{ ", show_fields(t, 0)), " }", 1);  }
const char* show_fields(Ty t, long i) { return ((i >= (t.labels.len)) ? "" : ((i == ((t.labels.len) - 1)) ? maca_cat_own(maca_cat_own(maca_cat("", ((const char*)t.labels.data[i])), ": ", 1), show_ty((*(Ty*)t.args.data[i])), 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ((const char*)t.labels.data[i])), ": ", 1), show_ty((*(Ty*)t.args.data[i])), 1), ", ", 1), show_fields(t, (i + 1)), 1)));  }
const char* show_joined(MacaList ts, const char* sep, long i) { return ((i >= (ts.len)) ? "" : ((i == ((ts.len) - 1)) ? show_ty((*(Ty*)ts.data[i])) : maca_cat_own(maca_cat(show_ty((*(Ty*)ts.data[i])), sep), show_joined(ts, sep, (i + 1)), 1)));  }
Env empty_env() { return (Env){ .names = maca_listv(0), .types = maca_listv(0), .fns = maca_listv(0), .sigs = maca_listv(0), .fields = maca_listv(0), .ftypes = maca_listv(0), .ctors = maca_listv(0), .owners = maca_listv(0), .slots = maca_listv(0), .varargs = maca_listv(0), .frozen = maca_listv(0), .infer = new_infer(), .errors = maca_listv(0), .holes = maca_listv(0), .fills = maca_listv(0), .here = 0 };  }
const char* diag_explain(const char* code) { return ((strcmp(code, "M0001") == 0) ? "a value is used where a different type is required" : ((strcmp(code, "M0002") == 0) ? "a `match` leaves a variant unhandled" : ((strcmp(code, "M0003") == 0) ? "config mode has no effects, and this call performs one" : ((strcmp(code, "M0004") == 0) ? "the option this sets does not exist" : ((strcmp(code, "M0005") == 0) ? "a constant is assigned after it is bound" : ((strcmp(code, "M0006") == 0) ? "this name is not defined anywhere in scope" : ((strcmp(code, "M0007") == 0) ? "the target being built for cannot carry this effect" : ((strcmp(code, "M0008") == 0) ? "a file the program embeds is not beside it" : ((strcmp(code, "M0009") == 0) ? "a method keeps a value the crate lent it for the call" : "")))))))));  }
const char* diag_message(Diagnostic d) { return d.message;  }
Diagnostic diag_at(Stmt s, const char* code, const char* why) { return (Diagnostic){ .code = code, .message = why, .note = "", .pos = s.pos };  }
Typed typed(Env env, Ty ty) { return (Typed){ .env = env, .ty = ty };  }
Env bind_mc(Env env, const char* name, Ty ty) { return ({ __typeof__(env) _w = env; _w.names = maca_list_cat(env.names, maca_listv(1, (long)(name))); _w.types = maca_list_cat(env.types, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ ty }))); _w; });  }
Env complain(Env env, const char* why) { return complain_as(env, "M0001", why);  }
Env complain_as(Env env, const char* code, const char* why) { Diagnostic said = (Diagnostic){ .code = code, .message = why, .note = "", .pos = env.here }; return ({ __typeof__(env) _w = env; _w.errors = maca_list_cat(env.errors, maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ said }))); _w; });  }
Env note(Env env, const char* why) { return ((strcmp(why, "") == 0) ? env : complain(env, why));  }
Env with_infer(Env env, Infer inf) { return ({ __typeof__(env) _w = env; _w.infer = inf; _w; });  }
Env joined(Env env, Ty want, Ty got) { Unify step = unify(env.infer, want, got); return note(with_infer(env, step.infer), step.error);  }
long is_array_name(const char* name) { return ((((int)strlen(name)) > 2) && (strcmp(maca_str_slice(name, (((int)strlen(name)) - 2), ((int)strlen(name))), "[]") == 0));  }
Ty ty_named(const char* name) { return ((strcmp(name, "") == 0) ? t_any() : ((maca_str_index_of(name, ") -> ") >= 0) ? ty_fn_named(name) : (is_array_name(name) ? t_array(ty_named(maca_str_slice(name, 0, (((int)strlen(name)) - 2)))) : ((strcmp(name, "int") == 0) ? t_int() : ((strcmp(name, "float") == 0) ? t_float() : (((strcmp(name, "str") == 0) || (strcmp(name, "Element") == 0)) ? t_str() : ((strcmp(name, "bool") == 0) ? t_bool() : ((strcmp(name, "bytes") == 0) ? t_bytes() : (is_type_var(name) ? t_any() : t_con(name, maca_listv(0)))))))))));  }
Ty ty_fn_named(const char* name) { long cut = maca_str_index_of(name, ") -> "); return t_fn(fn_arg_tys(maca_str_slice(name, 1, cut), maca_listv(0)), ty_named(maca_str_slice(name, (cut + 5), ((int)strlen(name)))));  }
MacaList fn_arg_tys(const char* list, MacaList acc) { long cut = maca_str_index_of(list, ", "); return ((strcmp(list, "") == 0) ? acc : ((cut < 0) ? maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ ty_named(list) }))) : fn_arg_tys(maca_str_slice(list, (cut + 2), ((int)strlen(list))), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ ty_named(maca_str_slice(list, 0, cut)) }))))));  }
long is_type_var(const char* name) { return (((strcmp(name, "unit") == 0) || (maca_str_index_of(name, "__") >= 0)) ? 0 : ((!starts_lower(name)) ? 0 : (!sized_number(name))));  }
long starts_lower(const char* name) { return (((((int)strlen(name)) > 0) && (isalpha((unsigned char)(maca_str_at(name, 0))[0]) != 0)) && (strcmp(maca_upper(maca_str_at(name, 0)), maca_str_at(name, 0)) != 0));  }
long sized_number(const char* name) { return (((((int)strlen(name)) > 1) && (isdigit((unsigned char)(maca_str_at(name, 1))[0]) != 0)) && (((strcmp(maca_str_at(name, 0), "i") == 0) || (strcmp(maca_str_at(name, 0), "u") == 0)) || (strcmp(maca_str_at(name, 0), "f") == 0)));  }
Typed declared_type(Env env, const char* decl) { return ((strcmp(decl, "") != 0) ? typed(env, ty_named(decl)) : ({ Instance made = fresh(env.infer); typed(with_infer(env, made.infer), made.ty); }));  }
Signature param_types(Env env, MacaList ps, long i, MacaList acc) { return ((i >= (ps.len)) ? (Signature){ .env = env, .tys = acc } : ({ Typed got = declared_type(env, (*(Expr*)ps.data[i]).ty); param_types(got.env, ps, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ got.ty })))); }));  }
Env env_of_module(Module m) { return collect_items(empty_env(), m.items, 0);  }
Env collect_items(Env env, MacaList items, long i) { return ((i >= (items.len)) ? env : collect_items(collect_item(env, (*(Stmt*)items.data[i])), items, (i + 1)));  }
Env collect_item(Env env, Stmt s) { return ((s.kind == SFn) ? add_fn(env, s) : ((s.kind == SRecord) ? add_fields(env, s.name, s.params, 0) : ((s.kind == SSum) ? add_ctors(env, s.name, s.params, 0) : ((s.kind == SBind) ? add_const(env, s) : env))));  }
Env add_const(Env env, Stmt s) { Typed got = type_in(env, s.value); Ty held = ((strcmp(s.ret, "") != 0) ? ty_named(s.ret) : got.ty); return ({ __typeof__(got.env) _w = got.env; _w.names = maca_list_cat(got.env.names, maca_listv(1, (long)(s.name))); _w.types = maca_list_cat(got.env.types, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ held }))); _w; });  }
Env add_fn(Env env, Stmt s) { Signature made = param_types(env, s.params, 0, maca_listv(0)); Typed back = declared_type(made.env, s.ret); return ({ __typeof__(back.env) _w = back.env; _w.fns = maca_list_cat(back.env.fns, maca_listv(1, (long)(s.name))); _w.sigs = maca_list_cat(back.env.sigs, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ t_fn(made.tys, back.ty) }))); _w.varargs = maca_list_cat(back.env.varargs, rest_taker(s)); _w; });  }
long rest_before_the_end(MacaList ps, long i) { return ((i >= ((ps.len) - 1)) ? 0 : (((*(Expr*)ps.data[i]).ival == 1) ? 1 : rest_before_the_end(ps, (i + 1))));  }
MacaList rest_taker(Stmt s) { long n = (s.params.len); return (((n > 0) && ((*(Expr*)s.params.data[(n - 1)]).ival == 1)) ? maca_listv(1, (long)(s.name)) : maca_listv(0));  }
Env add_fields(Env env, const char* rec, MacaList fs, long i) { return ((i >= (fs.len)) ? env : ({ Env held = ({ __typeof__(env) _w = env; _w.fields = maca_list_cat(env.fields, maca_listv(1, (long)(maca_cat_own(maca_cat(rec, "."), (*(Expr*)fs.data[i]).text, 1)))); _w.ftypes = maca_list_cat(env.ftypes, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ ty_named((*(Expr*)fs.data[i]).ty) }))); _w; }); add_fields(held, rec, fs, (i + 1)); }));  }
Env add_ctors(Env env, const char* sum, MacaList vs, long i) { return ((i >= (vs.len)) ? env : ({ Env held = ({ __typeof__(env) _w = env; _w.ctors = maca_list_cat(env.ctors, maca_listv(1, (long)((*(Expr*)vs.data[i]).text))); _w.owners = maca_list_cat(env.owners, maca_listv(1, (long)(sum))); _w.slots = maca_list_cat(env.slots, maca_listv(1, (long)(slot_types((*(Expr*)vs.data[i]))))); _w; }); add_ctors(held, sum, vs, (i + 1)); }));  }
Typed type_in(Env env, Expr e) { return (e.kind == EInt ? typed(env, t_int()) : (e.kind == EFloat ? typed(env, t_float()) : (e.kind == EStr ? typed(env, t_str()) : (e.kind == EBool ? typed(env, t_bool()) : (e.kind == EIdent ? ident_type(env, e) : (e.kind == ECall ? call_type(env, e) : (e.kind == EBinary ? binop_type(env, e) : (e.kind == ETernary ? ternary_type(env, e) : (e.kind == EIf ? ternary_type(env, e) : (e.kind == EUnary ? unary_type(env, e) : (e.kind == ERecord ? typed(check_literal(check_fields(env, e.text, e.children, 0), e.text, e.children), t_con(e.text, maca_listv(0))) : (e.kind == EWith ? with_type(env, e) : (e.kind == EField ? field_type(env, e) : (e.kind == EMethod ? method_type(env, e) : (e.kind == EBlock ? block_type(env, e) : (e.kind == EBad ? typed(complain(env, maca_cat_own("no expression starts at ", maca_cat_own(maca_cat("`", e.text), "`", 1), 2)), t_error()) : (e.kind == EAttr ? type_in(env, (*(Expr*)e.children.data[0])) : (e.kind == EGuard ? typed(walk_args(env, e.children, 0), t_bool()) : (e.kind == EMatch ? match_type(env, e) : (e.kind == EList ? list_type(env, e) : (e.kind == EWhile ? while_type(env, e) : (e.kind == EFor ? for_type(env, e) : (e.kind == ELambda ? lambda_type(env, e) : typed(env, t_any()))))))))))))))))))))))));  }
Typed lambda_type(Env env, Expr e) { MacaList ps = lambda_params(e); Signature made = param_types(env, ps, 0, maca_listv(0)); Env inner = bind_params(made.env, ps, made.tys, 0); Typed body = type_in(inner, lambda_body(e)); return typed(({ __typeof__(body.env) _w = body.env; _w.names = env.names; _w.types = env.types; _w; }), t_fn(made.tys, body.ty));  }
Typed while_type(Env env, Expr e) { Typed cond = type_in(env, (*(Expr*)e.children.data[0])); Env ready = joined(cond.env, t_bool(), cond.ty); Env inner = check_stmts(ready, e.stmts, 0); return typed(({ __typeof__(inner) _w = inner; _w.names = env.names; _w.types = env.types; _w; }), t_unit());  }
Typed for_type(Env env, Expr e) { Typed over = type_in(env, (*(Expr*)e.children.data[0])); Ty el = element_of(resolve(over.env.infer, over.ty)); Env inner = check_stmts(bind_mc(over.env, e.text, el), e.stmts, 0); return typed(({ __typeof__(inner) _w = inner; _w.names = env.names; _w.types = env.types; _w; }), t_unit());  }
Env check_stmts(Env env, MacaList stmts, long i) { return ((i >= (stmts.len)) ? env : ({ Stmt st = (*(Stmt*)stmts.data[i]); Typed got = stmt_type(env, st, 0); check_stmts(extend(got.env, st, got.ty), stmts, (i + 1)); }));  }
Typed range_type(Env env, Ty a, Ty b) { Env lo = joined(env, t_int(), a); return typed(joined(lo, t_int(), b), t_array(t_int()));  }
Typed shift_type(Env env, Ty a, Ty b) { Env value = joined(env, t_int(), a); return typed(joined(value, t_int(), b), t_int());  }
Typed unary_type(Env env, Expr e) { Typed inner = type_in(env, (*(Expr*)e.children.data[0])); return ((strcmp(e.text, "fail") == 0) ? typed(inner.env, t_any()) : ((strcmp(e.text, "try") == 0) ? typed(inner.env, t_str()) : ((strcmp(e.text, "spawn") == 0) ? typed(inner.env, t_con("Future", maca_listv(0))) : ((strcmp(e.text, "await") == 0) ? typed(inner.env, t_int()) : inner))));  }
Typed ident_type(Env env, Expr e) { long at = maca_list_index_of_str(env.names, e.text); long variant = maca_list_index_of_str(env.ctors, e.text); long declared = maca_list_index_of_str(env.fns, e.text); return ((at >= 0) ? typed(env, (*(Ty*)env.types.data[at])) : ((variant >= 0) ? typed(env, t_con(((const char*)env.owners.data[variant]), maca_listv(0))) : (((declared >= 0) && (maca_list_index_of_str(env.varargs, e.text) >= 0)) ? typed(complain(env, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", e.text), "` is variadic, so it cannot be used as a", 1), " function value; call it, or declare it", 1), maca_cat_own(maca_cat(" `", e.text), "(xs: T[])` and pass a list", 1), 3)), t_error()) : ((declared >= 0) ? typed(env, (*(Ty*)env.sigs.data[declared])) : typed(env, t_any())))));  }
Typed call_type(Env env, Expr e) { long declared = maca_list_index_of_str(env.fns, e.text); long variant = maca_list_index_of_str(env.ctors, e.text); long local = maca_list_index_of_str(env.names, e.text); return (((strcmp(e.text, "maca_cap") == 0) && ((e.children.len) == 2)) ? typed(env, ty_named((*(Expr*)e.children.data[0]).text)) : (((strcmp(e.text, "maca_closure") == 0) && ((e.children.len) > 0)) ? typed(walk_args(env, e.children, 0), closure_sig(env, e)) : (((declared >= 0) && tag_wins(env, e, declared)) ? typed(walk_args(env, e.children, 0), builtin_type(e.text)) : (((local >= 0) && callable(env, (*(Ty*)env.types.data[local]))) ? call_local(env, e, (*(Ty*)env.types.data[local])) : ((declared >= 0) ? call_declared(env, e, (*(Ty*)env.sigs.data[declared])) : ((variant >= 0) ? typed(walk_args(env, e.children, 0), t_con(((const char*)env.owners.data[variant]), maca_listv(0))) : ((local >= 0) ? call_local(env, e, (*(Ty*)env.types.data[local])) : typed(unknown_call(walk_args(env, e.children, 0), e.text), builtin_type(e.text)))))))));  }
Ty closure_sig(Env env, Expr e) { long at = maca_list_index_of_str(env.fns, (*(Expr*)e.children.data[0]).text); return ((at < 0) ? t_any() : (*(Ty*)env.sigs.data[at]));  }
Env unknown_call(Env env, const char* name) { return ((((!starts_lower(name)) || is_prelude_call(name)) || is_element_tag(name)) ? env : (is_host_builtin(name) ? env : complain_as(env, "M0006", maca_cat_own(maca_cat("call to undefined function `", name), "`", 1))));  }
long is_host_builtin(const char* name) { return ((((((((((((((((strcmp(name, "abs") == 0) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) || (strcmp(name, "clamp") == 0)) || (strcmp(name, "input") == 0)) || (strcmp(name, "read_stdin") == 0)) || (strcmp(name, "at_eof") == 0)) || (strcmp(name, "failures") == 0)) || (strcmp(name, "alloc_count") == 0)) || (strcmp(name, "reuse_count") == 0)) || (strcmp(name, "map") == 0)) || (strcmp(name, "data") == 0)) || (strcmp(name, "stored") == 0)) || (strcmp(name, "styles") == 0)) || (strcmp(name, "list_dir") == 0)) || is_register_builtin(name));  }
long is_register_builtin(const char* name) { return (((((((((((((strcmp(name, "mmio_write") == 0) || (strcmp(name, "mmio_read") == 0)) || (strcmp(name, "set_bits") == 0)) || (strcmp(name, "clear_bits") == 0)) || (strcmp(name, "toggle_bits") == 0)) || (strcmp(name, "bit") == 0)) || (strcmp(name, "shl") == 0)) || (strcmp(name, "shr") == 0)) || (strcmp(name, "bit_or") == 0)) || (strcmp(name, "bit_and") == 0)) || (strcmp(name, "delay") == 0)) || (strcmp(name, "nop") == 0)) || (strcmp(name, "forever") == 0));  }
long callable(Env env, Ty held) { return ((resolve(env.infer, held).kind == KFn) || (resolve(env.infer, held).kind == KVar));  }
Typed call_local(Env env, Expr e, Ty held) { Signature made = arg_types(env, e.children, 0, maca_listv(0)); Instance res = fresh(made.env.infer); Env ready = with_infer(made.env, res.infer); return typed(joined(ready, held, t_fn(made.tys, res.ty)), res.ty);  }
Signature arg_types(Env env, MacaList args, long i, MacaList acc) { return ((i >= (args.len)) ? (Signature){ .env = env, .tys = acc } : ({ Typed got = type_in(env, (*(Expr*)args.data[i])); arg_types(got.env, args, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ got.ty })))); }));  }
Typed call_declared(Env env, Expr e, Ty sig) { Instance used = instantiate(env.infer, generalize(env.infer, sig)); Env ready = with_infer(env, used.infer); MacaList params = fn_params(used.ty); return (takes_rest(ready, e, (params.len)) ? typed(noted_shapes(unify_args(ready, maca_list_slice(e.children, 0, ((params.len) - 1)), params, 0), sig.args, used.ty.args, 0), fn_ret(used.ty)) : (((params.len) != (e.children.len)) ? typed(wrong_arity(ready, e, (params.len)), fn_ret(used.ty)) : typed(noted_shapes(unify_args(ready, e.children, params, 0), sig.args, used.ty.args, 0), fn_ret(used.ty))));  }
Env noted_shapes(Env env, MacaList want, MacaList got, long i) { return (((i >= (want.len)) || (i >= (got.len))) ? env : noted_shapes(noted_shape(env, (*(Ty*)want.data[i]), (*(Ty*)got.data[i])), want, got, (i + 1)));  }
Env noted_shape(Env env, Ty want, Ty got) { Ty hole = resolve(env.infer, want); Ty seen = grounded(env.infer, got); return ((((hole.kind != KVar) || (seen.kind == KVar)) || (maca_list_index_of(env.holes, (long)(hole.slot)) >= 0)) ? env : ({ __typeof__(env) _w = env; _w.holes = maca_list_cat(env.holes, maca_listv(1, (long)(hole.slot))); _w.fills = maca_list_cat(env.fills, maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ seen }))); _w; }));  }
Ty solved_ty(Env env, Ty t) { Ty seen = resolve(env.infer, t); long at = ((seen.kind == KVar) ? maca_list_index_of(env.holes, (long)(seen.slot)) : (0 - 1)); return ((at >= 0) ? (*(Ty*)env.fills.data[at]) : ({ __typeof__(seen) _w = seen; _w.args = solved_all(env, seen.args, 0); _w; }));  }
MacaList solved_all(Env env, MacaList ts, long i) { return ((i >= (ts.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ solved_ty(env, (*(Ty*)ts.data[i])) })), solved_all(env, ts, (i + 1))));  }
long takes_rest(Env env, Expr e, long wanted) { return ((maca_list_index_of_str(env.varargs, e.text) >= 0) && ((e.children.len) >= (wanted - 1)));  }
Env wrong_arity(Env env, Expr e, long wanted) { return ((maca_list_index_of_str(env.varargs, e.text) >= 0) ? complain(env, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", e.text), "` takes at least ", 1), maca_int_to_str((wanted - 1)), 3), " argument(s), ", 1), maca_cat_own("given ", maca_int_to_str((e.children.len)), 2), 3)) : complain(env, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", e.text), "` takes ", 1), maca_int_to_str(wanted), 3), " argument(s), given ", 1), maca_int_to_str((e.children.len)), 3)));  }
Env unify_args(Env env, MacaList args, MacaList params, long i) { return ((i >= (args.len)) ? env : ({ Typed got = type_in(env, (*(Expr*)args.data[i])); unify_args(joined(got.env, (*(Ty*)params.data[i]), got.ty), args, params, (i + 1)); }));  }
Env walk_args(Env env, MacaList args, long i) { return ((i >= (args.len)) ? env : walk_args(type_in(env, (*(Expr*)args.data[i])).env, args, (i + 1)));  }
Ty builtin_type(const char* name) { return (is_str_builtin(name) ? t_str() : (is_float_builtin(name) ? t_float() : (is_int_builtin(name) ? t_int() : (is_bool_builtin(name) ? t_bool() : ((strcmp(name, "list_dir") == 0) ? t_array(t_str()) : (is_io_builtin(name) ? t_unit() : (is_element_tag(name) ? t_str() : t_any())))))));  }
long is_element_tag(const char* name) { return ((strcmp(name, "element") == 0) || (maca_str_index_of(ElementTags, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0));  }
long is_prelude_call(const char* name) { return ((((is_str_builtin(name) || is_float_builtin(name)) || is_int_builtin(name)) || is_bool_builtin(name)) || is_io_builtin(name));  }
long is_str_builtin(const char* name) { return (((((((((((((((strcmp(name, "str") == 0) || (strcmp(name, "chr") == 0)) || (strcmp(name, "read_line") == 0)) || (strcmp(name, "capture") == 0)) || (strcmp(name, "capture_err") == 0)) || (strcmp(name, "styles") == 0)) || (strcmp(name, "read_file") == 0)) || (strcmp(name, "env") == 0)) || (strcmp(name, "cwd") == 0)) || (strcmp(name, "real_path") == 0)) || (strcmp(name, "now_iso") == 0)) || (strcmp(name, "format_time") == 0)) || (strcmp(name, "maca_attr") == 0)) || (strcmp(name, "maca_flag") == 0)) || (strcmp(name, "maca_element") == 0));  }
long is_float_builtin(const char* name) { return (((((((((((strcmp(name, "float") == 0) || (strcmp(name, "sqrt") == 0)) || (strcmp(name, "floor") == 0)) || (strcmp(name, "ceil") == 0)) || (strcmp(name, "round") == 0)) || (strcmp(name, "pow") == 0)) || (strcmp(name, "sin") == 0)) || (strcmp(name, "cos") == 0)) || (strcmp(name, "tan") == 0)) || (strcmp(name, "log") == 0)) || (strcmp(name, "exp") == 0));  }
long is_int_builtin(const char* name) { return (((((((((strcmp(name, "int") == 0) || (strcmp(name, "exec") == 0)) || (strcmp(name, "len") == 0)) || (strcmp(name, "ord") == 0)) || (strcmp(name, "sign") == 0)) || (strcmp(name, "gcd") == 0)) || (strcmp(name, "now_ms") == 0)) || (strcmp(name, "file_size") == 0)) || (strcmp(name, "modified_ms") == 0));  }
long is_bool_builtin(const char* name) { return ((((((((((((strcmp(name, "assert") == 0) || (strcmp(name, "assert_eq") == 0)) || (strcmp(name, "at_eof") == 0)) || (strcmp(name, "write_file") == 0)) || (strcmp(name, "file_exists") == 0)) || (strcmp(name, "make_dir") == 0)) || (strcmp(name, "is_dir") == 0)) || (strcmp(name, "remove_file") == 0)) || (strcmp(name, "remove_dir") == 0)) || (strcmp(name, "copy_bytes") == 0)) || (strcmp(name, "chdir") == 0)) || (strcmp(name, "is_tty") == 0));  }
long is_io_builtin(const char* name) { return (((((((((((strcmp(name, "info") == 0) || (strcmp(name, "print") == 0)) || (strcmp(name, "err") == 0)) || (strcmp(name, "warn") == 0)) || (strcmp(name, "debug") == 0)) || (strcmp(name, "notice") == 0)) || (strcmp(name, "crit") == 0)) || (strcmp(name, "alert") == 0)) || (strcmp(name, "emerg") == 0)) || (strcmp(name, "panic") == 0)) || (strcmp(name, "sleep_ms") == 0));  }
long is_compare(const char* op) { return ((((((strcmp(op, "==") == 0) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">=") == 0));  }
long is_logic(const char* op) { return ((strcmp(op, "&&") == 0) || (strcmp(op, "||") == 0));  }
long is_shift(const char* op) { return ((strcmp(op, "<<") == 0) || (strcmp(op, ">>") == 0));  }
Typed binop_type(Env env, Expr e) { return ((strcmp(e.text, "=") == 0) ? type_in(env, (*(Expr*)e.children.data[1])) : operator_type(env, e));  }
Typed operator_type(Env env, Expr e) { Typed lhs = type_in(env, (*(Expr*)e.children.data[0])); Typed rhs = type_in(lhs.env, (*(Expr*)e.children.data[1])); return (((strcmp(e.text, "++") == 0) || joins(rhs.env, e.text, lhs.ty, rhs.ty)) ? typed(rhs.env, concat_type(rhs.env, lhs.ty, rhs.ty)) : ((strcmp(e.text, "..") == 0) ? range_type(rhs.env, lhs.ty, rhs.ty) : (is_shift(e.text) ? shift_type(rhs.env, lhs.ty, rhs.ty) : (is_compare(e.text) ? typed(joined(rhs.env, lhs.ty, rhs.ty), t_bool()) : (is_logic(e.text) ? typed(rhs.env, t_bool()) : arith_type(rhs.env, e.text, lhs.ty, rhs.ty))))));  }
long is_list(Ty t) { return ((t.kind == KCon) && (strcmp(t.name, "Array") == 0));  }
long joins(Env env, const char* op, Ty a, Ty b) { Ty left = resolve(env.infer, a); Ty right = resolve(env.infer, b); return ((strcmp(op, "+") == 0) && (((left.kind == KStr) && (right.kind == KStr)) || (is_list(left) && is_list(right))));  }
Ty concat_type(Env env, Ty a, Ty b) { Ty left = resolve(env.infer, a); Ty right = resolve(env.infer, b); return (is_list(left) ? left : (is_list(right) ? right : t_str()));  }
long numeric(Ty t) { return ((((t.kind == KInt) || (t.kind == KFloat)) || (t.kind == KVar)) || absorbing(t));  }
const char* overload_name(const char* op) { return ((strcmp(op, "+") == 0) ? "add" : ((strcmp(op, "-") == 0) ? "sub" : ((strcmp(op, "*") == 0) ? "mul" : ((strcmp(op, "/") == 0) ? "div" : ""))));  }
long declares_field(Env env, const char* owner, long i) { return ((i >= (env.fields.len)) ? 0 : ((maca_str_index_of(((const char*)env.fields.data[i]), maca_cat(owner, ".")) == 0) ? 1 : declares_field(env, owner, (i + 1))));  }
long is_nominal(Env env, Ty t) { return (((t.kind == KCon) && (strcmp(t.name, "Array") != 0)) && ((maca_list_index_of_str(env.owners, t.name) >= 0) || declares_field(env, t.name, 0)));  }
long overload_at(Env env, const char* op, Ty left) { return (((strcmp(overload_name(op), "") == 0) || (!is_nominal(env, left))) ? (0 - 1) : maca_list_index_of_str(env.fns, overload_name(op)));  }
Typed arith_type(Env env, const char* op, Ty a, Ty b) { long at = overload_at(env, op, resolve(env.infer, a)); return ((at >= 0) ? typed(env, fn_ret((*(Ty*)env.sigs.data[at]))) : (((strcmp(op, "/") == 0) && (resolve(env.infer, a).kind == KStr)) ? typed(env, t_str()) : numeric_type(env, a, b)));  }
Typed numeric_type(Env env, Ty a, Ty b) { Unify step = unify(env.infer, a, b); Env ready = with_infer(env, step.infer); Ty left = resolve(step.infer, a); Ty right = resolve(step.infer, b); return ((strcmp(step.error, "") != 0) ? typed(complain(ready, step.error), t_error()) : (((left.kind == KFloat) || (right.kind == KFloat)) ? typed(ready, t_float()) : (numeric(left) ? typed(ready, t_int()) : ((left.kind == KCon) ? typed(ready, left) : typed(complain(ready, maca_cat_own(maca_cat("", show_ty(left)), " is not a number", 1)), t_error())))));  }
Typed ternary_type(Env env, Expr e) { Typed cond = type_in(env, (*(Expr*)e.children.data[0])); Typed then = type_in(cond.env, (*(Expr*)e.children.data[1])); Typed other = type_in(then.env, (*(Expr*)e.children.data[2])); return typed(joined(other.env, then.ty, other.ty), then.ty);  }
Typed block_type(Env env, Expr e) { Typed inner = check_body(env, e.stmts, 0, t_any()); Typed last = type_in(inner.env, (*(Expr*)e.children.data[0])); return typed(({ __typeof__(last.env) _w = last.env; _w.names = env.names; _w.types = env.types; _w; }), last.ty);  }
Typed method_type(Env env, Expr e) { Typed recv = type_in(env, (*(Expr*)e.children.data[0])); return ((((strcmp(e.text, "map") == 0) || (strcmp(e.text, "parallel") == 0)) && ((e.children.len) == 2)) ? mapped_type(recv.env, resolve(recv.env.infer, recv.ty), (*(Expr*)e.children.data[1])) : (((strcmp(e.text, "filter") == 0) && ((e.children.len) == 2)) ? sifted_type(recv.env, resolve(recv.env.infer, recv.ty), (*(Expr*)e.children.data[1])) : ((field_fn_ty(env, resolve(recv.env.infer, recv.ty), e.text).kind == KFn) ? typed(walk_args(recv.env, e.children, 1), fn_ret(field_fn_ty(env, resolve(recv.env.infer, recv.ty), e.text))) : (is_ufcs_call(env, e, resolve(recv.env.infer, recv.ty)) ? call_type(env, e_call(e.text, e.children)) : ({ Env walked = walk_args(recv.env, e.children, 1); typed(walked, method_result(e.text, resolve(walked.infer, recv.ty))); })))));  }
Ty field_fn_ty(Env env, Ty recv, const char* name) { long at = maca_list_index_of_str(env.fields, maca_cat_own(maca_cat(recv.name, "."), name, 1)); return (((recv.kind != KCon) || (at < 0)) ? t_any() : (*(Ty*)env.ftypes.data[at]));  }
long is_ufcs_call(Env env, Expr e, Ty recv) { return ((maca_list_index_of_str(env.fns, e.text) >= 0) && ((method_result(e.text, recv).kind == KAny) || own_method(recv)));  }
long own_method(Ty t) { return ((t.kind == KRec) || (((t.kind == KCon) && (strcmp(t.name, "Array") != 0)) && (!is_map_ty(t))));  }
Typed mapped_type(Env env, Ty recv, Expr f) { Typed got = type_in(env, f); Instance res = fresh(got.env.infer); Env ready = joined(with_infer(got.env, res.infer), got.ty, t_fn(maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ element_of(recv) })), res.ty)); return typed(ready, t_array(resolve(ready.infer, res.ty)));  }
Typed sifted_type(Env env, Ty recv, Expr f) { Typed got = type_in(env, f); Env ready = joined(got.env, got.ty, t_fn(maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ element_of(recv) })), t_bool())); return typed(ready, recv);  }
Ty method_result(const char* name, Ty recv) { return (is_map_ty(recv) ? map_method_result(name, recv) : ((((strcmp(name, "length") == 0) || (strcmp(name, "count") == 0)) || (strcmp(name, "index_of") == 0)) ? t_int() : ((strcmp(name, "at") == 0) ? element_of(recv) : ((strcmp(name, "join") == 0) ? t_str() : ((strcmp(name, "chars") == 0) ? t_array(t_str()) : (((strcmp(name, "slice") == 0) || (strcmp(name, "push") == 0)) ? recv : ((strcmp(name, "get") == 0) ? element_of(recv) : (is_reshaping_method(name) ? recv : (is_picking_method(name) ? element_of(recv) : (is_text_method(name) ? t_str() : ((strcmp(name, "split") == 0) ? t_array(t_str()) : (is_asking_method(name) ? t_bool() : t_any()))))))))))));  }
long is_map_ty(Ty t) { return ((t.kind == KCon) && (strcmp(map_type_val(t.name), "") != 0));  }
Ty map_method_result(const char* name, Ty recv) { return ((strcmp(name, "get") == 0) ? ty_named(map_type_val(recv.name)) : ((strcmp(name, "keys") == 0) ? t_array(ty_named(map_type_key(recv.name))) : ((strcmp(name, "values") == 0) ? t_array(ty_named(map_type_val(recv.name))) : (((strcmp(name, "has") == 0) || (strcmp(name, "contains") == 0)) ? t_bool() : (((strcmp(name, "length") == 0) || (strcmp(name, "count") == 0)) ? t_int() : recv)))));  }
long is_reshaping_method(const char* name) { return ((((((strcmp(name, "pop") == 0) || (strcmp(name, "reverse") == 0)) || (strcmp(name, "sort") == 0)) || (strcmp(name, "set") == 0)) || (strcmp(name, "insert") == 0)) || (strcmp(name, "remove") == 0));  }
long is_picking_method(const char* name) { return (((((strcmp(name, "sum") == 0) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) || (strcmp(name, "first") == 0)) || (strcmp(name, "last") == 0));  }
long is_text_method(const char* name) { return ((((((((((strcmp(name, "trim") == 0) || (strcmp(name, "lower") == 0)) || (strcmp(name, "upper") == 0)) || (strcmp(name, "replace") == 0)) || (strcmp(name, "repeat") == 0)) || (strcmp(name, "substr") == 0)) || (strcmp(name, "pad_start") == 0)) || (strcmp(name, "pad_end") == 0)) || (strcmp(name, "fixed") == 0)) || (strcmp(name, "pad_center") == 0));  }
long is_asking_method(const char* name) { return ((((((strcmp(name, "starts_with") == 0) || (strcmp(name, "ends_with") == 0)) || (strcmp(name, "contains") == 0)) || (strcmp(name, "is_whitespace") == 0)) || (strcmp(name, "is_ascii_digit") == 0)) || (strcmp(name, "is_alpha") == 0));  }
Ty element_of(Ty t) { return ((t.kind == KStr) ? t_str() : ((((t.kind == KCon) && (strcmp(t.name, "Array") == 0)) && ((t.args.len) == 1)) ? (*(Ty*)t.args.data[0]) : t_any()));  }
Typed with_type(Env env, Expr e) { Typed base = type_in(env, (*(Expr*)e.children.data[0])); Ty owner = resolve(base.env.infer, base.ty); return typed(check_fields(base.env, owner.name, e.children, 1), base.ty);  }
Env check_fields(Env env, const char* rec, MacaList fs, long i) { return ((i >= (fs.len)) ? env : check_fields(field_set(env, rec, (*(Expr*)fs.data[i])), rec, fs, (i + 1)));  }
Env field_set(Env env, const char* rec, Expr f) { long at = maca_list_index_of_str(env.fields, maca_cat_own(maca_cat(rec, "."), (*(Expr*)f.children.data[0]).text, 1)); Typed got = type_in(env, f); return ((at < 0) ? got.env : joined(got.env, (*(Ty*)env.ftypes.data[at]), got.ty));  }
Env check_literal(Env env, const char* rec, MacaList fs) { return ((!declares_field(env, rec, 0)) ? env : unknown_fields(missing_fields(env, rec, fs), rec, fs, 0));  }
Env missing_fields(Env env, const char* rec, MacaList fs) { MacaList absent = absent_fields(env, rec, fs, 0, maca_listv(0)); const char* named = maca_list_join(maca_list_sorted(absent, 1), ", "); return (((absent.len) == 0) ? env : complain(env, maca_cat_own(maca_cat_own(maca_cat("`", rec), "` is missing field(s): ", 1), named, 1)));  }
MacaList absent_fields(Env env, const char* rec, MacaList fs, long i, MacaList acc) { return ((i >= (env.fields.len)) ? acc : (((maca_str_index_of(((const char*)env.fields.data[i]), maca_cat(rec, ".")) != 0) || names_field(fs, field_tail(((const char*)env.fields.data[i]), rec), 0)) ? absent_fields(env, rec, fs, (i + 1), acc) : absent_fields(env, rec, fs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(field_tail(((const char*)env.fields.data[i]), rec)))))));  }
const char* field_tail(const char* key, const char* rec) { return maca_str_slice(key, (((int)strlen(rec)) + 1), ((int)strlen(key)));  }
long names_field(MacaList fs, const char* name, long i) { return ((i >= (fs.len)) ? 0 : ((strcmp(lit_field_name((*(Expr*)fs.data[i])), name) == 0) || names_field(fs, name, (i + 1))));  }
const char* lit_field_name(Expr f) { return (*(Expr*)f.children.data[0]).text;  }
Env unknown_fields(Env env, const char* rec, MacaList fs, long i) { return ((i >= (fs.len)) ? env : ((maca_list_index_of_str(env.fields, maca_cat_own(maca_cat(rec, "."), lit_field_name((*(Expr*)fs.data[i])), 1)) >= 0) ? unknown_fields(env, rec, fs, (i + 1)) : ({ const char* told = lit_field_name((*(Expr*)fs.data[i])); unknown_fields(complain(env, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", rec), "` has no field `", 1), told, 1), "`", 1)), rec, fs, (i + 1)); })));  }
Typed field_type(Env env, Expr e) { Typed base = type_in(env, (*(Expr*)e.children.data[0])); Ty owner = resolve(base.env.infer, base.ty); long at = maca_list_index_of_str(env.fields, maca_cat_own(maca_cat(owner.name, "."), e.text, 1)); return ((at < 0) ? typed(base.env, t_any()) : typed(base.env, (*(Ty*)env.ftypes.data[at])));  }
const char* slot_types(Expr v) { return maca_list_join(({ MacaList _m = v.children; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(binder_type((*(Expr*)_m.data[_i]))); _r; }), ",");  }
const char* binder_type(Expr b) { return b.ty;  }
Typed match_type(Env env, Expr e) { return (((e.children.len) < 3) ? typed(walk_args(env, e.children, 0), t_any()) : ({ Typed seen = type_in(env, (*(Expr*)e.children.data[0])); Typed first = type_in(bound_arm(seen.env, (*(Expr*)e.children.data[1])), (*(Expr*)e.children.data[2])); Env whole = unify_arms(first.env, e.children, first.ty, 3); typed(check_arms(whole, e, resolve(whole.infer, seen.ty)), first.ty); }));  }
Env check_arms(Env env, Expr e, Ty scrut) { return ((((scrut.kind != KCon) || (maca_list_index_of_str(env.owners, scrut.name) < 0)) || catches_all(env, e, 1)) ? env : ({ MacaList left = uncovered(env, e, scrut.name, 0, maca_listv(0)); const char* named = maca_list_join(left, ", "); (((left.len) == 0) ? env : complain_as(env, "M0002", maca_cat_own(maca_cat_own(maca_cat("match on `", scrut.name), "` is not exhaustive; missing: ", 1), named, 1))); }));  }
long catches_all(Env env, Expr e, long i) { return (((i + 1) >= (e.children.len)) ? 0 : (wide_arm(env, (*(Expr*)e.children.data[i])) || catches_all(env, e, (i + 2))));  }
long wide_arm(Env env, Expr p) { return ((p.kind == EGuard) ? 0 : (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? (wide_arm(env, (*(Expr*)p.children.data[0])) || wide_arm(env, (*(Expr*)p.children.data[1]))) : (((p.kind != EIdent) || ((p.children.len) > 0)) ? 0 : ((strcmp(p.text, "_") == 0) || (starts_lower(p.text) && (maca_list_index_of_str(env.ctors, p.text) < 0))))));  }
MacaList uncovered(Env env, Expr e, const char* sum, long i, MacaList acc) { return ((i >= (env.ctors.len)) ? acc : (((strcmp(((const char*)env.owners.data[i]), sum) != 0) || arm_names(e, ((const char*)env.ctors.data[i]), 1)) ? uncovered(env, e, sum, (i + 1), acc) : uncovered(env, e, sum, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)env.ctors.data[i])))))));  }
long arm_names(Expr e, const char* ctor, long i) { return (((i + 1) >= (e.children.len)) ? 0 : (pat_names((*(Expr*)e.children.data[i]), ctor) || arm_names(e, ctor, (i + 2))));  }
long pat_names(Expr p, const char* ctor) { return ((p.kind == EGuard) ? 0 : (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? (pat_names((*(Expr*)p.children.data[0]), ctor) || pat_names((*(Expr*)p.children.data[1]), ctor)) : ((p.kind == EIdent) && (strcmp(p.text, ctor) == 0))));  }
Env unify_arms(Env env, MacaList cs, Ty want, long i) { return (((i + 1) >= (cs.len)) ? env : ({ Typed got = type_in(bound_arm(env, (*(Expr*)cs.data[i])), (*(Expr*)cs.data[(i + 1)])); unify_arms(joined(got.env, want, got.ty), cs, want, (i + 2)); }));  }
Env bound_arm(Env env, Expr pat) { return ((pat.kind == EGuard) ? bound_arm(env, (*(Expr*)pat.children.data[0])) : (((pat.kind == EBinary) && (strcmp(pat.text, "|") == 0)) ? bound_arm(bound_arm(env, (*(Expr*)pat.children.data[0])), (*(Expr*)pat.children.data[1])) : (((pat.children.len) == 0) ? named_pattern(env, pat) : ({ long at = maca_list_index_of_str(env.ctors, pat.text); ((at < 0) ? env : bind_slots_of(env, pat.children, maca_split(((const char*)env.slots.data[at]), ","), 0)); }))));  }
Env named_pattern(Env env, Expr pat) { return (((pat.kind != EIdent) || (!starts_upper(pat.text))) ? env : ((maca_list_index_of_str(env.ctors, pat.text) >= 0) ? env : complain_as(env, "M0006", maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", pat.text), "` is capitalized, so it is a constructor,", 1), " and nothing declares one by that name:", 1), maca_cat(" ", ctor_hint(env, pat.text)), 3))));  }
long starts_upper(const char* name) { return (((((int)strlen(name)) > 0) && (isalpha((unsigned char)(maca_str_at(name, 0))[0]) != 0)) && (!starts_lower(name)));  }
const char* ctor_hint(Env env, const char* name) { const char* near = nearest_ctor(name, env.ctors, 0, "", hint_span(name)); return ((strcmp(near, "") == 0) ? "a pattern that binds what it matched is lowercase" : maca_cat_own(maca_cat("did you mean `", near), "`?", 1));  }
long hint_span(const char* name) { return ((((int)strlen(name)) <= 3) ? 2 : ((((int)strlen(name)) <= 7) ? 3 : 4));  }
const char* nearest_ctor(const char* want, MacaList names, long i, const char* found, long best) { return ((i >= (names.len)) ? found : ({ long apart = edits_apart(want, ((const char*)names.data[i]), best); ((apart < best) ? nearest_ctor(want, names, (i + 1), ((const char*)names.data[i]), apart) : nearest_ctor(want, names, (i + 1), found, best)); }));  }
long edits_apart(const char* a, const char* b, long cap) { return (((((int)strlen(a)) > (((int)strlen(b)) + cap)) || (((int)strlen(b)) > (((int)strlen(a)) + cap))) ? cap : edit_rows(a, b, edit_row((((int)strlen(b)) + 1), 0, maca_listv(0)), 0));  }
long edit_rows(const char* a, const char* b, MacaList prev, long i) { return ((i >= ((int)strlen(a))) ? ((long)prev.data[((int)strlen(b))]) : edit_rows(a, b, edit_cells(a, b, prev, maca_listv(1, (long)((i + 1))), i, 0), (i + 1)));  }
MacaList edit_cells(const char* a, const char* b, MacaList prev, MacaList cur, long i, long j) { return ((j >= ((int)strlen(b))) ? cur : ({ long cost = ((strcmp(maca_str_at(a, i), maca_str_at(b, j)) == 0) ? 0 : 1); long best = least_of((((long)prev.data[j]) + cost), (((long)prev.data[(j + 1)]) + 1), (((long)cur.data[j]) + 1)); edit_cells(a, b, prev, maca_list_cat(cur, maca_listv(1, (long)(best))), i, (j + 1)); }));  }
MacaList edit_row(long n, long at, MacaList acc) { return ((at >= n) ? acc : edit_row(n, (at + 1), maca_list_cat(acc, maca_listv(1, (long)(at)))));  }
long least_of(long a, long b, long c) { long m = ((a < b) ? a : b); return ((m < c) ? m : c);  }
Env bound_cells(Env env, Expr pat, Ty el) { return ((pat.kind == EGuard) ? bound_cells(env, (*(Expr*)pat.children.data[0]), el) : (((pat.kind == EIdent) && ((strcmp(pat.text, "[]") == 0) || (strcmp(pat.text, "[..]") == 0))) ? bind_cell_names(env, pat, el, 0) : env));  }
Env bind_cell_names(Env env, Expr pat, Ty el, long i) { return ((i >= (pat.children.len)) ? env : (((*(Expr*)pat.children.data[i]).kind != EIdent) ? bind_cell_names(env, pat, el, (i + 1)) : (((strcmp(pat.text, "[..]") == 0) && (i == ((pat.children.len) - 1))) ? bind_mc(env, (*(Expr*)pat.children.data[i]).text, t_array(el)) : bind_cell_names(bind_mc(env, (*(Expr*)pat.children.data[i]).text, el), pat, el, (i + 1)))));  }
Env bind_slots_of(Env env, MacaList bs, MacaList tys, long i) { return (((i >= (bs.len)) || (i >= (tys.len))) ? env : bind_slots_of(bind_mc(env, (*(Expr*)bs.data[i]).text, ty_named(((const char*)tys.data[i]))), bs, tys, (i + 1)));  }
Typed list_type(Env env, Expr e) { return (((e.children.len) == 0) ? typed(env, t_array(t_any())) : ({ Typed first = type_in(env, (*(Expr*)e.children.data[0])); typed(unify_from(first.env, e.children, first.ty, 1, 1), t_array(first.ty)); }));  }
Env unify_from(Env env, MacaList cs, Ty want, long i, long step) { return ((i >= (cs.len)) ? env : ({ Typed got = type_in(env, (*(Expr*)cs.data[i])); unify_from(joined(got.env, want, got.ty), cs, want, (i + step), step); }));  }
Env check_fn_in(Env env, Stmt s) { Env inner = bind_params(env, s.params, signature_params(env, s.name), 0); Typed body = check_body(inner, s.body, 0, t_any()); Env told = (((strcmp(s.ret, "") == 0) && ((s.body.len) > 0)) ? joined(body.env, signature_ret(env, s.name), body.ty) : body.env); Env checked = return_check(told, s.ret, body.ty); return ({ __typeof__(checked) _w = checked; _w.names = env.names; _w.types = env.types; _w.frozen = env.frozen; _w; });  }
Ty signature_ret(Env env, const char* name) { long at = maca_list_index_of_str(env.fns, name); return ((at < 0) ? t_any() : fn_ret((*(Ty*)env.sigs.data[at])));  }
MacaList signature_params(Env env, const char* name) { long at = maca_list_index_of_str(env.fns, name); return ((at < 0) ? maca_listv(0) : fn_params((*(Ty*)env.sigs.data[at])));  }
Env bind_params(Env env, MacaList ps, MacaList tys, long i) { return ((i >= (ps.len)) ? env : ((i < (tys.len)) ? bind_params(bind_mc(env, (*(Expr*)ps.data[i]).text, (*(Ty*)tys.data[i])), ps, tys, (i + 1)) : ({ Typed got = declared_type(env, (*(Expr*)ps.data[i]).ty); bind_params(bind_mc(got.env, (*(Expr*)ps.data[i]).text, got.ty), ps, tys, (i + 1)); })));  }
Typed check_body(Env env, MacaList stmts, long i, Ty last) { return ((i >= (stmts.len)) ? typed(env, last) : ({ Stmt st = (*(Stmt*)stmts.data[i]); Typed got = stmt_type(env, st, (i == ((stmts.len) - 1))); check_body(extend(got.env, st, got.ty), stmts, (i + 1), got.ty); }));  }
Typed stmt_type(Env env, Stmt s, long is_last) { return (((is_last || (s.kind != SExpr)) || (!branches_for_effect(s.value))) ? type_in(env, s.value) : typed(effect_walk(env, s.value), t_unit()));  }
long branches_for_effect(Expr e) { return ((e.kind == EIf) || (e.kind == EMatch));  }
Env effect_walk(Env env, Expr e) { return ((e.kind == EIf) ? ({ Typed cond = type_in(env, (*(Expr*)e.children.data[0])); effect_walk(effect_walk(cond.env, (*(Expr*)e.children.data[1])), (*(Expr*)e.children.data[2])); }) : (((e.kind == EMatch) && ((e.children.len) >= 3)) ? ({ Typed seen = type_in(env, (*(Expr*)e.children.data[0])); effect_arms(seen.env, e.children, 1); }) : ((e.kind == EBlock) ? ({ Typed inner = check_body(env, e.stmts, 0, t_any()); Env after = effect_walk(inner.env, (*(Expr*)e.children.data[0])); ({ __typeof__(after) _w = after; _w.names = env.names; _w.types = env.types; _w; }); }) : type_in(env, e).env)));  }
Env effect_arms(Env env, MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? env : effect_arms(effect_walk(bound_arm(env, (*(Expr*)cs.data[i])), (*(Expr*)cs.data[(i + 1)])), cs, (i + 2)));  }
Env extend(Env env, Stmt s, Ty ty) { return ((s.kind != SBind) ? env : ((maca_list_index_of_str(env.frozen, s.name) >= 0) ? complain_as(env, "M0005", maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("cannot reassign constant `", s.name), "`;", 1), " declare it mutable with", 1), maca_cat_own(maca_cat(" `", s.name), " = …` (no `const`)", 1), 3)) : ((strcmp(s.ret, "") != 0) ? sealed(bind_mc(joined(env, ty_named(s.ret), ty), s.name, ty_named(s.ret)), s) : sealed(bind_mc(env, s.name, ty), s))));  }
Env sealed(Env env, Stmt s) { return ((s.frozen || starts_upper(s.name)) ? ({ __typeof__(env) _w = env; _w.frozen = maca_list_cat(env.frozen, maca_listv(1, (long)(s.name))); _w; }) : env);  }
Env return_check(Env env, const char* declared, Ty actual) { return ((strcmp(declared, "") == 0) ? env : joined(env, ty_named(declared), actual));  }
Env checked_module(Module m) { Env start = env_of_module(m); Env inferred = check_items(start, m.items, 0); return check_items(with_infer(start, inferred.infer), m.items, 0);  }
long check_module(Module m) { return (checked_module(m).errors.len);  }
Env check_items(Env env, MacaList items, long i) { return ((i >= (items.len)) ? env : check_items(check_item(env, (*(Stmt*)items.data[i])), items, (i + 1)));  }
Env check_item(Env env, Stmt s) { return ((s.kind == SFn) ? check_fn_in(({ __typeof__(env) _w = env; _w.here = s.pos; _w; }), s) : env);  }
MacaList check_diagnostics(Module m) { return maca_list_cat(maca_list_cat(maca_list_cat(checked_module(m).errors, clashing_names(m.items, 0, maca_listv(0), maca_listv(0))), variadic_errors(m.items, 0, maca_listv(0))), config_errors(m.items));  }
MacaList check_errors(Module m) { return ({ MacaList _m = check_diagnostics(m); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(diag_message((*(Diagnostic*)_m.data[_i]))); _r; });  }
MacaList config_errors(MacaList items) { return (writes_an_option(items, 0) ? config_refusals(items, 0, maca_listv(0)) : maca_listv(0));  }
long writes_an_option(MacaList items, long i) { return ((i >= (items.len)) ? 0 : ((((*(Stmt*)items.data[i]).kind == SBind) && (maca_str_index_of((*(Stmt*)items.data[i]).name, ".") > 0)) ? 1 : writes_an_option(items, (i + 1))));  }
MacaList config_refusals(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : config_refusals(items, (i + 1), maca_list_cat(acc, config_refusal((*(Stmt*)items.data[i])))));  }
MacaList config_refusal(Stmt s) { return ((s.kind == SFn) ? ((((s.body.len) == 0) && is_io_builtin(s.name)) ? maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ diag_at(s, "M0003", impure_config(s.name)) })) : maca_listv(0)) : ((s.kind != SBind) ? maca_listv(0) : maca_list_cat(impure_value(s), misspelt_option(s))));  }
MacaList impure_value(Stmt s) { const char* called = effectful_call(s.value); return ((strcmp(called, "") == 0) ? maca_listv(0) : maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ diag_at(s, "M0003", impure_config(called)) })));  }
const char* impure_config(const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("config must be pure, and `", name), "` performs ", 1), effect_of(name), 1), ": a NixOS", 1), " module is data, so there is no moment at which it could run", 1);  }
const char* effect_of(const char* name) { return (is_async_effect(name) ? "async" : "io");  }
long is_async_effect(const char* word) { return ((strcmp(word, "spawn") == 0) || (strcmp(word, "await") == 0));  }
const char* effectful_call(Expr e) { return (((e.kind == EUnary) && is_async_effect(e.text)) ? e.text : ((((e.kind == ECall) || (e.kind == EMethod)) && is_io_builtin(e.text)) ? e.text : effectful_in(e.children, 0)));  }
const char* effectful_in(MacaList xs, long i) { return ((i >= (xs.len)) ? "" : ({ const char* found = effectful_call((*(Expr*)xs.data[i])); ((strcmp(found, "") == 0) ? effectful_in(xs, (i + 1)) : found); }));  }
MacaList misspelt_option(Stmt s) { const char* root = option_root(s.name); const char* near = (((strcmp(root, "") == 0) || known_root(root)) ? "" : nearest_ctor(root, nixos_roots(), 0, "", hint_span(root))); const char* why = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("unknown NixOS option namespace `", root), "`: did you mean `", 1), near, 1), "`?", 1); return ((strcmp(near, "") == 0) ? maca_listv(0) : maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ diag_at(s, "M0004", why) })));  }
const char* option_root(const char* name) { long cut = maca_str_index_of(name, "."); return ((cut < 0) ? "" : maca_str_slice(name, 0, cut));  }
long known_root(const char* root) { return (maca_str_index_of(NixosRoots, maca_cat_own(maca_cat(" ", root), " ", 1)) >= 0);  }
MacaList nixos_roots() { return maca_split(maca_trim(NixosRoots), " ");  }
MacaList clashing_names(MacaList items, long i, MacaList seen, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind != SFn) ? clashing_names(items, (i + 1), seen, acc) : ((maca_list_index_of_str(seen, (*(Stmt*)items.data[i]).name) >= 0) ? clashing_names(items, (i + 1), seen, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ clash_of((*(Stmt*)items.data[i])) })))) : clashing_names(items, (i + 1), maca_list_cat(seen, maca_listv(1, (long)((*(Stmt*)items.data[i]).name))), acc))));  }
Diagnostic clash_of(Stmt s) { const char* why = maca_cat_own(maca_cat_own(maca_cat("`", s.name), "` is defined twice; imports share one namespace, so two", 1), " packages cannot both name a function this", 1); return diag_at(s, "M0001", why);  }
const char* surface_of(Ty t) { return (t.kind == KInt ? "int" : (t.kind == KFloat ? "float" : (t.kind == KStr ? "str" : (t.kind == KBool ? "bool" : (t.kind == KBytes ? "bytes" : (t.kind == KCon ? con_surface(t) : (t.kind == KFn ? fn_surface(t) : (t.kind == KOpt ? surface_of((*(Ty*)t.args.data[0])) : ""))))))));  }
const char* con_surface(Ty t) { return (((strcmp(t.name, "Array") == 0) && ((t.args.len) == 1)) ? maca_cat(surface_of((*(Ty*)t.args.data[0])), "[]") : t.name);  }
const char* fn_surface(Ty t) { return maca_cat_own(maca_cat_own(maca_cat("(", surface_joined(fn_params(t), 0)), ") -> ", 1), surface_of(fn_ret(t)), 1);  }
const char* surface_joined(MacaList ts, long i) { return ((i >= (ts.len)) ? "" : ((i == ((ts.len) - 1)) ? surface_of((*(Ty*)ts.data[i])) : maca_cat_own(maca_cat(surface_of((*(Ty*)ts.data[i])), ", "), surface_joined(ts, (i + 1)), 1)));  }
Module annotated(Module m) { return (Module){ .items = annotate_items(checked_module(m), m.items, 0, maca_listv(0)), .errors = m.errors };  }
MacaList annotate_items(Env env, MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : annotate_items(env, items, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ annotate_item(env, (*(Stmt*)items.data[i])) })))));  }
Stmt annotate_item(Env env, Stmt s) { return (is_impl_block(s) ? ({ __typeof__(s) _w = s; _w.value = annotated_methods(env, s.value); _w; }) : ((s.kind == SBind) ? ({ __typeof__(s) _w = s; _w.value = annotate_expr(env, s.value); _w; }) : ((s.kind != SFn) ? s : ({ MacaList tys = signature_params(env, s.name); Env inner = bind_params(env, s.params, tys, 0); long keep = ((s.body.len) > 0); ({ __typeof__(s) _w = s; _w.ret = erased_ret(item_ret(env, s), keep); _w.params = annotate_params(env, s.params, tys, 0, keep); _w.body = annotate_body(inner, s.body, 0, maca_listv(0)); _w; }); }))));  }
Expr annotated_methods(Env env, Expr e) { return ({ __typeof__(e) _w = e; _w.children = annotated_each(env, e.children, 0, maca_listv(0)); _w; });  }
MacaList annotated_each(Env env, MacaList fs, long i, MacaList acc) { return ((i >= (fs.len)) ? acc : ({ Expr one = annotated_method(env, (*(Expr*)fs.data[i])); annotated_each(env, fs, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ one })))); }));  }
Expr annotated_method(Env env, Expr f) { Expr lam = (*(Expr*)f.children.data[1]); MacaList kept = maca_list_slice(lam.children, 0, ((lam.children.len) - 1)); Expr body = annotate_expr(env, lambda_body(lam)); Expr made = ({ __typeof__(lam) _w = lam; _w.children = maca_list_cat(kept, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ body }))); _w; }); return ({ __typeof__(f) _w = f; _w.children = maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ (*(Expr*)f.children.data[0]) }), maca_box(sizeof(Expr), (Expr[]){ made })); _w; });  }
const char* item_ret(Env env, Stmt s) { return (((strcmp(s.ret, "") != 0) || ((s.body.len) == 0)) ? s.ret : surface_of(solved_ty(env, signature_ret(env, s.name))));  }
const char* erased_ret(const char* ret, long keep) { return ((keep && has_type_var(ret)) ? ret : ((ty_named(ret).kind == KAny) ? "" : ret));  }
MacaList annotate_params(Env env, MacaList ps, MacaList tys, long i, long keep) { return ((i >= (ps.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ annotate_param(env, (*(Expr*)ps.data[i]), tys, i, keep) })), annotate_params(env, ps, tys, (i + 1), keep)));  }
Expr annotate_param(Env env, Expr p, MacaList tys, long i, long keep) { return (((i >= (tys.len)) || (keep && has_type_var(p.ty))) ? p : ((!keep) ? ({ __typeof__(p) _w = p; _w.ty = surface_of(grounded(env.infer, (*(Ty*)tys.data[i]))); _w; }) : ({ __typeof__(p) _w = p; _w.ty = surface_of(solved_ty(env, (*(Ty*)tys.data[i]))); _w; })));  }
Ty grounded(Infer inf, Ty t) { Ty seen = resolve(inf, t); return ({ __typeof__(seen) _w = seen; _w.args = grounded_all(inf, seen.args, 0); _w; });  }
MacaList grounded_all(Infer inf, MacaList ts, long i) { return ((i >= (ts.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Ty), (Ty[]){ grounded(inf, (*(Ty*)ts.data[i])) })), grounded_all(inf, ts, (i + 1))));  }
MacaList annotate_body(Env env, MacaList stmts, long i, MacaList acc) { return ((i >= (stmts.len)) ? acc : ({ Stmt st = reassigned(env, (*(Stmt*)stmts.data[i])); Typed got = type_in(env, st.value); Stmt marked = ({ __typeof__(st) _w = st; _w.value = annotate_expr(env, st.value); _w; }); annotate_body(extend(got.env, st, got.ty), stmts, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ marked })))); }));  }
Stmt reassigned(Env env, Stmt s) { return (((s.kind == SBind) && (maca_list_index_of_str(env.names, s.name) >= 0)) ? ({ __typeof__(s) _w = s; _w.kind = SSet; _w; }) : s);  }
Expr annotate_expr(Env env, Expr e) { return (is_element_call(env, e) ? annotate_expr(env, lowered_element(env, e)) : (is_rest_call(env, e) ? annotated_node(env, gathered_rest(env, e)) : (is_field_call(env, e) ? annotated_node(env, ({ __typeof__(e) _w = e; _w.ival = 1; _w; })) : (((e.kind == ECall) && holds_fn(env, e.text)) ? annotated_node(env, ({ __typeof__(e) _w = e; _w.ival = 2; _w; })) : (((e.kind == EIdent) && holds_fn(env, e.text)) ? annotated_node(env, ({ __typeof__(e) _w = e; _w.ival = 3; _w; })) : (((e.kind == EIdent) && (maca_list_index_of_str(env.names, e.text) >= 0)) ? annotated_node(env, ({ __typeof__(e) _w = e; _w.ival = 4; _w; })) : annotated_node(env, e)))))));  }
long holds_fn(Env env, const char* name) { long at = maca_list_index_of_str(env.names, name); return ((at < 0) ? 0 : (resolve(env.infer, (*(Ty*)env.types.data[at])).kind == KFn));  }
long is_bound_name(Expr e) { return ((e.kind == EIdent) && ((e.ival == 3) || (e.ival == 4)));  }
long is_field_call(Env env, Expr e) { return ((e.kind == EMethod) && (field_fn_ty(env, resolve(env.infer, type_in(env, (*(Expr*)e.children.data[0])).ty), e.text).kind == KFn));  }
long is_rest_call(Env env, Expr e) { return ((e.kind == ECall) && (maca_list_index_of_str(env.varargs, e.text) >= 0));  }
Expr gathered_rest(Env env, Expr e) { long at = fixed_arity(env, e.text); return ({ __typeof__(e) _w = e; _w.children = maca_list_cat(maca_list_slice(e.children, 0, at), maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_list(maca_list_slice(e.children, at, (e.children.len))) }))); _w; });  }
long fixed_arity(Env env, const char* name) { return ((fn_params((*(Ty*)env.sigs.data[maca_list_index_of_str(env.fns, name)])).len) - 1);  }
Expr annotated_node(Env env, Expr e) { Typed inner = check_body(env, e.stmts, 0, t_any()); Typed seen = type_in(env, e); return ({ __typeof__(e) _w = e; _w.ty = surface_of(solved_ty(seen.env, seen.ty)); _w.stmts = annotate_body(looped(env, e), e.stmts, 0, maca_listv(0)); _w.children = annotate_children(inner.env, e, 0, maca_listv(0)); _w; });  }
long is_element_call(Env env, Expr e) { return ((((((e.kind == ECall) && is_element_tag(e.text)) && (!is_prelude_call(e.text))) && (maca_list_index_of_str(env.ctors, e.text) < 0)) && (maca_list_index_of_str(env.names, e.text) < 0)) && ((maca_list_index_of_str(env.fns, e.text) < 0) || tag_wins(env, e, maca_list_index_of_str(env.fns, e.text))));  }
long tag_wins(Env env, Expr e, long declared) { return ((((is_element_tag(e.text) && (!is_prelude_call(e.text))) && (maca_list_index_of_str(env.names, e.text) < 0)) && (maca_list_index_of_str(env.varargs, e.text) < 0)) && ((fn_params((*(Ty*)env.sigs.data[declared])).len) != (e.children.len)));  }
Expr lowered_element(Env env, Expr e) { long named = ((strcmp(e.text, "element") == 0) && ((e.children.len) > 0)); long from = (named ? 1 : 0); Expr tag = (named ? (*(Expr*)e.children.data[0]) : e_str(e.text)); return e_call("maca_element", maca_listv(3, maca_box(sizeof(Expr), (Expr[]){ tag }), maca_box(sizeof(Expr), (Expr[]){ element_attrs(env, e.children, from, e_str("")) }), maca_box(sizeof(Expr), (Expr[]){ element_kids(env, e.children, from, e_str("")) })));  }
Expr element_attrs(Env env, MacaList cs, long i, Expr acc) { return ((i >= (cs.len)) ? acc : (((*(Expr*)cs.data[i]).kind != EAttr) ? element_attrs(env, cs, (i + 1), acc) : element_attrs(env, cs, (i + 1), e_binary("++", acc, one_attribute(env, (*(Expr*)cs.data[i]))))));  }
Expr one_attribute(Env env, Expr a) { Expr value = (*(Expr*)a.children.data[0]); return ((resolve(env.infer, type_in(env, value).ty).kind == KBool) ? e_call("maca_flag", maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ e_str(a.text) }), maca_box(sizeof(Expr), (Expr[]){ value }))) : e_call("maca_attr", maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ e_str(a.text) }), maca_box(sizeof(Expr), (Expr[]){ as_html_text(env, value) }))));  }
Expr element_kids(Env env, MacaList cs, long i, Expr acc) { return ((i >= (cs.len)) ? acc : (((*(Expr*)cs.data[i]).kind == EAttr) ? element_kids(env, cs, (i + 1), acc) : element_kids(env, cs, (i + 1), e_binary("++", acc, as_html_text(env, (*(Expr*)cs.data[i]))))));  }
Expr as_html_text(Env env, Expr v) { Ty t = resolve(env.infer, type_in(env, v).ty); return ((t.kind == KStr) ? v : (is_str_list(env.infer, t) ? e_method(v, "join", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_str("") }))) : e_call("str", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ v })))));  }
long is_str_list(Infer inf, Ty t) { return ((((t.kind == KCon) && (strcmp(t.name, "Array") == 0)) && ((t.args.len) == 1)) && (resolve(inf, (*(Ty*)t.args.data[0])).kind == KStr));  }
Env looped(Env env, Expr e) { return (((e.kind != EFor) || ((e.children.len) == 0)) ? env : ({ Typed over = type_in(env, (*(Expr*)e.children.data[0])); bind_mc(over.env, e.text, element_of(resolve(over.env.infer, over.ty))); }));  }
MacaList annotate_children(Env env, Expr e, long i, MacaList acc) { return ((i >= (e.children.len)) ? acc : annotate_children(env, e, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ annotate_expr(child_scope(env, e, i), (*(Expr*)e.children.data[i])) })))));  }
Env child_scope(Env env, Expr e, long i) { return ((((e.kind == EMatch) && (i >= 2)) && ((i % 2) == 0)) ? arm_scope(env, (*(Expr*)e.children.data[0]), (*(Expr*)e.children.data[(i - 1)])) : (((e.kind == EMatch) && (i >= 1)) ? arm_scope(env, (*(Expr*)e.children.data[0]), (*(Expr*)e.children.data[i])) : ((((e.kind == EMethod) && (i > 0)) && ((*(Expr*)e.children.data[i]).kind == ELambda)) ? lambda_scope(env, e, i) : env)));  }
Env arm_scope(Env env, Expr scrut, Expr pat) { Typed seen = type_in(env, scrut); return bound_cells(bound_whole(bound_arm(seen.env, pat), pat, seen.ty), pat, element_of(resolve(seen.env.infer, seen.ty)));  }
Env bound_whole(Env env, Expr pat, Ty t) { return ((pat.kind == EGuard) ? bound_whole(env, (*(Expr*)pat.children.data[0]), t) : (((pat.kind != EIdent) || ((pat.children.len) > 0)) ? env : ((starts_upper(pat.text) || (!whole_binder(pat.text))) ? env : bind_mc(env, pat.text, t))));  }
long whole_binder(const char* name) { return ((((((strcmp(name, "_") != 0) && (strcmp(name, "[]") != 0)) && (strcmp(name, "[..]") != 0)) && (strcmp(name, "{}") != 0)) && (strcmp(name, "true") != 0)) && (strcmp(name, "false") != 0));  }
Env lambda_scope(Env env, Expr e, long i) { MacaList ps = lambda_params((*(Expr*)e.children.data[i])); Typed recv = type_in(env, (*(Expr*)e.children.data[0])); Ty el = element_of(resolve(recv.env.infer, recv.ty)); return (((i == 2) && ((ps.len) == 2)) ? bind_mc(bind_mc(env, (*(Expr*)ps.data[0]).text, type_in(env, (*(Expr*)e.children.data[1])).ty), (*(Expr*)ps.data[1]).text, el) : bind_each(env, ps, el, 0));  }
Env bind_each(Env env, MacaList ps, Ty t, long i) { return ((i >= (ps.len)) ? env : bind_each(bind_mc(env, (*(Expr*)ps.data[i]).text, t), ps, t, (i + 1)));  }
const char* type_of(Expr e) { return show_ty(type_in(empty_env(), e).ty);  }
long count_errors(Expr e) { return (type_in(empty_env(), e).env.errors.len);  }
Module lifted(Module m) { Module named = named_anons(m); Lift got = lift_items(named.items, 0, (Lift){ .items = maca_listv(0), .n = 0 }); return (Module){ .items = got.items, .errors = m.errors };  }
MacaList top_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : ((((*(Stmt*)items.data[i]).kind == SFn) || ((*(Stmt*)items.data[i]).kind == SBind)) ? top_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : top_names(items, (i + 1), acc)));  }
Module desugared(Module m, const char* src) { Assets at = (Assets){ .base = embed_dir(src), .names = text_const_names(m.items, 0, maca_listv(0)), .texts = text_const_texts(m.items, 0, maca_listv(0)) }; return ((maca_list_index_of_str(top_names(m.items, 0, maca_listv(0)), "data") >= 0) ? (Module){ .items = m.items, .errors = maca_listv(0) } : (Module){ .items = embed_items(m.items, at, 0, maca_listv(0)), .errors = embed_faults(m.items, at, 0, maca_listv(0)) });  }
const char* embed_dir(const char* path) { return maca_str_slice(path, 0, embed_sep(maca_chars(path), (((int)strlen(path)) - 1)));  }
long embed_sep(MacaList cs, long i) { return ((i < 0) ? 0 : (((strcmp(((const char*)cs.data[i]), "/") == 0) || (strcmp(((const char*)cs.data[i]), "\\") == 0)) ? (i + 1) : embed_sep(cs, (i - 1))));  }
long is_text_const(Stmt s) { return ((s.kind == SBind) && (s.value.kind == EStr));  }
MacaList text_const_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (is_text_const((*(Stmt*)items.data[i])) ? text_const_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : text_const_names(items, (i + 1), acc)));  }
MacaList text_const_texts(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (is_text_const((*(Stmt*)items.data[i])) ? text_const_texts(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).value.text)))) : text_const_texts(items, (i + 1), acc)));  }
long is_data_call(Expr e) { return ((e.kind == ECall) && (strcmp(e.text, "data") == 0));  }
const char* asset_spec(Expr e, Assets at) { return ((e.kind == EStr) ? e.text : (((e.kind == EIdent) && (maca_list_index_of_str(at.names, e.text) >= 0)) ? ((const char*)at.texts.data[maca_list_index_of_str(at.names, e.text)]) : ""));  }
const char* data_file(const char* base, const char* spec) { const char* local = maca_cat(base, local_spec(spec)); return (maca_file_exists(local) ? local : maca_cat(base, spec));  }
const char* local_spec(const char* spec) { long cut = embed_sep(maca_chars(spec), (((int)strlen(spec)) - 1)); const char* tail = maca_str_slice(spec, cut, ((int)strlen(spec))); long dot = last_dot(maca_chars(tail), (((int)strlen(tail)) - 1)); return ((dot > 0) ? maca_cat_own(maca_cat_own(maca_str_slice(spec, 0, (cut + dot)), ".local", 1), maca_str_slice(tail, dot, ((int)strlen(tail))), 3) : maca_cat(spec, ".local"));  }
long last_dot(MacaList cs, long i) { return ((i < 0) ? (-1) : ((strcmp(((const char*)cs.data[i]), ".") == 0) ? i : last_dot(cs, (i - 1))));  }
const char* asset_file(Expr e, Assets at) { return data_file(at.base, asset_spec((*(Expr*)e.children.data[0]), at));  }
long asset_ok(Expr e, Assets at) { return ((((e.children.len) == 1) && (strcmp(asset_spec((*(Expr*)e.children.data[0]), at), "") != 0)) && maca_file_exists(asset_file(e, at)));  }
const char* asset_text(Expr e, Assets at) { return maca_list_join(({ MacaList _m = maca_chars(one_line_text(maca_read_file(asset_file(e, at)))); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(embed_char(((const char*)_m.data[_i]))); _r; }), "");  }
const char* one_line_text(const char* text) { return maca_replace(maca_replace(text, "\r\n", "\n"), "\r", "\n");  }
const char* embed_char(const char* c) { return (((strcmp(c, "{") == 0) || (strcmp(c, "}") == 0)) ? c : raw_char(c));  }
MacaList embed_items(MacaList items, Assets at, long i, MacaList acc) { return ((i >= (items.len)) ? acc : embed_items(items, at, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ embed_stmt((*(Stmt*)items.data[i]), at) })))));  }
Stmt embed_stmt(Stmt s, Assets at) { return ({ __typeof__(s) _w = s; _w.value = embed_bound(s.value, s.ret, at); _w.body = embed_items(s.body, at, 0, maca_listv(0)); _w; });  }
Expr embed_bound(Expr e, const char* want, Assets at) { return (((is_data_call(e) && asset_ok(e, at)) && (strcmp(want, "str") == 0)) ? e_str(asset_text(e, at)) : embed_expr(e, at));  }
Expr embed_expr(Expr e, Assets at) { return ((is_data_call(e) && asset_ok(e, at)) ? e_call("decode", maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_str(asset_text(e, at)) }))) : ({ __typeof__(e) _w = e; _w.children = embed_kids(e.children, at, 0, maca_listv(0)); _w.stmts = embed_items(e.stmts, at, 0, maca_listv(0)); _w; }));  }
MacaList embed_kids(MacaList xs, Assets at, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : embed_kids(xs, at, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ embed_expr((*(Expr*)xs.data[i]), at) })))));  }
MacaList embed_faults(MacaList items, Assets at, long i, MacaList acc) { return ((i >= (items.len)) ? acc : embed_faults(items, at, (i + 1), maca_list_cat(maca_list_cat(acc, expr_faults((*(Stmt*)items.data[i]).value, at)), embed_faults((*(Stmt*)items.data[i]).body, at, 0, maca_listv(0)))));  }
MacaList expr_faults(Expr e, Assets at) { return (is_data_call(e) ? data_faults(e, at) : maca_list_cat(kid_faults(e.children, at, 0, maca_listv(0)), embed_faults(e.stmts, at, 0, maca_listv(0))));  }
MacaList kid_faults(MacaList xs, Assets at, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : kid_faults(xs, at, (i + 1), maca_list_cat(acc, expr_faults((*(Expr*)xs.data[i]), at))));  }
MacaList data_faults(Expr e, Assets at) { return (((e.children.len) != 1) ? maca_listv(1, (long)("data(...) takes one path, as in `data(\"config/links.json\")`")) : ((strcmp(asset_spec((*(Expr*)e.children.data[0]), at), "") == 0) ? maca_listv(1, (long)(maca_cat("data(...): the path is read while building, so write it out", " or bind it to a constant"))) : ((!maca_file_exists(asset_file(e, at))) ? maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("data(\"", asset_spec((*(Expr*)e.children.data[0]), at)), "\"): ", 1), asset_file(e, at), 1), ": no such file", 1))) : maca_listv(0))));  }
Lift lift_items(MacaList items, long i, Lift at) { return ((i >= (items.len)) ? at : ({ LiftedBody one = lift_stmt((*(Stmt*)items.data[i]), at); lift_items(items, (i + 1), ({ __typeof__(one.at) _w = one.at; _w.items = maca_list_cat(one.at.items, one.stmts); _w; })); }));  }
LiftedBody lift_stmt(Stmt s, Lift at) { return ((s.kind == SFn) ? ({ LiftedBody body = lift_body(s.body, 0, maca_listv(0), at); (LiftedBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ({ __typeof__(s) _w = s; _w.body = body.stmts; _w; }) })), .at = body.at }; }) : ((((s.kind == SRecord) || (s.kind == SSum)) || is_impl_block(s)) ? (LiftedBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s })), .at = at } : ({ Lifted got = lift_expr(s.value, at); (LiftedBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ({ __typeof__(s) _w = s; _w.value = got.node; _w; }) })), .at = got.at }; })));  }
LiftedBody lift_body(MacaList stmts, long i, MacaList acc, Lift at) { return ((i >= (stmts.len)) ? (LiftedBody){ .stmts = acc, .at = at } : ({ LiftedBody one = lift_stmt((*(Stmt*)stmts.data[i]), at); lift_body(stmts, (i + 1), maca_list_cat(acc, one.stmts), one.at); }));  }
Lifted lift_expr(Expr e, Lift at) { return (((e.kind == ELambda) && ((captures_of(e).len) == 0)) ? lift_one(e, at) : ((e.kind == ELambda) ? lift_closure(e, at) : ({ LiftedAll kids = lift_all(e.children, 0, maca_listv(0), at, e); LiftedBody body = lift_body(e.stmts, 0, maca_listv(0), kids.at); (Lifted){ .node = ({ __typeof__(e) _w = e; _w.children = kids.nodes; _w.stmts = body.stmts; _w; }), .at = body.at }; })));  }
LiftedAll lift_all(MacaList cs, long i, MacaList acc, Lift at, Expr owner) { return ((i >= (cs.len)) ? (LiftedAll){ .nodes = acc, .at = at } : (held_inline(owner, (*(Expr*)cs.data[i])) ? lift_all(cs, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ (*(Expr*)cs.data[i]) }))), at, owner) : ({ Lifted one = lift_expr((*(Expr*)cs.data[i]), at); lift_all(cs, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ one.node }))), one.at, owner); })));  }
long held_inline(Expr owner, Expr kid) { return ((kid.kind == ELambda) && (((owner.kind == EAttr) || is_attribute_call(owner)) || ((owner.kind == EMethod) && inlines_lambda(owner.text))));  }
long is_attribute_call(Expr owner) { return ((owner.kind == ECall) && (((strcmp(owner.text, "str") == 0) || (strcmp(owner.text, "maca_attr") == 0)) || (strcmp(owner.text, "maca_flag") == 0)));  }
long inlines_lambda(const char* name) { return (((((strcmp(name, "map") == 0) || (strcmp(name, "filter") == 0)) || (strcmp(name, "reduce") == 0)) || (strcmp(name, "fold") == 0)) || (strcmp(name, "parallel") == 0));  }
Lifted lift_one(Expr e, Lift at) { const char* name = maca_cat_own("maca_lambda_", maca_int_to_str(at.n), 2); Stmt made = s_fn(name, lambda_body(e).ty, lambda_params(e), maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(lambda_body(e)) }))); LiftedBody inner = lift_stmt(made, ({ __typeof__(at) _w = at; _w.n = (at.n + 1); _w; })); return (Lifted){ .node = e_ident(name), .at = ({ __typeof__(inner.at) _w = inner.at; _w.items = maca_list_cat(inner.at.items, inner.stmts); _w; }) };  }
Lifted lift_closure(Expr e, Lift at) { const char* name = maca_cat_own("maca_closure_", maca_int_to_str(at.n), 2); MacaList caps = captures_of(e); Stmt made = s_fn(name, lambda_body(e).ty, lambda_params(e), maca_list_cat(cap_binds(caps, 0), maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(lambda_body(e)) })))); LiftedBody inner = lift_stmt(made, ({ __typeof__(at) _w = at; _w.n = (at.n + 1); _w; })); return (Lifted){ .node = e_call("maca_closure", maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_ident(name) })), caps)), .at = ({ __typeof__(inner.at) _w = inner.at; _w.items = maca_list_cat(inner.at.items, inner.stmts); _w; }) };  }
MacaList cap_binds(MacaList caps, long i) { return ((i >= (caps.len)) ? maca_listv(0) : ({ Stmt one = s_bind_typed((*(Expr*)caps.data[i]).text, (*(Expr*)caps.data[i]).ty, e_call("maca_cap", maca_listv(2, maca_box(sizeof(Expr), (Expr[]){ e_str((*(Expr*)caps.data[i]).ty) }), maca_box(sizeof(Expr), (Expr[]){ e_int(i) })))); maca_list_cat(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ one })), cap_binds(caps, (i + 1))); }));  }
MacaList captures_of(Expr e) { return free_idents(lambda_body(e), lambda_names(lambda_params(e), 0, maca_listv(0)), maca_listv(0));  }
MacaList lambda_names(MacaList ps, long i, MacaList acc) { return ((i >= (ps.len)) ? acc : lambda_names(ps, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Expr*)ps.data[i]).text)))));  }
MacaList free_idents(Expr e, MacaList bound, MacaList acc) { return ((e.kind == EIdent) ? free_added(e, bound, acc) : ((e.kind == ELambda) ? free_idents(lambda_body(e), maca_list_cat(bound, lambda_names(lambda_params(e), 0, maca_listv(0))), acc) : ((e.kind == EBlock) ? free_idents((*(Expr*)e.children.data[0]), maca_list_cat(bound, stmt_names(e.stmts, 0, maca_listv(0))), free_stmts(e.stmts, 0, bound, acc)) : free_stmts(e.stmts, 0, free_bound(e, bound), free_kids(e.children, 0, bound, acc)))));  }
MacaList stmt_names(MacaList ss, long i, MacaList acc) { return ((i >= (ss.len)) ? acc : stmt_names(ss, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)ss.data[i]).name)))));  }
MacaList free_bound(Expr e, MacaList bound) { return ((e.kind == EFor) ? maca_list_cat(bound, maca_listv(1, (long)(e.text))) : bound);  }
MacaList free_added(Expr e, MacaList bound, MacaList acc) { return (((!is_bound_name(e)) || (maca_list_index_of_str(bound, e.text) >= 0)) ? acc : ((maca_list_index_of_str(free_names(acc, 0, maca_listv(0)), e.text) >= 0) ? acc : maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e })))));  }
MacaList free_names(MacaList acc, long i, MacaList out) { return ((i >= (acc.len)) ? out : free_names(acc, (i + 1), maca_list_cat(out, maca_listv(1, (long)((*(Expr*)acc.data[i]).text)))));  }
MacaList free_kids(MacaList cs, long i, MacaList bound, MacaList acc) { return ((i >= (cs.len)) ? acc : free_kids(cs, (i + 1), bound, free_idents((*(Expr*)cs.data[i]), bound, acc)));  }
MacaList free_stmts(MacaList ss, long i, MacaList bound, MacaList acc) { return ((i >= (ss.len)) ? acc : free_stmts(ss, (i + 1), maca_list_cat(bound, maca_listv(1, (long)((*(Stmt*)ss.data[i]).name))), free_idents((*(Stmt*)ss.data[i]).value, bound, acc)));  }
Module named_anons(Module m) { MacaList fixed = anon_stmts(m.items, 0); return (Module){ .items = maca_list_cat(fixed, anon_decls(anon_in_stmts(fixed, 0), 0, maca_listv(0))), .errors = m.errors };  }
MacaList anon_stmts(MacaList ss, long i) { return ((i >= (ss.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ anon_stmt((*(Stmt*)ss.data[i])) })), anon_stmts(ss, (i + 1))));  }
Stmt anon_stmt(Stmt s) { return (is_impl_block(s) ? ({ __typeof__(s) _w = s; _w.value = anon_methods(s.value); _w; }) : ({ __typeof__(s) _w = s; _w.value = anon_expr(s.value); _w.body = anon_stmts(s.body, 0); _w; }));  }
Expr anon_methods(Expr e) { return ({ __typeof__(e) _w = e; _w.children = anon_kids(e.children, 0); _w; });  }
Expr anon_expr(Expr e) { return ({ __typeof__(e) _w = e; _w.text = anon_text(e); _w.stmts = anon_stmts(e.stmts, 0); _w.children = anon_kids(e.children, 0); _w; });  }
MacaList anon_kids(MacaList cs, long i) { return ((i >= (cs.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ anon_expr((*(Expr*)cs.data[i])) })), anon_kids(cs, (i + 1))));  }
const char* anon_text(Expr e) { return ((((e.kind != ERecord) || (strcmp(e.text, "") != 0)) || ((e.children.len) == 0)) ? e.text : ((!anon_named_fields(e.children, 0)) ? e.text : maca_cat_own("MacaAnon_", maca_list_join(maca_list_sorted(anon_tags(e.children, 0, maca_listv(0)), 1), "_"), 2)));  }
long anon_named_fields(MacaList fs, long i) { return ((i >= (fs.len)) ? 1 : ((!anon_field_named((*(Expr*)fs.data[i]))) ? 0 : anon_named_fields(fs, (i + 1))));  }
long anon_field_named(Expr f) { return ((((f.kind != EBinary) || (strcmp(f.text, "=") != 0)) || ((f.children.len) < 2)) ? 0 : anon_word((*(Expr*)f.children.data[0]).text));  }
long anon_word(const char* name) { return ((strcmp(name, "") != 0) && ((isalpha((unsigned char)(maca_str_at(name, 0))[0]) != 0) || (strcmp(maca_str_at(name, 0), "_") == 0)));  }
MacaList anon_tags(MacaList fs, long i, MacaList acc) { return ((i >= (fs.len)) ? acc : anon_tags(fs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(anon_tag((*(Expr*)fs.data[i])))))));  }
const char* anon_tag(Expr f) { return maca_cat_own(maca_cat_own(maca_cat("", (*(Expr*)f.children.data[0]).text), "_", 1), type_tag((*(Expr*)f.children.data[1]).ty), 1);  }
long is_anon_lit(Expr e) { return ((e.kind == ERecord) && (maca_str_index_of(e.text, "MacaAnon_") == 0));  }
MacaList anon_in_stmts(MacaList ss, long i) { return ((i >= (ss.len)) ? maca_listv(0) : maca_list_cat(anon_in_stmt((*(Stmt*)ss.data[i])), anon_in_stmts(ss, (i + 1))));  }
MacaList anon_in_stmt(Stmt s) { return maca_list_cat(anon_in_expr(s.value), anon_in_stmts(s.body, 0));  }
MacaList anon_in_expr(Expr e) { MacaList under = maca_list_cat(anon_in_kids(e.children, 0), anon_in_stmts(e.stmts, 0)); return (is_anon_lit(e) ? maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e })), under) : under);  }
MacaList anon_in_kids(MacaList cs, long i) { return ((i >= (cs.len)) ? maca_listv(0) : maca_list_cat(anon_in_expr((*(Expr*)cs.data[i])), anon_in_kids(cs, (i + 1))));  }
MacaList anon_decls(MacaList es, long i, MacaList seen) { return ((i >= (es.len)) ? maca_listv(0) : ((maca_list_index_of_str(seen, (*(Expr*)es.data[i]).text) >= 0) ? anon_decls(es, (i + 1), seen) : maca_list_cat(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_record((*(Expr*)es.data[i]).text, anon_fields((*(Expr*)es.data[i]).children, 0)) })), anon_decls(es, (i + 1), maca_list_cat(seen, maca_listv(1, (long)((*(Expr*)es.data[i]).text)))))));  }
MacaList anon_fields(MacaList fs, long i) { return ((i >= (fs.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ e_param((*(Expr*)(*(Expr*)fs.data[i]).children.data[0]).text, (*(Expr*)(*(Expr*)fs.data[i]).children.data[1]).ty) })), anon_fields(fs, (i + 1))));  }
Module monomorphic(Module m) { MacaList gens = generic_items(m.items, 0, maca_listv(0)); return (((gens.len) == 0) ? m : ({ PolyBody first = mono_items(m.items, 0, maca_listv(0), (Poly){ .gens = gens, .wants = maca_listv(0), .done = maca_listv(0) }); PolyBody grown = expanded(checked_module(m), first.at, 0, maca_listv(0)); (Module){ .items = maca_list_cat(first.stmts, grown.stmts), .errors = m.errors }; }));  }
MacaList generic_items(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (is_generic_fn((*(Stmt*)items.data[i])) ? generic_items(items, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ (*(Stmt*)items.data[i]) })))) : generic_items(items, (i + 1), acc)));  }
long is_generic_fn(Stmt s) { return (((s.kind == SFn) && ((s.body.len) > 0)) && (has_type_var(s.ret) || any_type_var(s.params, 0)));  }
long any_type_var(MacaList ps, long i) { return ((i >= (ps.len)) ? 0 : (has_type_var((*(Expr*)ps.data[i]).ty) ? 1 : any_type_var(ps, (i + 1))));  }
long has_type_var(const char* name) { long cut = maca_str_index_of(name, ") -> "); return (is_array_name(name) ? has_type_var(maca_str_slice(name, 0, (((int)strlen(name)) - 2))) : ((cut >= 0) ? (var_in_each(maca_str_slice(name, 1, cut)) || has_type_var(maca_str_slice(name, (cut + 5), ((int)strlen(name))))) : ((strcmp(map_type_val(name), "") != 0) ? (has_type_var(map_type_key(name)) || has_type_var(map_type_val(name))) : ((strcmp(name, "") != 0) && (ty_named(name).kind == KAny)))));  }
long var_in_each(const char* list) { long cut = maca_str_index_of(list, ", "); return ((strcmp(list, "") == 0) ? 0 : ((cut < 0) ? has_type_var(list) : (has_type_var(maca_str_slice(list, 0, cut)) || var_in_each(maca_str_slice(list, (cut + 2), ((int)strlen(list)))))));  }
PolyBody mono_items(MacaList items, long i, MacaList acc, Poly at) { return ((i >= (items.len)) ? (PolyBody){ .stmts = acc, .at = at } : (is_generic_fn((*(Stmt*)items.data[i])) ? mono_items(items, (i + 1), acc, at) : ({ PolyBody one = mono_stmt((*(Stmt*)items.data[i]), at); mono_items(items, (i + 1), maca_list_cat(acc, one.stmts), one.at); })));  }
PolyBody mono_stmt(Stmt s, Poly at) { return (((s.kind == SRecord) || (s.kind == SSum)) ? (PolyBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s })), .at = at } : ((s.kind == SFn) ? ({ PolyBody body = mono_body(s.body, 0, maca_listv(0), at); (PolyBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ({ __typeof__(s) _w = s; _w.body = body.stmts; _w; }) })), .at = body.at }; }) : ({ PolyNode got = mono_expr(s.value, at); (PolyBody){ .stmts = maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ({ __typeof__(s) _w = s; _w.value = got.node; _w; }) })), .at = got.at }; })));  }
PolyBody mono_body(MacaList stmts, long i, MacaList acc, Poly at) { return ((i >= (stmts.len)) ? (PolyBody){ .stmts = acc, .at = at } : ({ PolyBody one = mono_stmt((*(Stmt*)stmts.data[i]), at); mono_body(stmts, (i + 1), maca_list_cat(acc, one.stmts), one.at); }));  }
PolyNode mono_expr(Expr e, Poly at) { PolyAll kids = mono_all(e.children, 0, maca_listv(0), at); PolyBody body = mono_body(e.stmts, 0, maca_listv(0), kids.at); Expr node = ({ __typeof__(e) _w = e; _w.children = kids.nodes; _w.stmts = body.stmts; _w; }); return ((e.kind != ECall) ? (PolyNode){ .node = node, .at = body.at } : mono_named(node, body.at));  }
PolyAll mono_all(MacaList cs, long i, MacaList acc, Poly at) { return ((i >= (cs.len)) ? (PolyAll){ .nodes = acc, .at = at } : ({ PolyNode one = mono_expr((*(Expr*)cs.data[i]), at); mono_all(cs, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ one.node }))), one.at); }));  }
PolyNode mono_named(Expr e, Poly at) { long found = generic_at(at.gens, e.text, 0); return ((found >= (at.gens.len)) ? (PolyNode){ .node = e, .at = at } : mono_call(e, (*(Stmt*)at.gens.data[found]), at));  }
PolyNode mono_call(Expr e, Stmt g, Poly at) { Want want = (Want){ .name = e.text, .tys = arg_type_names(e.children, 0, maca_listv(0)) }; return (PolyNode){ .node = ({ __typeof__(e) _w = e; _w.text = mangled(want); _w.ty = specialised_ty(g, want.tys, g.ret); _w; }), .at = ({ __typeof__(at) _w = at; _w.wants = maca_list_cat(at.wants, maca_listv(1, maca_box(sizeof(Want), (Want[]){ want }))); _w; }) };  }
long generic_at(MacaList gens, const char* name, long i) { return (((i >= (gens.len)) || (strcmp((*(Stmt*)gens.data[i]).name, name) == 0)) ? i : generic_at(gens, name, (i + 1)));  }
MacaList arg_type_names(MacaList cs, long i, MacaList acc) { return ((i >= (cs.len)) ? acc : arg_type_names(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Expr*)cs.data[i]).ty)))));  }
const char* mangled(Want w) { const char* tags = maca_list_join(({ MacaList _m = w.tys; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(type_tag(((const char*)_m.data[_i]))); _r; }), "_"); return maca_cat_own(maca_cat_own(maca_cat("", w.name), "__", 1), tags, 1);  }
const char* type_tag(const char* ty) { return ((strcmp(ty, "") == 0) ? "any" : (((((int)strlen(ty)) >= 2) && (strcmp(maca_str_slice(ty, (((int)strlen(ty)) - 2), ((int)strlen(ty))), "[]") == 0)) ? maca_cat("arr", type_tag(maca_str_slice(ty, 0, (((int)strlen(ty)) - 2)))) : ((strcmp(map_type_val(ty), "") != 0) ? maca_cat(type_tag(map_type_val(ty)), "map") : ((maca_str_index_of(ty, ") -> ") >= 0) ? maca_cat("fn", type_tag(maca_str_slice(ty, (maca_str_index_of(ty, ") -> ") + 5), ((int)strlen(ty))))) : ((strcmp(ty, "float") == 0) ? "f64" : ty)))));  }
PolyBody expanded(Env env, Poly at, long i, MacaList acc) { return ((i >= (at.wants.len)) ? (PolyBody){ .stmts = acc, .at = at } : ({ Want w = (*(Want*)at.wants.data[i]); const char* name = mangled(w); ((maca_list_index_of_str(at.done, name) >= 0) ? expanded(env, at, (i + 1), acc) : ({ Stmt g = (*(Stmt*)at.gens.data[generic_at(at.gens, w.name, 0)]); PolyBody one = mono_stmt(annotate_item(env, specialised(g, w, name)), ({ __typeof__(at) _w = at; _w.done = maca_list_cat(at.done, maca_listv(1, (long)(name))); _w; })); expanded(env, one.at, (i + 1), maca_list_cat(acc, one.stmts)); })); }));  }
Stmt specialised(Stmt g, Want w, const char* name) { return ({ __typeof__(g) _w = g; _w.name = name; _w.ret = specialised_ty(g, w.tys, g.ret); _w.params = specialised_params(g, g.params, w.tys, 0); _w.body = ground_body(g, w.tys, g.body, 0); _w; });  }
MacaList ground_body(Stmt g, MacaList tys, MacaList ss, long i) { return ((i >= (ss.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ ground_stmt(g, tys, (*(Stmt*)ss.data[i])) })), ground_body(g, tys, ss, (i + 1))));  }
Stmt ground_stmt(Stmt g, MacaList tys, Stmt s) { return ({ __typeof__(s) _w = s; _w.ret = specialised_ty(g, tys, s.ret); _w.value = ground_expr(g, tys, s.value); _w.body = ground_body(g, tys, s.body, 0); _w; });  }
Expr ground_expr(Stmt g, MacaList tys, Expr e) { return ({ __typeof__(e) _w = e; _w.stmts = ground_body(g, tys, e.stmts, 0); _w.children = ground_kids(g, tys, e.children, 0); _w; });  }
MacaList ground_kids(Stmt g, MacaList tys, MacaList cs, long i) { return ((i >= (cs.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ ground_expr(g, tys, (*(Expr*)cs.data[i])) })), ground_kids(g, tys, cs, (i + 1))));  }
MacaList specialised_params(Stmt g, MacaList ps, MacaList tys, long i) { return ((i >= (ps.len)) ? maca_listv(0) : maca_list_cat(maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ specialised_param(g, (*(Expr*)ps.data[i]), tys, i) })), specialised_params(g, ps, tys, (i + 1))));  }
Expr specialised_param(Stmt g, Expr p, MacaList tys, long i) { return ((i >= (tys.len)) ? p : ((strcmp(((const char*)tys.data[i]), "") == 0) ? ({ __typeof__(p) _w = p; _w.ty = specialised_ty(g, tys, p.ty); _w; }) : ({ __typeof__(p) _w = p; _w.ty = ((const char*)tys.data[i]); _w; })));  }
const char* specialised_ty(Stmt g, MacaList tys, const char* ty) { return (is_array_name(ty) ? maca_cat(specialised_ty(g, tys, maca_str_slice(ty, 0, (((int)strlen(ty)) - 2))), "[]") : ((strcmp(map_type_val(ty), "") != 0) ? maca_cat_own(maca_cat("Map ", specialised_ty(g, tys, map_type_key(ty))), maca_cat(" ", specialised_ty(g, tys, map_type_val(ty))), 3) : ((maca_str_index_of(ty, ") -> ") >= 0) ? specialised_fn(g, tys, ty) : ((!has_type_var(ty)) ? ty : var_type(g.params, tys, 0, ty)))));  }
const char* specialised_fn(Stmt g, MacaList tys, const char* ty) { long cut = maca_str_index_of(ty, ") -> "); return maca_cat_own(maca_cat_own(maca_cat("(", specialised_each(g, tys, maca_str_slice(ty, 1, cut))), ") -> ", 1), specialised_ty(g, tys, maca_str_slice(ty, (cut + 5), ((int)strlen(ty)))), 1);  }
const char* specialised_each(Stmt g, MacaList tys, const char* list) { long cut = maca_str_index_of(list, ", "); return ((strcmp(list, "") == 0) ? "" : ((cut < 0) ? specialised_ty(g, tys, list) : maca_cat_own(maca_cat_own(maca_cat("", specialised_ty(g, tys, maca_str_slice(list, 0, cut))), ", ", 1), specialised_each(g, tys, maca_str_slice(list, (cut + 2), ((int)strlen(list)))), 1)));  }
const char* var_type(MacaList ps, MacaList tys, long i, const char* v) { return (((i >= (ps.len)) || (i >= (tys.len))) ? "" : ({ const char* here = matched_var((*(Expr*)ps.data[i]).ty, ((const char*)tys.data[i]), v); ((strcmp(here, "") != 0) ? here : var_type(ps, tys, (i + 1), v)); }));  }
const char* matched_var(const char* declared, const char* concrete, const char* v) { return ((strcmp(declared, v) == 0) ? concrete : ((is_array_name(declared) && is_array_name(concrete)) ? matched_var(maca_str_slice(declared, 0, (((int)strlen(declared)) - 2)), maca_str_slice(concrete, 0, (((int)strlen(concrete)) - 2)), v) : ""));  }
MacaList variadic_errors(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : variadic_errors(items, (i + 1), maca_list_cat(acc, rest_misuse((*(Stmt*)items.data[i])))));  }
MacaList rest_misuse(Stmt s) { const char* why = rest_misuse_why(s); return ((strcmp(why, "") == 0) ? maca_listv(0) : maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ diag_at(s, "M0001", why) })));  }
const char* rest_misuse_why(Stmt s) { return ((s.kind != SFn) ? "" : (rest_before_the_end(s.params, 0) ? maca_cat_own(maca_cat_own(maca_cat("type mismatch: `", s.name), "`: a rest parameter must be last, so", 1), " nothing says where the arguments it gathers stop", 1) : (((rest_taker(s).len) == 0) ? "" : ((strcmp(s.name, "main") == 0) ? maca_cat("type mismatch: `main` takes no rest parameter, because the", " command line fills one `str[]`") : ((strcmp((*(Expr*)s.params.data[((s.params.len) - 1)]).ty, "[]") == 0) ? maca_cat_own(maca_cat_own(maca_cat("type mismatch: `", s.name), "` has a rest parameter with no type, so", 1), " nothing says what the list it gathers holds", 1) : "")))));  }
MacaList effect_list() { return eff_words(AllEffects);  }
const char* eff_none() { return " ";  }
MacaList eff_words(const char* set) { return ((strcmp(maca_trim(set), "") == 0) ? maca_listv(0) : maca_split(maca_trim(set), " "));  }
long eff_has(const char* set, const char* name) { return (maca_str_index_of(set, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0);  }
const char* eff_add(const char* set, const char* name) { return (eff_has(set, name) ? set : maca_cat_own(set, maca_cat_own(maca_cat("", name), " ", 1), 2));  }
const char* eff_union(const char* a, const char* b) { return eff_merge(a, eff_words(b), 0);  }
const char* eff_merge(const char* a, MacaList xs, long i) { return ((i >= (xs.len)) ? a : eff_merge(eff_add(a, ((const char*)xs.data[i])), xs, (i + 1)));  }
long is_known_target(const char* name) { return (maca_str_index_of(KnownTargets, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0);  }
MacaList target_effects(const char* target) { return ((strcmp(target, "js") == 0) ? maca_listv(4, (long)("io"), (long)("net"), (long)("async"), (long)("exn")) : (((strcmp(target, "embedded") == 0) || (strcmp(target, "all") == 0)) ? maca_listv(1, (long)("exn")) : ((strcmp(target, "nix") == 0) ? eff_words("") : effect_list())));  }
MacaList eff_outside(const char* set, MacaList allowed) { return eff_refused(effect_list(), set, allowed, 0, maca_listv(0));  }
MacaList eff_refused(MacaList all, const char* set, MacaList allowed, long i, MacaList acc) { return ((i >= (all.len)) ? acc : ((eff_has(set, ((const char*)all.data[i])) && (maca_list_index_of_str(allowed, ((const char*)all.data[i])) < 0)) ? eff_refused(all, set, allowed, (i + 1), maca_list_pushed(acc, (long)(((const char*)all.data[i])))) : eff_refused(all, set, allowed, (i + 1), acc)));  }
const char* eff_of_call(const char* name) { return ((strcmp(name, "sleep_ms") == 0) ? " async " : ((is_io_builtin(name) || (strcmp(name, "input") == 0)) ? " io " : eff_none()));  }
const char* eff_of_word(const char* word) { return (is_async_effect(word) ? " async " : (((strcmp(word, "try") == 0) || (strcmp(word, "fail") == 0)) ? " exn " : eff_none()));  }
const char* eff_of_method(Expr e) { long named = (((e.children.len) > 0) && ((*(Expr*)e.children.data[0]).kind == EIdent)); const char* host = (named ? (*(Expr*)e.children.data[0]).text : ""); return ((((strcmp(host, "net") == 0) || (strcmp(host, "http") == 0)) || (strcmp(host, "socket") == 0)) ? " net " : (((strcmp(host, "os") == 0) || (strcmp(host, "process") == 0)) ? " os " : (is_io_method(e.text) ? " io " : eff_none())));  }
long is_io_method(const char* name) { return ((((((strcmp(name, "read") == 0) || (strcmp(name, "write") == 0)) || (strcmp(name, "exists") == 0)) || (strcmp(name, "remove") == 0)) || (strcmp(name, "append") == 0)) || (strcmp(name, "create") == 0));  }
const char* node_effects(Expr e, MacaList names, MacaList sets) { long at = ((e.kind == ECall) ? maca_list_index_of_str(names, e.text) : (-1)); return ((at >= 0) ? ((const char*)sets.data[at]) : ((e.kind == ECall) ? eff_of_call(e.text) : ((e.kind == EUnary) ? eff_of_word(e.text) : ((e.kind == EMethod) ? eff_of_method(e) : eff_none()))));  }
const char* expr_effects(Expr e, MacaList names, MacaList sets) { return body_effects(e.stmts, names, sets, 0, kids_effects(e.children, names, sets, 0, node_effects(e, names, sets)));  }
const char* kids_effects(MacaList xs, MacaList names, MacaList sets, long i, const char* acc) { return ((i >= (xs.len)) ? acc : kids_effects(xs, names, sets, (i + 1), eff_union(acc, expr_effects((*(Expr*)xs.data[i]), names, sets))));  }
const char* body_effects(MacaList body, MacaList names, MacaList sets, long i, const char* acc) { return ((i >= (body.len)) ? acc : body_effects(body, names, sets, (i + 1), eff_union(acc, expr_effects((*(Stmt*)body.data[i]).value, names, sets))));  }
MacaList effect_fn_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind == SFn) ? effect_fn_names(items, (i + 1), maca_list_pushed(acc, (long)((*(Stmt*)items.data[i]).name))) : effect_fn_names(items, (i + 1), acc)));  }
MacaList blank_effects(long n, long i, MacaList acc) { return ((i >= n) ? acc : blank_effects(n, (i + 1), maca_list_pushed(acc, (long)(eff_none()))));  }
MacaList settled_effects(MacaList items) { MacaList names = effect_fn_names(items, 0, maca_listv(0)); return settle_effects(items, names, blank_effects((names.len), 0, maca_listv(0)), ((names.len) + 1));  }
MacaList settle_effects(MacaList items, MacaList names, MacaList sets, long fuel) { MacaList next = effect_pass(items, names, sets, 0, maca_listv(0)); return (((fuel <= 0) || same_effects(sets, next, 0)) ? next : settle_effects(items, names, next, (fuel - 1)));  }
long same_effects(MacaList a, MacaList b, long i) { return (((i >= (a.len)) || (i >= (b.len))) ? ((a.len) == (b.len)) : ((strcmp(((const char*)a.data[i]), ((const char*)b.data[i])) != 0) ? 0 : same_effects(a, b, (i + 1))));  }
MacaList effect_pass(MacaList items, MacaList names, MacaList sets, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind != SFn) ? effect_pass(items, names, sets, (i + 1), acc) : effect_pass(items, names, sets, (i + 1), maca_list_pushed(acc, (long)(body_effects((*(Stmt*)items.data[i]).body, names, sets, 0, eff_none()))))));  }
MacaList target_errors(Module m, const char* target) { return maca_list_cat(effect_errors(m, target), kept_borrow_errors(m, target));  }
MacaList kept_borrow_notes(Module m) { return ({ MacaList _m = kept_borrow_errors(m, "rust"); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(diag_message((*(Diagnostic*)_m.data[_i]))); _r; });  }
MacaList kept_borrow_errors(Module m, const char* target) { return ((strcmp(target, "rust") != 0) ? maca_listv(0) : borrow_errors(m.items, declared_types(m.items, 0, maca_listv(0)), 0, maca_listv(0)));  }
MacaList borrow_errors(MacaList items, MacaList own, long i, MacaList acc) { return ((i >= (items.len)) ? acc : ((!is_impl_block((*(Stmt*)items.data[i]))) ? borrow_errors(items, own, (i + 1), acc) : ({ MacaList kept = kept_borrows((*(Stmt*)items.data[i]), (*(Stmt*)items.data[i]).value.children, own, 0, maca_listv(0)); borrow_errors(items, own, (i + 1), maca_list_cat(acc, kept)); })));  }
MacaList kept_borrows(Stmt s, MacaList fs, MacaList own, long i, MacaList acc) { return ((i >= (fs.len)) ? acc : ({ Expr lam = (*(Expr*)(*(Expr*)fs.data[i]).children.data[1]); MacaList ps = maca_list_slice(lam.children, 0, ((lam.children.len) - 1)); MacaList one = kept_params(s, (*(Expr*)(*(Expr*)fs.data[i]).children.data[0]).text, lam, ps, own, 0, maca_listv(0)); kept_borrows(s, fs, own, (i + 1), maca_list_cat(acc, one)); }));  }
MacaList kept_params(Stmt s, const char* method, Expr lam, MacaList ps, MacaList own, long i, MacaList acc) { return ((i >= (ps.len)) ? acc : ((foreign_type((*(Expr*)ps.data[i]).ty, own) && escaping(lambda_body(lam), (*(Expr*)ps.data[i]).text)) ? kept_params(s, method, lam, ps, own, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ diag_at(s, "M0009", kept_message((*(Expr*)ps.data[i]).text, method)) })))) : kept_params(s, method, lam, ps, own, (i + 1), acc)));  }
const char* kept_message(const char* p, const char* method) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", p), "` in method `", 1), method, 1), "` is a borrowed foreign value: it can be read", 1), " and passed on, but not returned or stored, because it belongs to", 1), " the caller", 1);  }
long escaping(Expr body, const char* name) { return (answered_by(body, name) || stored_away(body, name));  }
long answered_by(Expr body, const char* name) { return (((body.kind == EBlock) && ((body.children.len) > 0)) ? answered_by((*(Expr*)body.children.data[0]), name) : ((body.kind == EIdent) && (strcmp(body.text, name) == 0)));  }
long stored_away(Expr e, const char* name) { return (((e.kind == ERecord) && field_holds(e.children, name, 0)) ? 1 : (((e.kind == EList) && any_mentions(e.children, name, 0)) ? 1 : (any_stores(e.children, name, 0) || body_stores(e.stmts, name, 0))));  }
long field_holds(MacaList fs, const char* name, long i) { return ((i >= (fs.len)) ? 0 : (((((*(Expr*)fs.data[i]).children.len) > 1) && mentions((*(Expr*)(*(Expr*)fs.data[i]).children.data[1]), name)) ? 1 : field_holds(fs, name, (i + 1))));  }
long any_mentions(MacaList cs, const char* name, long i) { return ((i >= (cs.len)) ? 0 : (mentions((*(Expr*)cs.data[i]), name) ? 1 : any_mentions(cs, name, (i + 1))));  }
long any_stores(MacaList cs, const char* name, long i) { return ((i >= (cs.len)) ? 0 : (stored_away((*(Expr*)cs.data[i]), name) ? 1 : any_stores(cs, name, (i + 1))));  }
long body_stores(MacaList ss, const char* name, long i) { return ((i >= (ss.len)) ? 0 : (stored_away((*(Stmt*)ss.data[i]).value, name) ? 1 : body_stores(ss, name, (i + 1))));  }
long mentions(Expr e, const char* name) { return (((e.kind == EIdent) && (strcmp(e.text, name) == 0)) ? 1 : any_mentions(e.children, name, 0));  }
MacaList effect_errors(Module m, const char* target) { MacaList allowed = target_effects(target); return (((allowed.len) >= (effect_list().len)) ? maca_listv(0) : effect_refusals(m.items, settled_effects(m.items), allowed, target, 0, 0, maca_listv(0)));  }
MacaList effect_refusals(MacaList items, MacaList sets, MacaList allowed, const char* target, long i, long k, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind != SFn) ? effect_refusals(items, sets, allowed, target, (i + 1), k, acc) : ({ MacaList said = effect_refusal((*(Stmt*)items.data[i]), ((const char*)sets.data[k]), allowed, target); effect_refusals(items, sets, allowed, target, (i + 1), (k + 1), maca_list_cat(acc, said)); })));  }
MacaList effect_refusal(Stmt s, const char* set, MacaList allowed, const char* target) { MacaList over = eff_outside(set, allowed); return (((over.len) == 0) ? maca_listv(0) : maca_listv(1, maca_box(sizeof(Diagnostic), (Diagnostic[]){ effect_diag(s, maca_list_join(over, ", "), target) })));  }
Diagnostic effect_diag(Stmt s, const char* what, const char* target) { Diagnostic said = diag_at(s, "M0007", maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", s.name), "` performs ", 1), what, 1), ", which", 1), maca_cat_own(maca_cat(" ", target_phrase(target)), " cannot carry", 1), 3)); return ({ __typeof__(said) _w = said; _w.note = maca_cat_own(maca_cat_own(maca_cat("build for a target that can, or keep ", what), " out of", 1), " the code this target compiles", 1); _w; });  }
const char* target_phrase(const char* target) { return ((strcmp(target, "all") == 0) ? "every program target" : maca_cat_own(maca_cat("the `", target), "` target", 1));  }
MacaList check_diagnostics_on(Module m, const char* target) { return maca_list_cat(check_diagnostics(m), target_errors(m, target));  }
MacaList check_errors_on(Module m, const char* target) { return ({ MacaList _m = check_diagnostics_on(m, target); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(diag_message((*(Diagnostic*)_m.data[_i]))); _r; });  }
const char* tw_look(const char* table, const char* key) { long at = maca_str_index_of(table, maca_cat_own(maca_cat("|", key), "=", 1)); return ((at < 0) ? "" : ({ const char* rest = maca_str_slice(table, ((at + ((int)strlen(key))) + 2), ((int)strlen(table))); maca_str_slice(rest, 0, maca_str_index_of(rest, "|")); }));  }
long tw_starts(const char* c, const char* p) { return maca_starts_with(c, p);  }
const char* tw_after(const char* c, const char* p) { return maca_str_slice(c, ((int)strlen(p)), ((int)strlen(c)));  }
const char* tw_decl(const char* prop, const char* v) { return ((strcmp(v, "") == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", prop), ":", 1), v, 1), ";", 1));  }
const char* tw_pair_decl(const char* a, const char* b, const char* v) { return ((strcmp(v, "") == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", a), ":", 1), v, 1), ";", 1), b, 1), ":", 1), v, 1), ";", 1));  }
long tw_digits(MacaList cs, long i, long acc) { return ((i >= (cs.len)) ? acc : ((!(isdigit((unsigned char)(((const char*)cs.data[i]))[0]) != 0)) ? (-1) : tw_digits(cs, (i + 1), ((acc * 10) + atol(((const char*)cs.data[i]))))));  }
long tw_int(const char* s) { return ((strcmp(s, "") == 0) ? (-1) : tw_digits(maca_chars(s), 0, 0));  }
const char* tw_signed(const char* s) { return (maca_starts_with(s, "-") ? ({ long n = tw_int(maca_str_slice(s, 1, ((int)strlen(s)))); ((n < 0) ? "" : maca_cat_own("-", maca_int_to_str(n), 2)); }) : ({ long n = tw_int(s); ((n < 0) ? "" : maca_cat_own("", maca_int_to_str(n), 2)); }));  }
long tw_pow10(long k) { return ((k <= 0) ? 1 : (10 * tw_pow10((k - 1))));  }
long tw_thou(const char* s) { long dot = maca_str_index_of(s, "."); return ((dot < 0) ? ({ long n = tw_int(s); ((n < 0) ? (-1) : (n * 1000)); }) : tw_thou_parts(tw_int(maca_str_slice(s, 0, dot)), maca_str_slice(s, (dot + 1), ((int)strlen(s)))));  }
long tw_thou_parts(long whole, const char* frac) { long f = tw_int(frac); return ((((whole < 0) || (f < 0)) || (((int)strlen(frac)) > 3)) ? (-1) : ((whole * 1000) + (f * tw_pow10((3 - ((int)strlen(frac)))))));  }
const char* tw_zeros(const char* s) { return (maca_ends_with(s, "0") ? tw_zeros(maca_str_slice(s, 0, (((int)strlen(s)) - 1))) : s);  }
const char* tw_pad(long n, long width) { return (((width <= 1) || (n >= tw_pow10((width - 1)))) ? maca_cat_own("", maca_int_to_str(n), 2) : maca_cat("0", tw_pad(n, (width - 1))));  }
const char* tw_dec(long n) { return (((n % 1000) == 0) ? maca_cat_own("", maca_int_to_str((n / 1000)), 2) : maca_cat_own(maca_cat_own(maca_cat_own("", maca_int_to_str((n / 1000)), 2), ".", 1), tw_zeros(tw_pad((n % 1000), 3)), 1));  }
const char* tw_space(const char* v) { return ((strcmp(v, "0") == 0) ? "0" : ((strcmp(v, "px") == 0) ? "1px" : ((strcmp(v, "auto") == 0) ? "auto" : ({ long n = tw_thou(v); ((n < 0) ? "" : maca_cat_own(maca_cat("", tw_dec((n / 4))), "rem", 1)); }))));  }
const char* tw_ratio(const char* v) { long cut = maca_str_index_of(v, "/"); long a = tw_int(maca_str_slice(v, 0, cut)); long b = tw_int(maca_str_slice(v, (cut + 1), ((int)strlen(v)))); return (((a < 0) || (b <= 0)) ? "" : ({ long t = ((a * 1000000) / b); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("", maca_int_to_str((t / 10000)), 2), ".", 1), tw_pad((t % 10000), 4), 1), "%", 1); }));  }
const char* tw_size(const char* v) { return (((strcmp(v, "none") == 0) || (strcmp(v, "auto") == 0)) ? v : ((strcmp(v, "full") == 0) ? "100%" : ((strcmp(v, "screen") == 0) ? "100vh" : ((((strcmp(v, "min") == 0) || (strcmp(v, "max") == 0)) || (strcmp(v, "fit") == 0)) ? maca_cat_own(maca_cat("", v), "-content", 1) : ((maca_str_index_of(v, "/") >= 0) ? tw_ratio(v) : tw_space(v))))));  }
const char* tw_border(MacaList sides, long i, const char* w) { return ((i >= (sides.len)) ? "" : ({ const char* s = ((const char*)sides.data[i]); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("border-", s), "-width:", 1), w, 1), ";border-", 1), s, 1), "-style:solid;", 1), tw_border(sides, (i + 1), w), 1); }));  }
const char* tw_trim_dash(const char* s) { return (maca_ends_with(s, "-") ? tw_trim_dash(maca_str_slice(s, 0, (((int)strlen(s)) - 1))) : s);  }
long tw_last_dash(MacaList cs, long i, long at) { return ((i >= (cs.len)) ? at : ((strcmp(((const char*)cs.data[i]), "-") == 0) ? tw_last_dash(cs, (i + 1), i) : tw_last_dash(cs, (i + 1), at)));  }
const char* tw_arbitrary(const char* c) { long open_mc = maca_str_index_of(c, "["); return (((open_mc < 0) || (!maca_ends_with(c, "]"))) ? "" : tw_bracket(tw_trim_dash(maca_str_slice(c, 0, open_mc)), maca_replace(maca_str_slice(c, (open_mc + 1), (((int)strlen(c)) - 1)), "_", " ")));  }
const char* tw_bracket(const char* pre, const char* raw) { const char* sides = tw_look(TwEdge, pre); const char* prop = tw_look(TwProp, pre); return ((strcmp(sides, "") != 0) ? tw_border(maca_split(sides, " "), 0, raw) : ((strcmp(prop, "") != 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", prop), ":", 1), raw, 1), ";", 1) : ""));  }
const char* tw_edges(const char* c) { long at = tw_last_dash(maca_chars(c), 0, (-1)); return ((at < 0) ? "" : ({ const char* sides = tw_look(TwEdge, maca_str_slice(c, 0, at)); long n = tw_int(maca_str_slice(c, (at + 1), ((int)strlen(c)))); (((strcmp(sides, "") == 0) || (n < 0)) ? "" : tw_border(maca_split(sides, " "), 0, maca_cat_own(maca_cat_own("", maca_int_to_str(n), 2), "px", 1))); }));  }
const char* tw_offsets(const char* c) { return (tw_starts(c, "top-") ? tw_decl("top", tw_space(tw_after(c, "top-"))) : (tw_starts(c, "right-") ? tw_decl("right", tw_space(tw_after(c, "right-"))) : (tw_starts(c, "bottom-") ? tw_decl("bottom", tw_space(tw_after(c, "bottom-"))) : (tw_starts(c, "left-") ? tw_decl("left", tw_space(tw_after(c, "left-"))) : ""))));  }
const char* tw_track(const char* c) { long at = tw_last_dash(maca_chars(c), 0, (-1)); return ((at < 0) ? "" : tw_axis(maca_str_slice(c, 0, at), maca_str_slice(c, (at + 1), ((int)strlen(c)))));  }
const char* tw_axis(const char* head, const char* val) { long dash = maca_str_index_of(head, "-"); const char* axis = (maca_starts_with(head, "col") ? "column" : (maca_starts_with(head, "row") ? "row" : "")); return (((strcmp(axis, "") == 0) || (dash < 0)) ? "" : tw_place(axis, maca_str_slice(head, (dash + 1), ((int)strlen(head))), val));  }
const char* tw_place(const char* axis, const char* part, const char* val) { const char* n = tw_signed(val); long span = tw_int(val); return (((strcmp(part, "span") == 0) && (strcmp(val, "full") == 0)) ? maca_cat_own(maca_cat("grid-", axis), ":1 / -1;", 1) : (((strcmp(part, "span") == 0) && (span >= 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("grid-", axis), ":span ", 1), maca_int_to_str(span), 3), " / span ", 1), maca_int_to_str(span), 3), ";", 1) : ((strcmp(part, "span") == 0) ? "" : (((strcmp(part, "start") == 0) && (strcmp(val, "auto") == 0)) ? maca_cat_own(maca_cat("grid-", axis), "-start:auto;", 1) : (((strcmp(part, "end") == 0) && (strcmp(val, "auto") == 0)) ? maca_cat_own(maca_cat("grid-", axis), "-end:auto;", 1) : (((strcmp(part, "start") == 0) || (strcmp(part, "end") == 0)) ? tw_decl(maca_cat_own(maca_cat_own(maca_cat("grid-", axis), "-", 1), part, 1), n) : ""))))));  }
const char* tw_leading(const char* v) { const char* named = tw_look(TwLead, v); return ((strcmp(named, "") != 0) ? maca_cat_own(maca_cat("line-height:", named), ";", 1) : tw_decl("line-height", tw_space(v)));  }
const char* tw_widest(const char* v) { const char* named = tw_look(TwWide, v); return ((strcmp(named, "") != 0) ? maca_cat_own(maca_cat("max-width:", named), ";", 1) : tw_decl("max-width", tw_size(v)));  }
const char* tw_repeat(const char* prop, const char* v) { long n = tw_int(v); return ((n < 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("grid-template-", prop), ":repeat(", 1), maca_int_to_str(n), 3), ",minmax(0,1fr));", 1));  }
const char* tw_body(const char* c) { const char* fixed = tw_look(TwFixed, c); return ((strcmp(fixed, "") != 0) ? maca_cat_own(maca_cat("", fixed), ";", 1) : tw_measured(c));  }
const char* tw_measured(const char* c) { const char* arbitrary = tw_arbitrary(c); const char* edges = tw_edges(c); return (tw_starts(c, "min-h-") ? tw_decl("min-height", tw_size(tw_after(c, "min-h-"))) : (tw_starts(c, "min-w-") ? tw_decl("min-width", tw_size(tw_after(c, "min-w-"))) : ((strcmp(arbitrary, "") != 0) ? arbitrary : (tw_starts(c, "underline-offset-") ? tw_pixels("text-underline-offset", tw_after(c, "underline-offset-")) : (tw_starts(c, "leading-") ? tw_leading(tw_after(c, "leading-")) : (tw_starts(c, "decoration-") ? tw_decl("text-decoration-color", tw_look(TwColor, tw_after(c, "decoration-"))) : ((strcmp(edges, "") != 0) ? edges : tw_placed(c))))))));  }
const char* tw_pixels(const char* prop, const char* v) { long n = tw_int(v); return ((n < 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", prop), ":", 1), maca_int_to_str(n), 3), "px;", 1));  }
const char* tw_placed(const char* c) { const char* offsets = tw_offsets(c); const char* track = tw_track(c); return (tw_starts(c, "scroll-mt-") ? tw_decl("scroll-margin-top", tw_space(tw_after(c, "scroll-mt-"))) : (tw_starts(c, "scroll-mb-") ? tw_decl("scroll-margin-bottom", tw_space(tw_after(c, "scroll-mb-"))) : ((strcmp(offsets, "") != 0) ? offsets : (tw_starts(c, "max-w-") ? tw_widest(tw_after(c, "max-w-")) : (tw_starts(c, "max-h-") ? tw_decl("max-height", tw_size(tw_after(c, "max-h-"))) : (tw_starts(c, "gap-x-") ? tw_decl("column-gap", tw_space(tw_after(c, "gap-x-"))) : (tw_starts(c, "gap-y-") ? tw_decl("row-gap", tw_space(tw_after(c, "gap-y-"))) : (tw_starts(c, "grid-cols-") ? tw_repeat("columns", tw_after(c, "grid-cols-")) : (tw_starts(c, "grid-rows-") ? tw_repeat("rows", tw_after(c, "grid-rows-")) : ((strcmp(track, "") != 0) ? track : tw_scaled(c)))))))))));  }
const char* tw_scaled(const char* c) { long at = maca_str_index_of(c, "-"); return ((at < 0) ? "" : tw_scale(maca_str_slice(c, 0, at), maca_str_slice(c, (at + 1), ((int)strlen(c)))));  }
const char* tw_scale(const char* p, const char* v) { return (((strcmp(p, "p") == 0) || (strcmp(p, "m") == 0)) ? tw_decl(tw_box(p), tw_space(v)) : (((((strcmp(p, "px") == 0) || (strcmp(p, "py") == 0)) || (strcmp(p, "mx") == 0)) || (strcmp(p, "my") == 0)) ? tw_axes(p, tw_space(v)) : ((strcmp(tw_side_of(p), "") != 0) ? tw_decl(maca_cat_own(maca_cat_own(maca_cat("", tw_box(maca_str_slice(p, 0, 1))), "-", 1), tw_side_of(p), 1), tw_space(v)) : ((strcmp(p, "gap") == 0) ? tw_decl("gap", tw_space(v)) : tw_valued(p, v)))));  }
const char* tw_box(const char* p) { return ((strcmp(p, "p") == 0) ? "padding" : "margin");  }
const char* tw_side_of(const char* p) { return tw_look(TwSide, p);  }
const char* tw_axes(const char* p, const char* v) { const char* box = tw_box(maca_str_slice(p, 0, 1)); return (maca_ends_with(p, "x") ? tw_pair_decl(maca_cat_own(maca_cat("", box), "-left", 1), maca_cat_own(maca_cat("", box), "-right", 1), v) : tw_pair_decl(maca_cat_own(maca_cat("", box), "-top", 1), maca_cat_own(maca_cat("", box), "-bottom", 1), v));  }
const char* tw_valued(const char* p, const char* v) { return ((strcmp(p, "w") == 0) ? tw_decl("width", tw_size(v)) : ((strcmp(p, "h") == 0) ? tw_decl("height", tw_size(v)) : ((strcmp(p, "basis") == 0) ? tw_decl("flex-basis", tw_size(v)) : ((strcmp(p, "text") == 0) ? tw_typed(v) : ((strcmp(p, "leading") == 0) ? tw_decl("line-height", tw_look(TwLead, v)) : ((strcmp(p, "tracking") == 0) ? tw_decl("letter-spacing", tw_look(TwTrack, v)) : ((strcmp(p, "opacity") == 0) ? tw_opacity(v) : ((strcmp(p, "z") == 0) ? tw_decl("z-index", tw_signed(v)) : ((strcmp(p, "bg") == 0) ? tw_decl("background-color", tw_look(TwColor, v)) : ((strcmp(p, "border") == 0) ? tw_decl("border-color", tw_look(TwColor, v)) : ((strcmp(p, "caret") == 0) ? tw_decl("caret-color", tw_look(TwColor, v)) : (((strcmp(p, "rounded") == 0) && (strcmp(v, "sm") == 0)) ? "border-radius:0.125rem;" : ""))))))))))));  }
const char* tw_typed(const char* v) { const char* sized = tw_look(TwText, v); return ((strcmp(sized, "") != 0) ? maca_cat_own(maca_cat("font-size:", sized), ";", 1) : tw_decl("color", tw_look(TwColor, v)));  }
const char* tw_opacity(const char* v) { long n = tw_int(v); return ((n < 0) ? "" : maca_cat_own(maca_cat("opacity:", tw_dec((n * 10))), ";", 1));  }
MacaList tw_parts(const char* c, MacaList acc) { long at = maca_str_index_of(c, ":"); return (((at < 0) || (maca_str_index_of(maca_str_slice(c, 0, at), "[") >= 0)) ? maca_list_cat(acc, maca_listv(1, (long)(c))) : tw_parts(maca_str_slice(c, (at + 1), ((int)strlen(c))), maca_list_cat(acc, maca_listv(1, (long)(maca_str_slice(c, 0, at))))));  }
const char* tw_variant(const char* v) { return maca_cat(tw_look(TwPseudo, v), tw_look(TwMedia, v));  }
long tw_known(MacaList parts, long i, long n) { return ((i >= n) ? 1 : ((strcmp(tw_variant(((const char*)parts.data[i])), "") == 0) ? 0 : tw_known(parts, (i + 1), n)));  }
const char* tw_selector(MacaList parts, long i, long n, const char* acc) { return ((i >= n) ? acc : tw_selector(parts, (i + 1), n, maca_cat(acc, tw_look(TwPseudo, ((const char*)parts.data[i])))));  }
MacaList tw_queries(MacaList parts, long i, long n, MacaList acc) { return ((i >= n) ? acc : ({ const char* found = tw_look(TwMedia, ((const char*)parts.data[i])); MacaList got = ((strcmp(found, "") == 0) ? acc : maca_list_cat(acc, maca_listv(1, (long)(found)))); tw_queries(parts, (i + 1), n, got); }));  }
const char* tw_escape(const char* c) { return tw_escaped(maca_chars(c), 0, "");  }
const char* tw_escaped(MacaList cs, long i, const char* acc) { return ((i >= (cs.len)) ? acc : ((maca_str_index_of(TwEscaped, ((const char*)cs.data[i])) >= 0) ? tw_escaped(cs, (i + 1), maca_cat_own(maca_cat(acc, "\\"), ((const char*)cs.data[i]), 1)) : tw_escaped(cs, (i + 1), maca_cat(acc, ((const char*)cs.data[i])))));  }
const char* tw_rule(const char* c) { MacaList parts = tw_parts(c, maca_listv(0)); long n = ((parts.len) - 1); const char* body = tw_body(((const char*)parts.data[n])); return (((strcmp(body, "") == 0) || (!tw_known(parts, 0, n))) ? "" : tw_wrapped(tw_queries(parts, 0, n, maca_listv(0)), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(".", tw_escape(c)), tw_selector(parts, 0, n, ""), 1), " ", 1), maca_cat_own(maca_cat("{ ", body), " }", 1), 3)));  }
const char* tw_wrapped(MacaList media, const char* rule) { return (((media.len) == 0) ? rule : ({ const char* q = maca_list_join(media, " and "); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("@media", q), " { ", 1), rule, 1), " }", 1); }));  }
long tw_order(const char* c) { MacaList parts = tw_parts(c, maca_listv(0)); return (((parts.len) == 1) ? 0 : tw_rank(parts, 0, ((parts.len) - 1), 1));  }
long tw_rank(MacaList parts, long i, long n, long r) { return ((i >= n) ? r : tw_rank(parts, (i + 1), n, tw_layer(((const char*)parts.data[i]), r)));  }
long tw_layer(const char* v, long r) { return ((strcmp(v, "sm") == 0) ? 3 : ((strcmp(v, "md") == 0) ? 4 : ((strcmp(v, "lg") == 0) ? 5 : ((strcmp(v, "xl") == 0) ? 6 : ((strcmp(v, "max-lg") == 0) ? 7 : ((strcmp(v, "max-md") == 0) ? 8 : ((strcmp(v, "max-sm") == 0) ? 9 : (((strcmp(v, "dark") == 0) && (r < 2)) ? 2 : r))))))));  }
MacaList tw_words(const char* s) { return maca_split(maca_replace(maca_replace(s, "\\n", " "), "\\t", " "), " ");  }
MacaList tw_in_exprs(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : tw_in_exprs(xs, (i + 1), tw_in_expr((*(Expr*)xs.data[i]), acc)));  }
MacaList tw_in_expr(Expr e, MacaList acc) { MacaList got = ((e.kind == EStr) ? maca_list_cat(acc, tw_words(e.text)) : acc); long from = (tw_tagged(e) ? 1 : 0); return tw_in_stmts(e.stmts, 0, tw_in_exprs(e.children, from, got));  }
long tw_tagged(Expr e) { return ((e.kind == ECall) && (((strcmp(e.text, "maca_element") == 0) || (strcmp(e.text, "maca_attr") == 0)) || (strcmp(e.text, "maca_flag") == 0)));  }
MacaList tw_in_stmts(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : tw_in_stmts(items, (i + 1), tw_in_stmt((*(Stmt*)items.data[i]), acc)));  }
MacaList tw_in_stmt(Stmt s, MacaList acc) { return tw_in_stmts(s.body, 0, tw_in_expr(s.value, tw_in_exprs(s.params, 0, acc)));  }
MacaList tw_unique(MacaList cs, long i, MacaList acc) { return ((i >= (cs.len)) ? acc : (((i > 0) && (strcmp(((const char*)cs.data[i]), ((const char*)cs.data[(i - 1)])) == 0)) ? tw_unique(cs, (i + 1), acc) : tw_unique(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)cs.data[i])))))));  }
const char* tw_quoted(const char* s) { return maca_replace(s, "\\", "\\\\");  }
const char* tw_sheet(MacaList cs, long rank, const char* acc) { return ((rank > 9) ? acc : tw_sheet(cs, (rank + 1), maca_cat(acc, tw_rules(cs, 0, rank, ""))));  }
const char* tw_rules(MacaList cs, long i, long rank, const char* acc) { return ((i >= (cs.len)) ? acc : ({ const char* c = ((const char*)cs.data[i]); const char* r = ((tw_order(c) == rank) ? tw_rule(c) : ""); const char* line = ((strcmp(r, "") == 0) ? "" : maca_cat(tw_quoted(r), "\\n")); tw_rules(cs, (i + 1), rank, maca_cat(acc, line)); }));  }
const char* style_sheet(MacaList items) { return maca_cat(TwReset, tw_sheet(tw_unique(maca_list_sorted(tw_in_stmts(items, 0, maca_listv(0)), 1), 0, maca_listv(0)), 0, ""));  }
const char* quoted(const char* s) { return maca_cat_own(maca_cat("\"", s), "\"", 1);  }
const char* c_id(const char* name) { return ((maca_str_index_of(Reserved, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0) ? maca_cat_own(maca_cat("", name), "_mc", 1) : name);  }
const char* emit_expr(Expr e) { return (e.kind == EInt ? maca_int_to_str(e.ival) : (e.kind == EFloat ? e.text : (e.kind == EStr ? quoted(e.text) : (e.kind == EBool ? emit_bool(e) : (e.kind == EIdent ? c_ident_value(e) : (e.kind == ECall ? emit_call(e) : (e.kind == EBinary ? emit_binary(e) : (e.kind == ETernary ? emit_ternary(e) : (e.kind == EIf ? emit_ternary(e) : (e.kind == EUnary ? emit_unary(e) : (e.kind == ERecord ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", e.text), "){ ", 1), emit_lit_fields(e.children, 0), 1), " }", 1) : (e.kind == EWith ? emit_with(e) : (e.kind == EBlock ? emit_block(e) : (e.kind == EField ? maca_cat_own(maca_cat_own(maca_cat("", emit_expr((*(Expr*)e.children.data[0]))), ".", 1), c_id(e.text), 1) : (e.kind == EMatch ? emit_match(e) : (e.kind == EMethod ? emit_method(e) : (e.kind == EList ? emit_list(e) : (e.kind == EJump ? emit_jump(e, "") : (e.kind == EWhile ? maca_cat_own(maca_cat("({ ", emit_while(e)), " 0; })", 1) : (e.kind == EFor ? maca_cat_own(maca_cat("({ ", emit_for(e)), " 0; })", 1) : "0"))))))))))))))))))));  }
const char* emit_while(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("while (", emit_expr((*(Expr*)e.children.data[0]))), ")", 1), maca_cat_own(maca_cat(" { ", block_stmts(e.stmts, 0)), "}", 1), 3);  }
const char* emit_for(Expr e) { Expr over = (*(Expr*)e.children.data[0]); const char* el = c_elem_of(over.ty); const char* cell = c_cell_at("_f", el, "_fi"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ MacaList _f = ", emit_expr(over)), ";", 1), maca_cat(" for (int _fi = 0; _fi < _f.len; _fi++) { ", type_c(el)), 3), maca_cat(" ", c_id(e.text)), 3), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" = ", cell), "; ", 1), block_stmts(e.stmts, 0), 1), "} }", 1), 3);  }
long is_loop(Expr e) { return ((e.kind == EWhile) || (e.kind == EFor));  }
const char* emit_loop(Expr e) { return ((e.kind == EWhile) ? emit_while(e) : emit_for(e));  }
const char* emit_unary(Expr e) { return ((strcmp(e.text, "fail") == 0) ? maca_cat_own(maca_cat("maca_fail(", emit_expr((*(Expr*)e.children.data[0]))), ")", 1) : ((strcmp(e.text, "try") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own("({ jmp_buf* _jb = maca_try_push(); char* _tr;", maca_cat_own(maca_cat(" if (setjmp(*_jb) == 0) { (void)(", emit_expr((*(Expr*)e.children.data[0]))), ");", 1), 2), " maca_try_pop(); _tr = maca_cat(\"\", \"\"); }", 1), " else { _tr = maca_last_fail(); } _tr; })", 1) : ((strcmp(e.text, "spawn") == 0) ? emit_spawn((*(Expr*)e.children.data[0])) : ((strcmp(e.text, "await") == 0) ? maca_cat_own(maca_cat("((int)maca_await(", emit_expr((*(Expr*)e.children.data[0]))), "))", 1) : maca_cat_own(maca_cat_own(maca_cat("(", e.text), emit_expr((*(Expr*)e.children.data[0])), 1), ")", 1)))));  }
const char* emit_spawn(Expr call) { const char* task = c_id(call.text); long n = (call.children.len); return ((n == 0) ? maca_cat_own(maca_cat("maca_spawn((MacaTask)", task), ", 0)", 1) : ((n == 1) ? maca_cat_own(maca_cat_own(maca_cat("maca_spawn((MacaTask)", task), ",", 1), maca_cat_own(maca_cat(" (long)(", emit_expr((*(Expr*)call.children.data[0]))), "))", 1), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_spawn2((MacaTask2)", task), ",", 1), maca_cat_own(maca_cat(" (long)(", emit_expr((*(Expr*)call.children.data[0]))), "),", 1), 3), maca_cat_own(maca_cat(" (long)(", emit_expr((*(Expr*)call.children.data[1]))), "))", 1), 3)));  }
const char* emit_jump(Expr e, const char* ret) { return ((strcmp(e.text, "return") != 0) ? e.text : (((e.children.len) == 0) ? maca_cat("return ", c_zero(ret)) : maca_cat("return ", emit_expr((*(Expr*)e.children.data[0])))));  }
const char* emit_bool(Expr e) { return ((strcmp(e.text, "true") == 0) ? "1" : "0");  }
const char* emit_ternary(Expr e) { const char* cond = emit_expr((*(Expr*)e.children.data[0])); const char* then = c_arm((*(Expr*)e.children.data[1]), (*(Expr*)e.children.data[2]).ty); const char* els = (is_missing_else((*(Expr*)e.children.data[2])) ? c_zero((*(Expr*)e.children.data[1]).ty) : c_arm((*(Expr*)e.children.data[2]), (*(Expr*)e.children.data[1]).ty)); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", cond), " ? ", 1), then, 1), " : ", 1), els, 1), ")", 1);  }
const char* c_arm(Expr e, const char* other) { return (is_raise(e) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emit_expr(e)), ", ", 1), c_zero(other), 1), ")", 1) : emit_expr(e));  }
const char* emit_list(Expr e) { long n = (e.children.len); return ((n == 0) ? "maca_listv(0)" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("maca_listv(", maca_int_to_str(n), 2), ", ", 1), c_cells(c_elem_of(e.ty), e.children, 0), 1), ")", 1));  }
const char* c_cells(const char* el, MacaList cs, long i) { return ((i >= (cs.len)) ? "" : ((i == ((cs.len) - 1)) ? c_cell(el, (*(Expr*)cs.data[i])) : maca_cat_own(maca_cat_own(maca_cat("", c_cell(el, (*(Expr*)cs.data[i]))), ", ", 1), c_cells(el, cs, (i + 1)), 1)));  }
const char* c_cell(const char* el, Expr e) { return c_cell_of(el, emit_expr(e));  }
const char* c_cell_of(const char* el, const char* code) { return (c_boxed(el) ? ({ const char* ct = type_c(el); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_box(sizeof(", ct), "), (", 1), ct, 1), "[]){ ", 1), code, 1), " })", 1); }) : maca_cat_own(maca_cat("(long)(", code), ")", 1));  }
const char* c_cell_at(const char* recv, const char* el, const char* ix) { return (c_boxed(el) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(*(", type_c(el)), "*)", 1), recv, 1), ".data[", 1), ix, 1), "])", 1) : ((strcmp(el, "str") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((const char*)", recv), ".data[", 1), ix, 1), "])", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((long)", recv), ".data[", 1), ix, 1), "])", 1)));  }
long c_boxed(const char* el) { return ((((strcmp(el, "") != 0) && (strcmp(el, "int") != 0)) && (strcmp(el, "bool") != 0)) && (strcmp(el, "str") != 0));  }
const char* c_elem_of(const char* ty) { return (is_list_type(ty) ? maca_str_slice(ty, 0, (((int)strlen(ty)) - 2)) : "");  }
long map_method(const char* name) { return (((((((((strcmp(name, "get") == 0) || (strcmp(name, "set") == 0)) || (strcmp(name, "has") == 0)) || (strcmp(name, "keys") == 0)) || (strcmp(name, "length") == 0)) || (strcmp(name, "count") == 0)) || (strcmp(name, "remove") == 0)) || (strcmp(name, "values") == 0)) || (strcmp(name, "contains") == 0));  }
const char* emit_map_method(Expr e, const char* recv) { const char* val = map_type_val((*(Expr*)e.children.data[0]).ty); const char* key = (((e.children.len) > 1) ? emit_expr((*(Expr*)e.children.data[1])) : ""); return ((strcmp(e.text, "keys") == 0) ? maca_cat_own(maca_cat("maca_map_keys(", recv), ")", 1) : ((strcmp(e.text, "values") == 0) ? maca_cat_own(maca_cat("maca_map_vals(", recv), ")", 1) : (((strcmp(e.text, "length") == 0) || (strcmp(e.text, "count") == 0)) ? maca_cat_own(maca_cat("(", recv), ".keys.len)", 1) : (((strcmp(e.text, "has") == 0) || (strcmp(e.text, "contains") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_map_has(", recv), ", ", 1), key, 1), ")", 1) : ((strcmp(e.text, "set") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_map_set(", recv), ", ", 1), key, 1), ", ", 1), c_cell(val, (*(Expr*)e.children.data[2])), 1), ")", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_map_remove(", recv), ", ", 1), key, 1), ")", 1) : (((e.children.len) > 2) ? c_map_read(val, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_map_or(", recv), ", ", 1), key, 1), ",", 1), maca_cat_own(maca_cat(" ", c_cell(val, (*(Expr*)e.children.data[2]))), ")", 1), 3)) : c_map_read(val, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_map_get(", recv), ", ", 1), key, 1), ")", 1)))))))));  }
const char* c_map_read(const char* val, const char* call) { return (c_boxed(val) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(*(", type_c(val)), "*)", 1), call, 1), ")", 1) : (((strcmp(val, "int") == 0) || (strcmp(val, "bool") == 0)) ? maca_cat_own(maca_cat("((long)", call), ")", 1) : maca_cat_own(maca_cat("((const char*)", call), ")", 1)));  }
long c_own_type(const char* ty) { return ((((((((((strcmp(ty, "") != 0) && (strcmp(ty, "str") != 0)) && (strcmp(ty, "int") != 0)) && (strcmp(ty, "bool") != 0)) && (strcmp(ty, "float") != 0)) && (strcmp(ty, "Element") != 0)) && (!is_list_type(ty))) && (!is_map_type(ty))) && (!c_sized_number(ty))) && (!is_vector_type(ty)));  }
long c_sized_number(const char* ty) { return ((((((int)strlen(ty)) > 1) && (isdigit((unsigned char)(maca_str_at(ty, 1))[0]) != 0)) && (maca_str_index_of(ty, "x") < 0)) && (((strcmp(maca_str_at(ty, 0), "i") == 0) || (strcmp(maca_str_at(ty, 0), "u") == 0)) || (strcmp(maca_str_at(ty, 0), "f") == 0)));  }
long c_sized_float(const char* ty) { return (c_sized_number(ty) && (strcmp(maca_str_at(ty, 0), "f") == 0));  }
long is_vector_type(const char* ty) { return (c_vec_lanes(ty) > 1);  }
long c_vec_lanes(const char* ty) { long at = maca_str_index_of(ty, "x"); return (((at < 2) || (!c_sized_number(maca_str_slice(ty, 0, at)))) ? 0 : atol(maca_str_slice(ty, (at + 1), ((int)strlen(ty)))));  }
const char* c_vec_scalar(const char* ty) { return type_c(maca_str_slice(ty, 0, maca_str_index_of(ty, "x")));  }
const char* c_vec_splat(const char* ty, const char* arg) { const char* one = maca_cat_own(maca_cat("", arg), ", ", 1); const char* lanes = maca_repeat(one, c_vec_lanes(ty)); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", ty), "){ ", 1), lanes, 1), " })", 1);  }
const char* c_vec_sum(const char* ty, const char* recv) { const char* over = maca_cat_own(maca_cat_own("for (int _vi = 0; _vi < ", maca_int_to_str(c_vec_lanes(ty)), 2), "; _vi++)", 1); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ ", ty), " _v = ", 1), recv, 1), "; ", 1), c_vec_scalar(ty), 1), " _a = 0;", 1), maca_cat_own(maca_cat(" ", over), " _a += _v[_vi]; _a; })", 1), 3);  }
const char* emit_method(Expr e) { const char* recv = emit_expr((*(Expr*)e.children.data[0])); const char* rty = (*(Expr*)e.children.data[0]).ty; long on_list = is_list_type(rty); long on_str = (strcmp(rty, "str") == 0); const char* el = c_elem_of(rty); return ((e.ival == 1) ? c_indirect(maca_cat_own(maca_cat_own(maca_cat("", recv), ".", 1), c_id(e.text), 1), type_c(e.ty), maca_list_slice(e.children, 1, (e.children.len))) : (((strcmp(e.text, "splat") == 0) && is_vector_type((*(Expr*)e.children.data[0]).text)) ? c_vec_splat((*(Expr*)e.children.data[0]).text, emit_expr((*(Expr*)e.children.data[1]))) : (((strcmp(e.text, "sum") == 0) && is_vector_type(rty)) ? c_vec_sum(rty, recv) : (c_own_type(rty) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(e.text)), "(", 1), c_ufcs_args(recv, e.children), 1), ")", 1) : ((is_map_type(rty) && map_method(e.text)) ? emit_map_method(e, recv) : (((strcmp(e.text, "count") == 0) || ((strcmp(e.text, "length") == 0) && on_list)) ? maca_cat_own(maca_cat("(", recv), ".len)", 1) : ((strcmp(e.text, "length") == 0) ? maca_cat_own(maca_cat("((int)strlen(", recv), "))", 1) : (((strcmp(e.text, "at") == 0) || ((strcmp(e.text, "get") == 0) && on_str)) ? c_at(recv, on_str, emit_expr((*(Expr*)e.children.data[1]))) : ((strcmp(e.text, "get") == 0) ? c_cell_at(recv, el, emit_expr((*(Expr*)e.children.data[1]))) : (((strcmp(e.text, "slice") == 0) && on_list) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_slice(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ",", 1), maca_cat_own(maca_cat(" ", emit_expr((*(Expr*)e.children.data[2]))), ")", 1), 3) : ((strcmp(e.text, "slice") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_str_slice(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ",", 1), maca_cat_own(maca_cat(" ", emit_expr((*(Expr*)e.children.data[2]))), ")", 1), 3) : ((strcmp(e.text, "ends_with") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_ends_with(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ")", 1) : (((strcmp(e.text, "index_of") == 0) && on_list) ? c_list_find(recv, el, (*(Expr*)e.children.data[1])) : ((strcmp(e.text, "index_of") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_str_index_of(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ")", 1) : ((strcmp(e.text, "join") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_join(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ")", 1) : ((strcmp(e.text, "push") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_pushed(", recv), ", ", 1), c_cell(el, (*(Expr*)e.children.data[1])), 1), ")", 1) : (((strcmp(e.text, "map") == 0) || (strcmp(e.text, "parallel") == 0)) ? emit_map(e) : ((strcmp(e.text, "filter") == 0) ? emit_filter(e) : (((strcmp(e.text, "reduce") == 0) || (strcmp(e.text, "fold") == 0)) ? emit_reduce(e) : ((strcmp(e.text, "chars") == 0) ? maca_cat_own(maca_cat("maca_chars(", recv), ")", 1) : ((strcmp(e.text, "is_whitespace") == 0) ? maca_cat_own(maca_cat("(isspace((unsigned char)", c_char(recv, on_str)), ") != 0)", 1) : ((strcmp(e.text, "is_ascii_digit") == 0) ? maca_cat_own(maca_cat("(isdigit((unsigned char)", c_char(recv, on_str)), ") != 0)", 1) : ((strcmp(e.text, "is_alpha") == 0) ? maca_cat_own(maca_cat("(isalpha((unsigned char)", c_char(recv, on_str)), ") != 0)", 1) : (((strcmp(e.text, "upper") == 0) && on_str) ? maca_cat_own(maca_cat("maca_upper(", recv), ")", 1) : ((strcmp(e.text, "upper") == 0) ? maca_cat_own(maca_cat("toupper((unsigned char)", c_char(recv, on_str)), ")", 1) : (((strcmp(e.text, "contains") == 0) && on_list) ? maca_cat_own(maca_cat("(", c_list_find(recv, el, (*(Expr*)e.children.data[1]))), " >= 0)", 1) : ((strcmp(e.text, "contains") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(strstr(", recv), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ") != NULL)", 1) : ((on_list && c_list_method(e.text)) ? emit_list_method(e, recv, el) : ((c_padding(e.text) && ((e.children.len) == 2)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_", e.text), "(", 1), recv, 1), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ", \" \")", 1) : (c_str_method(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_", e.text), "(", 1), c_ufcs_args(recv, e.children), 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(e.text)), "(", 1), c_ufcs_args(recv, e.children), 1), ")", 1)))))))))))))))))))))))))))))));  }
long c_list_method(const char* name) { return (((((((((((strcmp(name, "sum") == 0) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) || (strcmp(name, "first") == 0)) || (strcmp(name, "last") == 0)) || (strcmp(name, "pop") == 0)) || (strcmp(name, "reverse") == 0)) || (strcmp(name, "sort") == 0)) || (strcmp(name, "set") == 0)) || (strcmp(name, "insert") == 0)) || (strcmp(name, "remove") == 0));  }
const char* emit_list_method(Expr e, const char* recv, const char* el) { const char* held = type_c(el); const char* cell = c_cell_at("_q", el, "_qi"); const char* over = "for (int _qi = 0; _qi < _q.len; _qi++)"; const char* arg1 = emit_args(e.children, 1); return ((strcmp(e.text, "sum") == 0) ? c_over(recv, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", held), " _a = 0; ", 1), over, 1), " _a += ", 1), cell, 1), "; _a;", 1)) : ((strcmp(e.text, "min") == 0) ? c_over(recv, c_fold_pick(held, over, el, "<")) : ((strcmp(e.text, "max") == 0) ? c_over(recv, c_fold_pick(held, over, el, ">")) : ((strcmp(e.text, "first") == 0) ? c_over(recv, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_q.len ? ", c_cell_at("_q", el, "0")), " : ", 1), c_zero(el), 1), ";", 1)) : ((strcmp(e.text, "last") == 0) ? c_over(recv, maca_cat_own(maca_cat("_q.len ? ", c_cell_at("_q", el, "_q.len - 1")), maca_cat_own(maca_cat(" : ", c_zero(el)), ";", 1), 3)) : ((strcmp(e.text, "pop") == 0) ? c_over(recv, "maca_list_slice(_q, 0, _q.len - 1);") : ((strcmp(e.text, "reverse") == 0) ? maca_cat_own(maca_cat("maca_list_reverse(", recv), ")", 1) : ((strcmp(e.text, "sort") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_sorted(", recv), ", ", 1), c_cell_kind(el), 1), ")", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_remove(", recv), ", ", 1), arg1, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_", e.text), "(", 1), recv, 1), ", ", 1), emit_expr((*(Expr*)e.children.data[1])), 1), ",", 1), maca_cat_own(maca_cat(" ", c_cell(el, (*(Expr*)e.children.data[2]))), ")", 1), 3))))))))));  }
const char* c_over(const char* recv, const char* body) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaList _q = ", recv), "; ", 1), body, 1), " })", 1);  }
const char* c_fold_pick(const char* held, const char* over, const char* el, const char* op) { const char* cell = c_cell_at("_q", el, "_qi"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", held), " _a = _q.len ? ", 1), c_cell_at("_q", el, "0"), 1), " : ", 1), c_zero(el), 1), ";", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" ", over), " if (", 1), cell, 1), " ", 1), op, 1), " _a) _a = ", 1), cell, 1), "; _a;", 1), 3);  }
const char* c_cell_kind(const char* el) { return ((strcmp(el, "str") == 0) ? "1" : ((strcmp(el, "float") == 0) ? "2" : "0"));  }
long c_padding(const char* name) { return ((strcmp(name, "pad_start") == 0) || (strcmp(name, "pad_end") == 0));  }
long c_str_method(const char* name) { return (((((((((((strcmp(name, "trim") == 0) || (strcmp(name, "lower") == 0)) || (strcmp(name, "replace") == 0)) || (strcmp(name, "repeat") == 0)) || (strcmp(name, "substr") == 0)) || (strcmp(name, "starts_with") == 0)) || (strcmp(name, "split") == 0)) || (strcmp(name, "pad_start") == 0)) || (strcmp(name, "pad_end") == 0)) || (strcmp(name, "pad_center") == 0)) || (strcmp(name, "fixed") == 0));  }
const char* c_ufcs_args(const char* recv, MacaList cs) { const char* rest = emit_args(cs, 1); return ((strcmp(rest, "") == 0) ? recv : maca_cat_own(maca_cat_own(maca_cat("", recv), ", ", 1), rest, 1));  }
const char* c_apply(Expr f, const char* el, const char* arg, const char* ret) { return ((f.kind == ELambda) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ ", type_c(el)), " ", 1), c_id((*(Expr*)f.children.data[0]).text), 1), " = ", 1), arg, 1), ";", 1), maca_cat_own(maca_cat(" ", emit_expr(lambda_body(f))), "; })", 1), 3) : ((f.ival == 3) ? c_call_value(c_id(f.text), ret, maca_listv(1, (long)(type_c(el))), maca_listv(1, (long)(arg))) : ((f.kind == EIdent) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(f.text)), "(", 1), arg, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", emit_expr(f)), "(", 1), arg, 1), ")", 1))));  }
const char* emit_map(Expr e) { const char* src = emit_expr((*(Expr*)e.children.data[0])); const char* el = c_elem_of((*(Expr*)e.children.data[0]).ty); const char* cell = c_cell_at("_m", el, "_i"); Expr f = (*(Expr*)e.children.data[1]); const char* got = c_cell_of(c_elem_of(e.ty), c_apply(f, el, cell, type_c(c_elem_of(e.ty)))); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaList _m = ", src), "; MacaList _r; _r.len = _m.len;", 1), " _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long));", 1), maca_cat_own(maca_cat(" for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = ", got), ";", 1), 3), " _r; })", 1);  }
const char* emit_filter(Expr e) { const char* src = emit_expr((*(Expr*)e.children.data[0])); const char* el = c_elem_of((*(Expr*)e.children.data[0]).ty); const char* cell = c_cell_at("_m", el, "_i"); const char* kept = c_apply((*(Expr*)e.children.data[1]), el, cell, "long"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaList _m = ", src), "; MacaList _r; _r.len = 0;", 1), " _r.data = maca_cells((_m.len ? _m.len : 1) * sizeof(long));", 1), " for (int _i = 0; _i < _m.len; _i++)", 1), maca_cat_own(maca_cat(" if (", kept), ") _r.data[_r.len++] = _m.data[_i]; _r; })", 1), 3);  }
const char* emit_reduce(Expr e) { const char* src = emit_expr((*(Expr*)e.children.data[0])); const char* el = c_elem_of((*(Expr*)e.children.data[0]).ty); const char* cell = c_cell_at("_m", el, "_i"); Expr seed = (*(Expr*)e.children.data[1]); const char* held = local_c_type(seed); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaList _m = ", src), "; ", 1), held, 1), " _acc = ", 1), emit_expr(seed), 1), ";", 1), " for (int _i = 0; _i < _m.len; _i++) _acc =", 1), maca_cat_own(maca_cat(" ", c_apply2((*(Expr*)e.children.data[2]), held, el, cell)), "; _acc; })", 1), 3);  }
const char* c_apply2(Expr f, const char* held, const char* el, const char* cur) { return ((f.kind == ELambda) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ ", held), " ", 1), c_id((*(Expr*)f.children.data[0]).text), 1), " = _acc;", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" ", type_c(el)), " ", 1), c_id((*(Expr*)f.children.data[1]).text), 1), " = ", 1), cur, 1), ";", 1), 3), maca_cat_own(maca_cat(" ", emit_expr(lambda_body(f))), "; })", 1), 3) : ((f.ival == 3) ? c_call_value(c_id(f.text), held, maca_listv(2, (long)(held), (long)(type_c(el))), maca_listv(2, (long)("_acc"), (long)(cur))) : ((f.kind == EIdent) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(f.text)), "(_acc, ", 1), cur, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", emit_expr(f)), "(_acc, ", 1), cur, 1), ")", 1))));  }
const char* c_list_find(const char* recv, const char* el, Expr want) { return ((strcmp(el, "str") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_index_of_str(", recv), ", ", 1), emit_expr(want), 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_index_of(", recv), ", ", 1), c_cell(el, want), 1), ")", 1));  }
const char* c_at(const char* recv, long on_str, const char* ix) { return (on_str ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_str_at(", recv), ", ", 1), ix, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((int)", recv), "[", 1), ix, 1), "])", 1));  }
const char* c_char(const char* recv, long on_str) { return (on_str ? maca_cat_own(maca_cat("(", recv), ")[0]", 1) : maca_cat_own(maca_cat("(", recv), ")", 1));  }
long is_list_type(const char* ty) { return ((((int)strlen(ty)) >= 2) && (strcmp(maca_str_slice(ty, (((int)strlen(ty)) - 2), ((int)strlen(ty))), "[]") == 0));  }
const char* emit_call(Expr e) { const char* args = emit_args(e.children, 0); return ((e.ival == 2) ? c_indirect(c_id(e.text), type_c(e.ty), e.children) : ((strcmp(e.text, "maca_closure") == 0) ? c_closure(e) : ((strcmp(e.text, "maca_cap") == 0) ? c_cell_at("_maca_env", (*(Expr*)e.children.data[0]).text, emit_expr((*(Expr*)e.children.data[1]))) : (((strcmp(e.text, "map") == 0) && ((e.children.len) == 0)) ? "maca_map_new()" : ((strcmp(e.text, "info") == 0) ? c_say("stdout", args, "\\n") : ((strcmp(e.text, "print") == 0) ? c_say("stdout", args, "") : ((strcmp(e.text, "str") == 0) ? c_str_of(c_arg_ty(e.children), args) : ((strcmp(e.text, "int") == 0) ? c_int_of(c_arg_ty(e.children), args) : ((strcmp(e.text, "read_file") == 0) ? maca_cat_own(maca_cat("maca_read_file(", args), ")", 1) : ((strcmp(e.text, "write_file") == 0) ? maca_cat_own(maca_cat("maca_write_file(", args), ")", 1) : (((strcmp(e.text, "read_line") == 0) || (strcmp(e.text, "at_eof") == 0)) ? ((strcmp(e.text, "at_eof") == 0) ? "maca_at_eof()" : "maca_input(\"\")") : ((strcmp(e.text, "exec") == 0) ? maca_cat_own(maca_cat("maca_exec(", args), ")", 1) : ((strcmp(e.text, "assert") == 0) ? maca_cat_own(maca_cat("maca_assert(", args), ")", 1) : ((strcmp(e.text, "assert_eq") == 0) ? c_assert_eq(e.children) : ((strcmp(e.text, "failures") == 0) ? "maca_failures()" : (((strcmp(e.text, "styles") == 0) && ((e.children.len) == 0)) ? "MACA_STYLES" : emit_prelude(e, args)))))))))))))))));  }
const char* emit_prelude(Expr e, const char* args) { return ((strcmp(e.text, "float") == 0) ? maca_cat_own(maca_cat("((double)(", args), "))", 1) : (c_math_call(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("__builtin_", e.text), "((double)(", 1), args, 1), "))", 1) : ((strcmp(e.text, "pow") == 0) ? maca_cat_own(maca_cat("__builtin_pow(", args), ")", 1) : ((strcmp(e.text, "abs") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", args), ") _x = ", 1), args, 1), "; _x < 0 ? -_x : _x; })", 1) : ((strcmp(e.text, "sign") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", args), ") _x = ", 1), args, 1), "; (_x > 0) - (_x < 0); })", 1) : (((strcmp(e.text, "clamp") == 0) && ((e.children.len) == 3)) ? c_clamp(e) : (((strcmp(e.text, "gcd") == 0) && ((e.children.len) == 2)) ? c_gcd(e) : ((strcmp(e.text, "len") == 0) ? c_len(e.children, args) : ((c_picking(e.text) && ((e.children.len) == 2)) ? c_pick2(e) : (((strcmp(e.text, "err") == 0) || (strcmp(e.text, "warn") == 0)) ? c_say("stderr", args, "\\n") : (c_runtime_call(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_", e.text), "(", 1), args, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(e.text)), "(", 1), args, 1), ")", 1))))))))))));  }
long c_math_call(const char* name) { return (((((((((strcmp(name, "sqrt") == 0) || (strcmp(name, "floor") == 0)) || (strcmp(name, "ceil") == 0)) || (strcmp(name, "round") == 0)) || (strcmp(name, "sin") == 0)) || (strcmp(name, "cos") == 0)) || (strcmp(name, "tan") == 0)) || (strcmp(name, "log") == 0)) || (strcmp(name, "exp") == 0));  }
long c_picking(const char* name) { return ((strcmp(name, "min") == 0) || (strcmp(name, "max") == 0));  }
const char* c_pick2(Expr e) { const char* a = emit_expr((*(Expr*)e.children.data[0])); const char* b = emit_expr((*(Expr*)e.children.data[1])); const char* op = ((strcmp(e.text, "min") == 0) ? "<" : ">"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", a), ") _x = ", 1), a, 1), ", _y = ", 1), b, 1), ";", 1), maca_cat_own(maca_cat(" _x ", op), " _y ? _x : _y; })", 1), 3);  }
const char* c_clamp(Expr e) { const char* v = emit_expr((*(Expr*)e.children.data[0])); const char* lo = emit_expr((*(Expr*)e.children.data[1])); const char* hi = emit_expr((*(Expr*)e.children.data[2])); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", v), ") _v = ", 1), v, 1), ", _lo = ", 1), lo, 1), ", _hi = ", 1), hi, 1), ";", 1), " _v < _lo ? _lo : (_v > _hi ? _hi : _v); })", 1);  }
const char* c_gcd(Expr e) { const char* a = emit_expr((*(Expr*)e.children.data[0])); const char* b = emit_expr((*(Expr*)e.children.data[1])); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", a), ") _a = ", 1), a, 1), ", _b = ", 1), b, 1), ";", 1), " while (_b) { __typeof__(_a) _t = _b; _b = _a % _b; _a = _t; }", 1), " _a < 0 ? -_a : _a; })", 1);  }
const char* c_len(MacaList cs, const char* args) { return (is_list_type(c_arg_ty(cs)) ? maca_cat_own(maca_cat("(", args), ").len", 1) : maca_cat_own(maca_cat("((int)strlen(", args), "))", 1));  }
long c_runtime_call(const char* name) { return ((c_char_call(name) || c_file_call(name)) || c_host_call(name));  }
long c_char_call(const char* name) { return ((((((strcmp(name, "chr") == 0) || (strcmp(name, "ord") == 0)) || (strcmp(name, "now_ms") == 0)) || (strcmp(name, "now_iso") == 0)) || (strcmp(name, "format_time") == 0)) || (strcmp(name, "sleep_ms") == 0));  }
long c_file_call(const char* name) { return ((((((((((strcmp(name, "file_exists") == 0) || (strcmp(name, "is_dir") == 0)) || (strcmp(name, "file_size") == 0)) || (strcmp(name, "modified_ms") == 0)) || (strcmp(name, "make_dir") == 0)) || (strcmp(name, "list_dir") == 0)) || (strcmp(name, "remove_file") == 0)) || (strcmp(name, "remove_dir") == 0)) || (strcmp(name, "copy_bytes") == 0)) || (strcmp(name, "real_path") == 0));  }
long c_host_call(const char* name) { return ((((((((strcmp(name, "env") == 0) || (strcmp(name, "cwd") == 0)) || (strcmp(name, "chdir") == 0)) || (strcmp(name, "is_tty") == 0)) || (strcmp(name, "capture") == 0)) || (strcmp(name, "capture_err") == 0)) || (strcmp(name, "alloc_count") == 0)) || (strcmp(name, "reuse_count") == 0));  }
const char* c_assert_eq(MacaList cs) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_assert_eq(", c_shown(cs, 0)), ", ", 1), c_shown(cs, 1), 1), ", ", 1), c_shown(cs, 2), 1), ")", 1);  }
const char* c_shown(MacaList cs, long i) { return ((i >= (cs.len)) ? "\"\"" : c_str_of((*(Expr*)cs.data[i]).ty, emit_expr((*(Expr*)cs.data[i]))));  }
const char* c_arg_ty(MacaList cs) { return (((cs.len) == 0) ? "" : (*(Expr*)cs.data[0]).ty);  }
const char* c_str_of(const char* ty, const char* args) { return ((strcmp(ty, "str") == 0) ? args : (((strcmp(ty, "float") == 0) || c_sized_float(ty)) ? maca_cat_own(maca_cat("maca_float_to_str(", args), ")", 1) : ((strcmp(ty, "bool") == 0) ? maca_cat_own(maca_cat("maca_bool_to_str(", args), ")", 1) : (is_list_type(ty) ? c_list_str(c_elem_of(ty), args) : maca_cat_own(maca_cat("maca_int_to_str(", args), ")", 1)))));  }
const char* c_list_str(const char* el, const char* code) { const char* piece = c_str_of(el, c_cell_at("_t", el, "_ti")); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaList _t = ", code), "; char* _s = maca_cat(\"[\", \"\");", 1), " for (int _ti = 0; _ti < _t.len; _ti++) {", 1), " if (_ti) _s = maca_cat_own(_s, \", \", 1);", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" _s = maca_cat_own(_s, ", piece), ", ", 1), maca_int_to_str((c_fresh(piece) ? 3 : 1)), 3), ");", 1), 3), " } _s = maca_cat_own(_s, \"]\", 1); _s; })", 1);  }
const char* c_int_of(const char* ty, const char* args) { return ((strcmp(ty, "int") == 0) ? args : (((strcmp(ty, "float") == 0) || (strcmp(ty, "bool") == 0)) ? maca_cat_own(maca_cat("((long)(", args), "))", 1) : maca_cat_own(maca_cat("atol(", args), ")", 1)));  }
long c_compare(const char* op) { return ((((((strcmp(op, "==") == 0) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">=") == 0));  }
long c_is_str(Expr e) { return ((e.kind == EStr) || (strcmp(e.ty, "str") == 0));  }
long c_joins(const char* op, Expr l, Expr r) { return ((strcmp(op, "++") == 0) || ((strcmp(op, "+") == 0) && ((c_is_str(l) && c_is_str(r)) || (is_list_type(l.ty) && is_list_type(r.ty)))));  }
const char* emit_binary(Expr e) { Expr l = (*(Expr*)e.children.data[0]); Expr r = (*(Expr*)e.children.data[1]); long is_cmp = c_compare(e.text); long is_str_eq = (is_cmp && (c_is_str(l) || c_is_str(r))); return ((((strcmp(e.text, "=") == 0) && (l.kind == EMethod)) && (strcmp(l.text, "get") == 0)) ? c_store(l, r) : ((strcmp(e.text, "..") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_range(", emit_expr(l)), ", ", 1), emit_expr(r), 1), ")", 1) : (((strcmp(c_overload(e.text), "") != 0) && c_own_type(l.ty)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(c_overload(e.text))), "(", 1), emit_expr(l), 1), ", ", 1), emit_expr(r), 1), ")", 1) : ((c_joins(e.text, l, r) && (is_list_type(l.ty) || is_list_type(r.ty))) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_list_cat(", emit_expr(l)), ", ", 1), emit_expr(r), 1), ")", 1) : (c_joins(e.text, l, r) ? c_join(c_joined(l), c_joined(r)) : (is_str_eq ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(strcmp(", emit_expr(l)), ", ", 1), emit_expr(r), 1), ") ", 1), e.text, 1), " 0)", 1) : (((strcmp(e.text, "/") == 0) && c_is_str(l)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_path_join(", emit_expr(l)), ", ", 1), emit_expr(r), 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emit_expr(l)), " ", 1), e.text, 1), " ", 1), emit_expr(r), 1), ")", 1))))))));  }
const char* c_joined(Expr e) { return ((c_is_str(e) || (((strcmp(e.ty, "int") != 0) && (strcmp(e.ty, "float") != 0)) && (strcmp(e.ty, "bool") != 0))) ? emit_expr(e) : c_str_of(e.ty, emit_expr(e)));  }
long c_fresh(const char* code) { long at = maca_str_index_of(code, "("); return ((at > 0) && (maca_str_index_of(Fresh, maca_cat_own(maca_cat_own(" ", maca_str_slice(code, 0, at), 2), " ", 1)) >= 0));  }
const char* c_join(const char* l, const char* r) { long own = ((c_fresh(l) ? 1 : 0) + (c_fresh(r) ? 2 : 0)); return ((own == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_cat(", l), ", ", 1), r, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_cat_own(", l), ", ", 1), r, 1), ", ", 1), maca_int_to_str(own), 3), ")", 1));  }
const char* c_say(const char* f, const char* args, const char* end) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_say(", f), ", ", 1), args, 1), ", \"", 1), end, 1), "\", ", 1), maca_int_to_str((c_fresh(args) ? 1 : 0)), 3), ")", 1);  }
const char* c_overload(const char* op) { return ((strcmp(op, "+") == 0) ? "add" : ((strcmp(op, "-") == 0) ? "sub" : ((strcmp(op, "*") == 0) ? "mul" : ((strcmp(op, "/") == 0) ? "div" : ((strcmp(op, "%") == 0) ? "rem" : ((strcmp(op, "++") == 0) ? "concat" : ""))))));  }
const char* c_store(Expr l, Expr r) { Expr holder = (*(Expr*)l.children.data[0]); const char* el = c_elem_of(holder.ty); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", emit_expr(holder)), ".data[", 1), emit_expr((*(Expr*)l.children.data[1])), 1), "] =", 1), maca_cat(" ", c_cell(el, r)), 3);  }
const char* emit_match(Expr e) { return emit_arms(emit_expr((*(Expr*)e.children.data[0])), e.children, 1, c_elem_of((*(Expr*)e.children.data[0]).ty), (*(Expr*)e.children.data[0]).ty);  }
const char* emit_match_stmt(Expr e) { return stmt_arms(emit_expr((*(Expr*)e.children.data[0])), e.children, 1, any_bound(e.children, 1), c_elem_of((*(Expr*)e.children.data[0]).ty), (*(Expr*)e.children.data[0]).ty);  }
const char* stmt_arms(const char* scrut, MacaList cs, long i, long tagged, const char* el, const char* sum) { return (((i + 1) >= (cs.len)) ? "" : (((strcmp((*(Expr*)cs.data[i]).text, "_") == 0) && ((*(Expr*)cs.data[i]).kind != EGuard)) ? maca_cat_own(maca_cat("{ ", emit_expr((*(Expr*)cs.data[(i + 1)]))), "; }", 1) : ({ const char* head = maca_cat_own(maca_cat_own(maca_cat("if (", test(scrut, (*(Expr*)cs.data[i]), tagged, el)), ")", 1), maca_cat_own(maca_cat(" { ", arm_body(scrut, (*(Expr*)cs.data[i]), (*(Expr*)cs.data[(i + 1)]), el, sum)), "; }", 1), 3); const char* rest = stmt_arms(scrut, cs, (i + 2), tagged, el, sum); ((strcmp(rest, "") == 0) ? head : maca_cat_own(maca_cat_own(maca_cat("", head), " else ", 1), rest, 1)); })));  }
const char* emit_arms(const char* scrut, MacaList cs, long i, const char* el, const char* sum) { return emit_arms_at(scrut, cs, i, any_bound(cs, 1), el, sum);  }
const char* emit_arms_at(const char* scrut, MacaList cs, long i, long tagged, const char* el, const char* sum) { return (((i + 1) >= (cs.len)) ? "0" : (((strcmp((*(Expr*)cs.data[i]).text, "_") == 0) && ((*(Expr*)cs.data[i]).kind != EGuard)) ? emit_expr((*(Expr*)cs.data[(i + 1)])) : ((((i + 3) >= (cs.len)) && ((*(Expr*)cs.data[i]).kind != EGuard)) ? arm_body(scrut, (*(Expr*)cs.data[i]), (*(Expr*)cs.data[(i + 1)]), el, sum) : ({ const char* cond = test(scrut, (*(Expr*)cs.data[i]), tagged, el); const char* body = arm_body(scrut, (*(Expr*)cs.data[i]), (*(Expr*)cs.data[(i + 1)]), el, sum); const char* rest = emit_arms_at(scrut, cs, (i + 2), tagged, el, sum); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", cond), " ? ", 1), body, 1), " : ", 1), rest, 1), ")", 1); }))));  }
long bound_pat(Expr p) { return ((p.kind == EGuard) ? bound_pat((*(Expr*)p.children.data[0])) : (((is_alt_pat(p) || is_fields_pat(p)) || is_cells_pat(p)) ? 0 : ((p.children.len) > 0)));  }
long any_bound(MacaList cs, long i) { return ((i >= (cs.len)) ? 0 : (bound_pat((*(Expr*)cs.data[i])) ? 1 : any_bound(cs, (i + 2))));  }
long is_alt_pat(Expr p) { return ((p.kind == EBinary) && (strcmp(p.text, "|") == 0));  }
long is_cells_pat(Expr p) { return ((p.kind == EIdent) && ((strcmp(p.text, "[]") == 0) || (strcmp(p.text, "[..]") == 0)));  }
const char* cells_test(const char* scrut, Expr p) { long n = (p.children.len); const char* size = ((strcmp(p.text, "[..]") == 0) ? maca_cat_own(maca_cat_own(maca_cat("", scrut), ".len >= ", 1), maca_int_to_str((n - 1)), 3) : maca_cat_own(maca_cat_own(maca_cat("", scrut), ".len == ", 1), maca_int_to_str(n), 3)); return maca_cat(size, literal_cells(scrut, p, 0));  }
const char* literal_cells(const char* scrut, Expr p, long i) { return ((i >= (p.children.len)) ? "" : (((*(Expr*)p.children.data[i]).kind != EStr) ? literal_cells(scrut, p, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" && strcmp(", c_cell_at(scrut, "str", maca_int_to_str(i))), ",", 1), maca_cat_own(maca_cat(" ", quoted((*(Expr*)p.children.data[i]).text)), ") == 0", 1), 3), maca_cat("", literal_cells(scrut, p, (i + 1))), 3)));  }
const char* bind_cells(const char* scrut, Expr p, const char* el, long i) { return ((i >= (p.children.len)) ? "" : (((*(Expr*)p.children.data[i]).kind == EStr) ? bind_cells(scrut, p, el, (i + 1)) : (((strcmp(p.text, "[..]") == 0) && (i == ((p.children.len) - 1))) ? maca_cat_own(maca_cat_own(maca_cat("MacaList ", c_id((*(Expr*)p.children.data[i]).text)), " =", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" maca_list_slice(", scrut), ", ", 1), maca_int_to_str(i), 3), ", ", 1), scrut, 1), ".len);", 1), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", type_c(el)), " ", 1), c_id((*(Expr*)p.children.data[i]).text), 1), " =", 1), maca_cat_own(maca_cat(" ", c_cell_at(scrut, el, maca_int_to_str(i))), ";", 1), 3), maca_cat(" ", bind_cells(scrut, p, el, (i + 1))), 3))));  }
const char* test(const char* scrut, Expr pat, long tagged, const char* el) { return ((pat.kind == EGuard) ? guard_test(scrut, pat, tagged, el) : (is_alt_pat(pat) ? maca_cat_own(maca_cat("(", test(scrut, (*(Expr*)pat.children.data[0]), tagged, el)), maca_cat_own(maca_cat(" || ", test(scrut, (*(Expr*)pat.children.data[1]), tagged, el)), ")", 1), 3) : (is_cells_pat(pat) ? cells_test(scrut, pat) : ((is_fields_pat(pat) || is_bind_pat(pat)) ? "1" : (((pat.kind == EStr) && (strcmp(el, "") != 0)) ? one_cell_test(scrut, pat, el) : (tagged ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", scrut), ".tag == ", 1), pat.text, 1), "_tag", 1) : ((pat.kind == EStr) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(strcmp(", scrut), ", ", 1), quoted(pat.text), 1), ") == 0)", 1) : maca_cat_own(maca_cat_own(maca_cat("", scrut), " == ", 1), c_id(pat.text), 1))))))));  }
const char* one_cell_test(const char* scrut, Expr pat, const char* el) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", scrut), ".len == 1 && strcmp(", 1), c_cell_at(scrut, el, "0"), 1), ",", 1), maca_cat_own(maca_cat(" ", quoted(pat.text)), ") == 0", 1), 3);  }
const char* guard_test(const char* scrut, Expr pat, long tagged, const char* el) { const char* when = emit_expr((*(Expr*)pat.children.data[1])); return ((strcmp((*(Expr*)pat.children.data[0]).text, "_") == 0) ? when : (is_bind_pat((*(Expr*)pat.children.data[0])) ? maca_cat_own(maca_cat_own(maca_cat("({ ", bind_one(scrut, (*(Expr*)pat.children.data[0]))), when, 1), "; })", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", test(scrut, (*(Expr*)pat.children.data[0]), tagged, el)), " && ", 1), when, 1), ")", 1)));  }
long is_fields_pat(Expr p) { return ((p.kind == EIdent) && (strcmp(p.text, "{}") == 0));  }
long is_bind_pat(Expr p) { return (((((p.kind == EIdent) && c_lower_name(p.text)) && ((p.children.len) == 0)) && (!is_cells_pat(p))) && (!is_fields_pat(p)));  }
long c_lower_name(const char* name) { return (((((int)strlen(name)) > 0) && (isalpha((unsigned char)(maca_str_at(name, 0))[0]) != 0)) && (strcmp(maca_upper(maca_str_at(name, 0)), maca_str_at(name, 0)) != 0));  }
const char* bind_one(const char* scrut, Expr p) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("__typeof__(", scrut), ") ", 1), c_id(p.text), 1), " = ", 1), scrut, 1), "; ", 1);  }
const char* arm_body(const char* scrut, Expr pat, Expr body, const char* arm_elem, const char* sum) { return ((pat.kind == EGuard) ? arm_body(scrut, (*(Expr*)pat.children.data[0]), body, arm_elem, sum) : ((is_cells_pat(pat) && ((pat.children.len) > 0)) ? maca_cat_own(maca_cat_own(maca_cat("({ ", bind_cells(scrut, pat, arm_elem, 0)), emit_expr(body), 1), "; })", 1) : (is_bind_pat(pat) ? maca_cat_own(maca_cat_own(maca_cat("({ ", bind_one(scrut, pat)), emit_expr(body), 1), "; })", 1) : ((((pat.children.len) == 0) || is_alt_pat(pat)) ? emit_expr(body) : (is_fields_pat(pat) ? maca_cat_own(maca_cat_own(maca_cat("({ ", bind_fields(scrut, pat.children, 0)), emit_expr(body), 1), "; })", 1) : maca_cat_own(maca_cat_own(maca_cat("({ ", bind_slots(scrut, pat.children, sum, 0)), emit_expr(body), 1), "; })", 1))))));  }
const char* bind_fields(const char* scrut, MacaList fs, long i) { return ((i >= (fs.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("__typeof__(", scrut), ".", 1), c_id((*(Expr*)fs.data[i]).text), 1), ") ", 1), c_id((*(Expr*)fs.data[i]).text), 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" = ", scrut), ".", 1), c_id((*(Expr*)fs.data[i]).text), 1), ";", 1), 3), maca_cat(" ", bind_fields(scrut, fs, (i + 1))), 3));  }
const char* bind_slots(const char* scrut, MacaList bs, const char* sum, long i) { return ((i >= (bs.len)) ? "" : (((strcmp((*(Expr*)bs.data[i]).ty, sum) == 0) && (strcmp(type_c(sum), sum) == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", sum), " ", 1), c_id((*(Expr*)bs.data[i]).text), 1), " = *(", 1), sum, 1), "*)", 1), scrut, 1), "._", 1), maca_int_to_str(i), 3), ";", 1), maca_cat(" ", bind_slots(scrut, bs, sum, (i + 1))), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("__typeof__(", scrut), "._", 1), maca_int_to_str(i), 3), ") ", 1), c_id((*(Expr*)bs.data[i]).text), 1), " = ", 1), scrut, 1), "._", 1), maca_int_to_str(i), 3), ";", 1), maca_cat(" ", bind_slots(scrut, bs, sum, (i + 1))), 3)));  }
const char* emit_block(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("({ ", block_stmts(e.stmts, 0)), emit_expr((*(Expr*)e.children.data[0])), 1), "; })", 1);  }
const char* block_stmts(MacaList body, long i) { return ((i >= (body.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat("", emit_stmt((*(Stmt*)body.data[i]), 0, "")), " ", 1), block_stmts(body, (i + 1)), 1));  }
const char* emit_with(Expr e) { const char* base = emit_expr((*(Expr*)e.children.data[0])); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ __typeof__(", base), ") _w = ", 1), base, 1), "; ", 1), emit_updates(e.children, 1), 1), "_w; })", 1);  }
const char* emit_updates(MacaList fs, long i) { return ((i >= (fs.len)) ? "" : ({ Expr f = (*(Expr*)fs.data[i]); const char* set = maca_cat_own(maca_cat_own(maca_cat("_w.", c_id((*(Expr*)f.children.data[0]).text)), " =", 1), maca_cat_own(maca_cat(" ", emit_expr((*(Expr*)f.children.data[1]))), "; ", 1), 3); maca_cat(set, emit_updates(fs, (i + 1))); }));  }
const char* emit_lit_fields(MacaList fs, long i) { return maca_list_join(({ MacaList _m = fs; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(emit_lit_field((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* emit_lit_field(Expr f) { return maca_cat_own(maca_cat_own(maca_cat(".", c_id((*(Expr*)f.children.data[0]).text)), " = ", 1), emit_expr((*(Expr*)f.children.data[1])), 1);  }
const char* emit_args(MacaList xs, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(xs, i, (xs.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(emit_expr((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* emit_unit(Expr e) { return maca_cat_own(maca_cat("int main(void) { return ", emit_expr(e)), "; }", 1);  }
long is_fn_type(const char* ty) { return (maca_str_index_of(ty, ") -> ") >= 0);  }
const char* c_decl(const char* ty, const char* name) { return maca_cat_own(maca_cat_own(maca_cat("", type_c(ty)), " ", 1), name, 1);  }
const char* c_ident_value(Expr e) { return ((is_fn_type(e.ty) && (e.ival != 3)) ? maca_cat_own(maca_cat("(MacaFn){ (void*)", c_id(e.text)), ", (MacaList){0, 0} }", 1) : c_id(e.text));  }
const char* c_arg_type(Expr a) { return type_c(a.ty);  }
MacaList c_arg_names(long n, long i, MacaList acc) { return ((i >= n) ? acc : c_arg_names(n, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(maca_cat_own("_a", maca_int_to_str(i), 2))))));  }
const char* c_arg_binds(MacaList tys, MacaList vals, long i) { return ((i >= (tys.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ((const char*)tys.data[i])), " _a", 1), maca_int_to_str(i), 3), " = ", 1), ((const char*)vals.data[i]), 1), "; ", 1), c_arg_binds(tys, vals, (i + 1)), 1));  }
const char* c_call_value(const char* fnv, const char* rt, MacaList tys, MacaList vals) { MacaList names = c_arg_names((tys.len), 0, maca_listv(0)); const char* sep = ", "; const char* thin_ps = (((tys.len) == 0) ? "void" : maca_list_join(tys, sep)); const char* fat_ps = maca_list_join(maca_list_cat(maca_listv(1, (long)("MacaList")), tys), sep); const char* fat = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", rt), "(*)(", 1), fat_ps, 1), "))_c.fn)(", 1), maca_list_join(maca_list_cat(maca_listv(1, (long)("_c.env")), names), sep), 3), ")", 1); const char* thin = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", rt), "(*)(", 1), thin_ps, 1), "))_c.fn)(", 1), maca_list_join(names, sep), 3), ")", 1); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("({ MacaFn _c = ", fnv), "; ", 1), c_arg_binds(tys, vals, 0), 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_c.env.len ? ", fat), " : ", 1), thin, 1), "; })", 1), 3);  }
const char* c_indirect(const char* fnv, const char* rt, MacaList args) { MacaList tys = ({ MacaList _m = args; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(c_arg_type((*(Expr*)_m.data[_i]))); _r; }); MacaList vals = ({ MacaList _m = args; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(emit_expr((*(Expr*)_m.data[_i]))); _r; }); return c_call_value(fnv, rt, tys, vals);  }
const char* c_closure(Expr e) { MacaList caps = maca_list_slice(e.children, 1, (e.children.len)); const char* env = (((caps.len) == 0) ? "maca_listv(0)" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("maca_listv(", maca_int_to_str((caps.len)), 2), ", ", 1), c_cap_cells(caps, 0), 1), ")", 1)); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(MacaFn){ (void*)", c_id((*(Expr*)e.children.data[0]).text)), ", ", 1), env, 1), " }", 1);  }
const char* c_cap_cells(MacaList caps, long i) { return ((i >= ((caps.len) - 1)) ? c_cell_of((*(Expr*)caps.data[i]).ty, emit_expr((*(Expr*)caps.data[i]))) : maca_cat_own(maca_cat_own(maca_cat("", c_cell_of((*(Expr*)caps.data[i]).ty, emit_expr((*(Expr*)caps.data[i])))), ", ", 1), c_cap_cells(caps, (i + 1)), 1));  }
long is_closure_fn(const char* name) { return ((((int)strlen(name)) > 13) && (strcmp(maca_str_slice(name, 0, 13), "maca_closure_") == 0));  }
const char* c_sig_params(Stmt s) { const char* ps = c_params(s.params, 0); return ((!is_closure_fn(s.name)) ? ps : ((strcmp(ps, "") == 0) ? "MacaList _maca_env" : maca_cat("MacaList _maca_env, ", ps)));  }
long is_map_type(const char* ty) { return ((((int)strlen(ty)) > 4) && (strcmp(maca_str_slice(ty, 0, 4), "Map ") == 0));  }
const char* type_c(const char* ty) { return (is_fn_type(ty) ? "MacaFn" : (is_list_type(ty) ? "MacaList" : (is_map_type(ty) ? "MacaMap" : ((strcmp(ty, "float") == 0) ? "double" : ((strcmp(ty, "Future") == 0) ? "MacaFuture*" : (((strcmp(ty, "str") == 0) || (strcmp(ty, "Element") == 0)) ? "const char*" : (c_sized_float(ty) ? ((strcmp(ty, "f32") == 0) ? "float" : "double") : (((((strcmp(ty, "") == 0) || (strcmp(ty, "int") == 0)) || (strcmp(ty, "bool") == 0)) || c_sized_number(ty)) ? "long" : ty))))))));  }
const char* emit_fn(Stmt s) { const char* params = c_sig_params(s); const char* made = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_ret(s)), " ", 1), c_name(s), 1), "(", 1), params, 1), ") ", 1), maca_cat_own(maca_cat("{ ", emit_stmts(s.body, 0, s.ret)), " }", 1), 3); return maca_cat(made, c_main_shim(s));  }
long c_shimmed(Stmt s) { return ((strcmp(s.name, "main") == 0) && ((s.params.len) > 0));  }
const char* c_name(Stmt s) { return (c_shimmed(s) ? "maca_main" : c_id(s.name));  }
const char* c_ret(Stmt s) { return ((((strcmp(s.name, "main") == 0) && (!c_shimmed(s))) && (strcmp(type_c(s.ret), "long") == 0)) ? "int" : type_c(s.ret));  }
const char* c_main_shim(Stmt s) { return (c_shimmed(s) ? maca_cat("\nint main(int argc, char** argv)", " { return maca_main(maca_args(argc, argv)); }") : "");  }
const char* c_params(MacaList ps, long i) { return ((i >= (ps.len)) ? "" : ((i == ((ps.len) - 1)) ? emit_param((*(Expr*)ps.data[i])) : maca_cat_own(maca_cat_own(maca_cat("", emit_param((*(Expr*)ps.data[i]))), ", ", 1), c_params(ps, (i + 1)), 1)));  }
const char* emit_param(Expr p) { return c_decl(p.ty, c_id(p.text));  }
const char* emit_stmts(MacaList body, long i, const char* ret) { return ((i >= (body.len)) ? "" : ({ const char* here = emit_stmt((*(Stmt*)body.data[i]), (i == ((body.len) - 1)), ret); maca_cat_own(maca_cat_own(maca_cat("", here), " ", 1), emit_stmts(body, (i + 1), ret), 1); }));  }
const char* bind_c_decl(Stmt s) { const char* held = ((strcmp(s.ret, "") != 0) ? s.ret : s.value.ty); return (is_fn_type(held) ? c_decl(held, c_id(s.name)) : maca_cat_own(maca_cat_own(maca_cat("", bind_c_type(s)), " ", 1), c_id(s.name), 1));  }
const char* bind_c_type(Stmt s) { return ((strcmp(s.ret, "") != 0) ? type_c(s.ret) : local_c_type(s.value));  }
const char* local_c_type(Expr e) { return ((strcmp(e.ty, "") != 0) ? type_c(e.ty) : guessed_c_type(e));  }
const char* guessed_c_type(Expr e) { return (e.kind == EStr ? "const char*" : (e.kind == EFloat ? "double" : (e.kind == ERecord ? e.text : (e.kind == EList ? "MacaList" : (e.kind == EBinary ? concat_c_type(e) : (e.kind == EMethod ? method_c_type(e) : (e.kind == ECall ? call_c_type(e) : "long")))))));  }
const char* call_c_type(Expr e) { return (c_str_call(e.text) ? "const char*" : "long");  }
long c_str_call(const char* name) { return ((((((((strcmp(name, "read_file") == 0) || (strcmp(name, "capture") == 0)) || (strcmp(name, "capture_err") == 0)) || (strcmp(name, "env") == 0)) || (strcmp(name, "cwd") == 0)) || (strcmp(name, "real_path") == 0)) || (strcmp(name, "input") == 0)) || (strcmp(name, "chr") == 0));  }
const char* method_c_type(Expr e) { return (is_vector_type((*(Expr*)e.children.data[0]).text) ? (*(Expr*)e.children.data[0]).text : (((strcmp(e.text, "sum") == 0) && is_vector_type((*(Expr*)e.children.data[0]).ty)) ? c_vec_scalar((*(Expr*)e.children.data[0]).ty) : (c_listy_method(e.text) ? "MacaList" : (c_texty_method(e.text) ? "const char*" : "long"))));  }
long c_listy_method(const char* name) { return (((((((((((strcmp(name, "chars") == 0) || (strcmp(name, "split") == 0)) || (strcmp(name, "map") == 0)) || (strcmp(name, "filter") == 0)) || (strcmp(name, "pop") == 0)) || (strcmp(name, "reverse") == 0)) || (strcmp(name, "sort") == 0)) || (strcmp(name, "set") == 0)) || (strcmp(name, "insert") == 0)) || (strcmp(name, "remove") == 0)) || (strcmp(name, "parallel") == 0));  }
long c_texty_method(const char* name) { return ((((((((((strcmp(name, "upper") == 0) || (strcmp(name, "lower") == 0)) || (strcmp(name, "trim") == 0)) || (strcmp(name, "replace") == 0)) || (strcmp(name, "repeat") == 0)) || (strcmp(name, "substr") == 0)) || (strcmp(name, "pad_start") == 0)) || (strcmp(name, "pad_end") == 0)) || (strcmp(name, "fixed") == 0)) || (strcmp(name, "pad_center") == 0));  }
const char* concat_c_type(Expr e) { return ((strcmp(e.text, "..") == 0) ? "MacaList" : ((strcmp(e.text, "++") == 0) ? "const char*" : "long"));  }
const char* emit_stmt(Stmt s, long is_last, const char* ret) { return ((s.kind == SSet) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(s.name)), " = ", 1), emit_expr(s.value), 1), ";", 1) : ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", bind_c_decl(s)), " = ", 1), emit_expr(s.value), 1), ";", 1) : (is_loop(s.value) ? emit_loop(s.value) : (((s.value.kind == EIf) && (!is_last)) ? emit_if_stmt(s.value) : (((s.value.kind == EMatch) && (!is_last)) ? emit_match_stmt(s.value) : (((s.value.kind == EIf) && is_missing_else((*(Expr*)s.value.children.data[2]))) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", emit_if_stmt(s.value)), " return ", 1), c_zero(ret), 1), ";", 1) : ((s.value.kind == EJump) ? maca_cat_own(maca_cat("", emit_jump(s.value, ret)), ";", 1) : ((is_last && is_raise(s.value)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", emit_expr(s.value)), "; return ", 1), c_zero(ret), 1), ";", 1) : (is_last ? maca_cat_own(maca_cat("return ", emit_expr(s.value)), ";", 1) : maca_cat_own(maca_cat("", emit_expr(s.value)), ";", 1))))))))));  }
const char* emit_if_stmt(Expr e) { const char* cond = emit_expr((*(Expr*)e.children.data[0])); const char* then = c_branch((*(Expr*)e.children.data[1])); Expr els = (*(Expr*)e.children.data[2]); return (is_missing_else(els) ? maca_cat_own(maca_cat_own(maca_cat("if (", cond), ") ", 1), then, 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("if (", cond), ") ", 1), then, 1), " else ", 1), c_branch(els), 1));  }
long is_missing_else(Expr e) { return ((e.kind == EIdent) && (strcmp(e.text, "?") == 0));  }
const char* c_branch(Expr e) { return ((e.kind == EIf) ? emit_if_stmt(e) : ((e.kind == EBlock) ? maca_cat_own(maca_cat_own(maca_cat("{ ", block_stmts(e.stmts, 0)), emit_expr((*(Expr*)e.children.data[0])), 1), "; }", 1) : maca_cat_own(maca_cat("{ ", emit_expr(e)), "; }", 1)));  }
long is_raise(Expr e) { return ((e.kind == EUnary) && (strcmp(e.text, "fail") == 0));  }
const char* c_zero(const char* ty) { return (is_fn_type(ty) ? "0" : (((((strcmp(ty, "") == 0) || (strcmp(ty, "int") == 0)) || (strcmp(ty, "bool") == 0)) || (strcmp(ty, "float") == 0)) ? "0" : ((strcmp(ty, "str") == 0) ? "(const char*)0" : maca_cat_own(maca_cat("(", type_c(ty)), "){0}", 1))));  }
const char* c_preamble() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#include <string.h>\n#include <stdlib.h>\n#include <stdio.h>\n#include <stdarg.h>\n#include <ctype.h>\n#include <unistd.h>\n#include <sys/stat.h>\n#include <dirent.h>\n#include <time.h>\n#include <stdint.h>\n#include <strings.h>\n#include <sys/time.h>\n", "typedef float f32x4 __attribute__((vector_size(16)));\ntypedef float f32x8 __attribute__((vector_size(32)));\ntypedef double f64x2 __attribute__((vector_size(16)));\ntypedef double f64x4 __attribute__((vector_size(32)));\ntypedef int i32x4 __attribute__((vector_size(16)));\ntypedef int i32x8 __attribute__((vector_size(32)));\n"), "typedef struct maca_hdr { long size; long used; } maca_hdr;\nstatic long maca_allocs = 0;\n", 1), "static long* maca_cells(long bytes) { long n = bytes > 0 ? bytes : (long)sizeof(long); maca_hdr* h = (maca_hdr*)malloc(sizeof(maca_hdr) + (size_t)n); if (!h) { fputs(\"out of memory\\n\", stderr); exit(1); } h->size = n; h->used = n / (long)sizeof(long); maca_allocs++; return (long*)(h + 1); }\n", 1), "static long maca_alloc_count(void) { return maca_allocs; }\nstatic long maca_reuse_count(void) { return 0; }\n", 1), "static char* maca_cat(const char* a, const char* b) { char* r = malloc(strlen(a) + strlen(b) + 1); strcpy(r, a); strcat(r, b); return r; }\n", 1), "static void maca_drop_str(const char* s) { free((void*)(uintptr_t)s); }\n", 1), "static char* maca_cat_own(const char* a, const char* b, int own) { char* r = maca_cat(a, b); if (own & 1) maca_drop_str(a); if (own & 2) maca_drop_str(b); return r; }\n", 1), "static int maca_say(FILE* f, const char* s, const char* end, int own) { fprintf(f, \"%s%s\", s ? s : \"\", end); if (own) maca_drop_str(s); return 0; }\n", 1), "static char* maca_int_to_str(long n) { char* r = malloc(24); snprintf(r, 24, \"%ld\", n); return r; }\n", 1), "static char* maca_float_to_str(double x) { char* r = malloc(32); if (x == (double)(long long)x && x < 1e15 && x > -1e15) snprintf(r, 32, \"%.1f\", x); else snprintf(r, 32, \"%g\", x); return r; }\n", 1), "static char* maca_fixed(double x, int n) { if (n < 0) n = 0; if (n > 17) n = 17; int need = snprintf(NULL, 0, \"%.*f\", n, x); char* r = malloc((size_t)need + 1); snprintf(r, (size_t)need + 1, \"%.*f\", n, x); return r; }\n", 1), "static const char* maca_bool_to_str(int b) { return b ? \"true\" : \"false\"; }\n", 1), "static char* maca_upper(const char* s) { size_t n = strlen(s); char* r = malloc(n + 1); for (size_t i = 0; i < n; i++) r[i] = toupper((unsigned char)s[i]); r[n] = 0; return r; }\n", 1), "typedef struct { long* data; int len; } MacaList;\n", 1), "typedef struct { void* fn; MacaList env; } MacaFn;\n", 1), "typedef struct { MacaList keys; MacaList vals; } MacaMap;\n", 1), "static MacaMap maca_map_new(void) { MacaMap m; m.keys.data = NULL; m.keys.len = 0; m.vals.data = NULL; m.vals.len = 0; return m; }\n", 1), "static int maca_map_at(MacaMap m, const char* k) { for (int i = 0; i < m.keys.len; i++) if (strcmp((const char*)m.keys.data[i], k) == 0) return i; return -1; }\n", 1), "static int maca_map_has(MacaMap m, const char* k) { return maca_map_at(m, k) >= 0; }\n", 1), "static MacaList maca_list_sorted(MacaList a, int kind);\nstatic MacaList maca_map_keys(MacaMap m) { return maca_list_sorted(m.keys, 1); }\n", 1), "static MacaList maca_map_vals(MacaMap m) { return m.vals; }\n", 1), "static long maca_map_or(MacaMap m, const char* k, long d) { int i = maca_map_at(m, k); return i < 0 ? d : m.vals.data[i]; }\n", 1), "static long maca_map_get(MacaMap m, const char* k) { return maca_map_or(m, k, 0); }\n", 1), "static MacaMap maca_map_remove(MacaMap m, const char* k) { int at = maca_map_at(m, k); if (at < 0) return m; MacaMap r; r.keys.len = m.keys.len - 1; r.vals.len = r.keys.len; r.keys.data = maca_cells((r.keys.len ? r.keys.len : 1) * sizeof(long)); r.vals.data = maca_cells((r.vals.len ? r.vals.len : 1) * sizeof(long)); int w = 0; for (int i = 0; i < m.keys.len; i++) { if (i == at) continue; r.keys.data[w] = m.keys.data[i]; r.vals.data[w] = m.vals.data[i]; w++; } return r; }\n", 1), "static MacaMap maca_map_set(MacaMap m, const char* k, long v) { int i = maca_map_at(m, k); MacaMap r; if (i >= 0) { r = m; r.vals.data = maca_cells((m.vals.len ? m.vals.len : 1) * sizeof(long)); memcpy(r.vals.data, m.vals.data, m.vals.len * sizeof(long)); r.vals.data[i] = v; return r; } r.keys.len = m.keys.len + 1; r.vals.len = m.vals.len + 1; r.keys.data = maca_cells(r.keys.len * sizeof(long)); r.vals.data = maca_cells(r.vals.len * sizeof(long)); memcpy(r.keys.data, m.keys.data, m.keys.len * sizeof(long)); memcpy(r.vals.data, m.vals.data, m.vals.len * sizeof(long)); r.keys.data[m.keys.len] = (long)k; r.vals.data[m.vals.len] = v; return r; }\n", 1), "static MacaList maca_listv(int n, ...) { MacaList l; l.data = maca_cells(n * sizeof(long)); l.len = n; va_list ap; va_start(ap, n); for (int i = 0; i < n; i++) l.data[i] = va_arg(ap, long); va_end(ap); return l; }\n", 1), "static MacaList maca_list_cat(MacaList a, MacaList b) { MacaList l; l.len = a.len + b.len; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data, a.len * sizeof(long)); memcpy(l.data + a.len, b.data, b.len * sizeof(long)); return l; }\n", 1), "static MacaList maca_list_pushed(MacaList a, long v) { maca_hdr* h = a.data ? ((maca_hdr*)a.data) - 1 : 0; long room = h ? h->size / (long)sizeof(long) : 0; if (!h || a.len != h->used || a.len >= room) { long want = a.len * 2 > 8 ? a.len * 2 : 8; long* g = maca_cells(want * (long)sizeof(long)); if (a.len > 0) memcpy(g, a.data, (size_t)a.len * sizeof(long)); a.data = g; h = ((maca_hdr*)g) - 1; } a.data[a.len] = v; a.len++; h->used = a.len; return a; }\n", 1), "static MacaList maca_list_slice(MacaList a, int lo, int hi) { if (lo < 0) lo = 0; if (hi > a.len) hi = a.len; if (hi < lo) hi = lo; MacaList l; l.len = hi - lo; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data + lo, l.len * sizeof(long)); return l; }\n", 1), "static int maca_list_index_of(MacaList a, long v) { for (int i = 0; i < a.len; i++) if (a.data[i] == v) return i; return -1; }\n", 1), "static int maca_list_index_of_str(MacaList a, const char* v) { for (int i = 0; i < a.len; i++) if (strcmp((const char*)a.data[i], v) == 0) return i; return -1; }\n", 1), "static int maca_str_index_of(const char* a, const char* b) { const char* p = strstr(a, b); return p ? (int)(p - a) : -1; }\n", 1), "static char* maca_str_slice(const char* s, int from, int to) { int n = (int)strlen(s); if (from < 0) from = 0; if (to > n) to = n; if (to < from) to = from; int m = to - from; char* r = malloc(m + 1); memcpy(r, s + from, m); r[m] = 0; return r; }\n", 1), "static int maca_ends_with(const char* s, const char* suf) { size_t n = strlen(s), m = strlen(suf); return m <= n && memcmp(s + n - m, suf, m) == 0; }\n", 1), "static char* maca_list_join(MacaList a, const char* sep) { size_t n = 1; for (int i = 0; i < a.len; i++) n += strlen((const char*)a.data[i]) + strlen(sep); char* r = malloc(n); r[0] = 0; for (int i = 0; i < a.len; i++) { if (i) strcat(r, sep); strcat(r, (const char*)a.data[i]); } return r; }\n", 1), "static long maca_box(int n, const void* p) { void* r = malloc(n); memcpy(r, p, n); return (long)r; }\n", 1), "static MacaList maca_chars(const char* s) { int n = (int)strlen(s); MacaList l; l.len = n; l.data = maca_cells((n ? n : 1) * sizeof(long)); for (int i = 0; i < n; i++) { char* c = malloc(2); c[0] = s[i]; c[1] = 0; l.data[i] = (long)c; } return l; }\n", 1), "static MacaList maca_args(int argc, char** argv) { MacaList l; l.len = argc - 1; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); for (int i = 0; i < l.len; i++) l.data[i] = (long)argv[i + 1]; return l; }\n", 1), "static char* maca_read_file(const char* path) { struct stat st; if (stat(path, &st) != 0 || !S_ISREG(st.st_mode)) { char* e = malloc(1); e[0] = 0; return e; } FILE* f = fopen(path, \"rb\"); if (!f) { char* e = malloc(1); e[0] = 0; return e; } fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET); char* r = malloc(n + 1); size_t got = fread(r, 1, n, f); r[got] = 0; fclose(f); return r; }\n", 1), "static int maca_write_file(const char* path, const char* text) { FILE* f = fopen(path, \"wb\"); if (!f) return 0; fputs(text, f); fclose(f); return 1; }\n", 1), "static char* maca_str_at(const char* s, int i) { char* r = malloc(2); r[0] = (i >= 0 && i < (int)strlen(s)) ? s[i] : 0; r[1] = 0; return r; }\n", 1), "static MacaList maca_range(int lo, int hi) { MacaList l; l.len = hi > lo ? hi - lo : 0; l.data = maca_cells((l.len ? l.len : 1) * sizeof(long)); for (int i = 0; i < l.len; i++) l.data[i] = lo + i; return l; }\n", 1), "static MacaList maca_list_reverse(MacaList a) { MacaList l = maca_list_slice(a, 0, a.len); for (int i = 0; i < l.len / 2; i++) { long t = l.data[i]; l.data[i] = l.data[l.len - 1 - i]; l.data[l.len - 1 - i] = t; } return l; }\n", 1), "static int maca_cmp_cell(const void* a, const void* b) { long x = *(const long*)a, y = *(const long*)b; return (x > y) - (x < y); }\n", 1), "static int maca_cmp_cell_str(const void* a, const void* b) { return strcmp((const char*)*(const long*)a, (const char*)*(const long*)b); }\n", 1), "static int maca_cmp_cell_float(const void* a, const void* b) { double x = *(double*)*(const long*)a, y = *(double*)*(const long*)b; return (x > y) - (x < y); }\n", 1), "static MacaList maca_list_sorted(MacaList a, int kind) { MacaList l = maca_list_slice(a, 0, a.len); if (l.len > 1) qsort(l.data, (size_t)l.len, sizeof(long), kind == 1 ? maca_cmp_cell_str : kind == 2 ? maca_cmp_cell_float : maca_cmp_cell); return l; }\n", 1), "static MacaList maca_list_set(MacaList a, int at, long v) { MacaList l = maca_list_slice(a, 0, a.len); if (at >= 0 && at < l.len) l.data[at] = v; return l; }\n", 1), "static MacaList maca_list_insert(MacaList a, int at, long v) { if (at < 0) at = 0; if (at > a.len) at = a.len; MacaList l; l.len = a.len + 1; l.data = maca_cells((size_t)l.len * sizeof(long)); memcpy(l.data, a.data, (size_t)at * sizeof(long)); l.data[at] = v; memcpy(l.data + at + 1, a.data + at, (size_t)(a.len - at) * sizeof(long)); return l; }\n", 1), "static MacaList maca_list_remove(MacaList a, int at) { if (at < 0 || at >= a.len) return maca_list_slice(a, 0, a.len); MacaList l; l.len = a.len - 1; l.data = maca_cells((size_t)(l.len ? l.len : 1) * sizeof(long)); memcpy(l.data, a.data, (size_t)at * sizeof(long)); memcpy(l.data + at, a.data + at + 1, (size_t)(l.len - at) * sizeof(long)); return l; }\n", 1), "static char* maca_trim(const char* s) { const char* a = s; while (*a == ' ' || *a == '\\t' || *a == '\\n' || *a == '\\r') a++; const char* b = s + strlen(s); while (b > a && (b[-1] == ' ' || b[-1] == '\\t' || b[-1] == '\\n' || b[-1] == '\\r')) b--; size_t n = (size_t)(b - a); char* r = malloc(n + 1); memcpy(r, a, n); r[n] = 0; return r; }\n", 1), "static char* maca_lower(const char* s) { size_t n = strlen(s); char* r = malloc(n + 1); for (size_t i = 0; i < n; i++) r[i] = tolower((unsigned char)s[i]); r[n] = 0; return r; }\n", 1), "static int maca_starts_with(const char* s, const char* p) { size_t n = strlen(p); return strlen(s) >= n && memcmp(s, p, n) == 0; }\n", 1), "static char* maca_substr(const char* s, int at, int n) { return maca_str_slice(s, at, at + (n < 0 ? 0 : n)); }\n", 1), "static char* maca_repeat(const char* s, int n) { if (n < 0) n = 0; size_t len = strlen(s), total = len * (size_t)n; char* r = malloc(total + 1); for (int i = 0; i < n; i++) memcpy(r + (size_t)i * len, s, len); r[total] = 0; return r; }\n", 1), "static char* maca_replace(const char* s, const char* from, const char* to) { if (!*from) return maca_cat(s, \"\"); size_t lf = strlen(from), lt = strlen(to), hits = 0; for (const char* p = strstr(s, from); p; p = strstr(p + lf, from)) hits++; char* r = malloc(strlen(s) + hits * (lt > lf ? lt - lf : 0) + 1); char* w = r; const char* p = s; for (;;) { const char* hit = strstr(p, from); if (!hit) { strcpy(w, p); break; } memcpy(w, p, (size_t)(hit - p)); w += hit - p; memcpy(w, to, lt); w += lt; p = hit + lf; } return r; }\n", 1), "static char* maca_pad(const char* s, int w, const char* p, int at_start) { if (!*p) p = \" \"; size_t len = strlen(s); if (w <= 0 || (size_t)w <= len) return maca_cat(s, \"\"); size_t fill = (size_t)w - len, pl = strlen(p); char* r = malloc((size_t)w + 1); char* into = at_start ? r : r + len; for (size_t i = 0; i < fill; i++) into[i] = p[i % pl]; memcpy(at_start ? r + fill : r, s, len); r[(size_t)w] = 0; return r; }\n", 1), "static char* maca_pad_start(const char* s, int w, const char* p) { return maca_pad(s, w, p, 1); }\nstatic char* maca_pad_end(const char* s, int w, const char* p) { return maca_pad(s, w, p, 0); }\nstatic char* maca_pad_center(const char* s, int w, const char* p) { if (!*p) p = \" \"; size_t len = strlen(s); if (w <= 0 || (size_t)w <= len) return maca_cat(s, \"\"); size_t fill = (size_t)w - len, left = fill / 2, pl = strlen(p); char* r = malloc((size_t)w + 1); for (size_t i = 0; i < left; i++) r[i] = p[i % pl]; memcpy(r + left, s, len); for (size_t i = left + len; i < (size_t)w; i++) r[i] = p[(i - left - len) % pl]; r[(size_t)w] = 0; return r; }\n", 1), "static MacaList maca_split(const char* s, const char* sep) { MacaList l; int cap = 8; l.len = 0; l.data = maca_cells(cap * sizeof(long)); if (!*sep) { l.data[l.len++] = (long)maca_cat(s, \"\"); return l; } size_t ls = strlen(sep); const char* p = s; for (;;) { if (l.len == cap) { cap *= 2; long* d = maca_cells(cap * sizeof(long)); memcpy(d, l.data, (size_t)l.len * sizeof(long)); l.data = d; } const char* hit = strstr(p, sep); if (!hit) { l.data[l.len++] = (long)maca_str_slice(p, 0, (int)strlen(p)); break; } l.data[l.len++] = (long)maca_str_slice(p, 0, (int)(hit - p)); p = hit + ls; } return l; }\n", 1), "static int maca_failed_count = 0;\nstatic int maca_failures(void) { return maca_failed_count; }\n", 1), "static int maca_assert(int cond, const char* msg) { if (cond) return 1; maca_failed_count++; fprintf(stderr, \"assertion failed: %s\\n\", msg && *msg ? msg : \"(no message)\"); return 0; }\n", 1), "static int maca_assert_eq(const char* got, const char* want, const char* msg) { if (!got) got = \"\"; if (!want) want = \"\"; if (strcmp(got, want) == 0) return 1; maca_failed_count++; fprintf(stderr, \"assertion failed: %s\\n  got:  %s\\n  want: %s\\n\", msg && *msg ? msg : \"(no message)\", got, want); return 0; }\n", 1), "static char* maca_chr(int b) { char* r = malloc(2); r[0] = (b > 0 && b < 256) ? (char)b : 0; r[1] = 0; return r; }\nstatic int maca_ord(const char* s) { return (s && s[0]) ? (int)(unsigned char)s[0] : -1; }\n", 1), "static char* maca_env(const char* name) { const char* v = getenv(name); return maca_cat(v ? v : \"\", \"\"); }\nstatic char* maca_cwd(void) { char* r = malloc(4096); if (!getcwd(r, 4096)) r[0] = 0; return r; }\n", 1), "static int maca_chdir(const char* p) { return chdir(p) == 0; }\nstatic int maca_is_tty(void) { return isatty(1); }\n", 1), "static int maca_file_exists(const char* p) { struct stat st; return stat(p, &st) == 0; }\nstatic int maca_is_dir(const char* p) { struct stat st; return stat(p, &st) == 0 && S_ISDIR(st.st_mode); }\n", 1), "static long maca_file_size(const char* p) { struct stat st; return stat(p, &st) == 0 ? (long)st.st_size : -1; }\nstatic long maca_modified_ms(const char* p) { struct stat st; return stat(p, &st) == 0 ? (long)(st.st_mtime * 1000) : -1; }\n", 1), "static int maca_make_dir(const char* p) { char* d = maca_cat(p, \"\"); for (char* q = d + 1; *q; q++) if (*q == '/') { *q = 0; mkdir(d, 0777); *q = '/'; } mkdir(d, 0777); return maca_is_dir(d); }\n", 1), "static int maca_remove_file(const char* p) { return unlink(p) == 0; }\nstatic int maca_remove_dir(const char* p) { DIR* d = opendir(p); if (!d) return 0; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, \".\") == 0 || strcmp(it->d_name, \"..\") == 0) continue; char* c = maca_cat(p, maca_cat(\"/\", it->d_name)); if (maca_is_dir(c)) maca_remove_dir(c); else maca_remove_file(c); } closedir(d); return rmdir(p) == 0; }\n", 1), "static int maca_copy_bytes(const char* src, const char* dst) { FILE* a = fopen(src, \"rb\"); if (!a) return 0; FILE* b = fopen(dst, \"wb\"); if (!b) { fclose(a); return 0; } char buf[8192]; size_t n; while ((n = fread(buf, 1, sizeof buf, a)) > 0) fwrite(buf, 1, n, b); fclose(a); fclose(b); return 1; }\n", 1), "static MacaList maca_list_dir(const char* p) { MacaList l; l.len = 0; int cap = 16; l.data = maca_cells((size_t)cap * sizeof(long)); DIR* d = opendir(p); if (!d) return l; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, \".\") == 0 || strcmp(it->d_name, \"..\") == 0) continue; if (l.len == cap) { cap *= 2; long* g = maca_cells((size_t)cap * sizeof(long)); memcpy(g, l.data, (size_t)l.len * sizeof(long)); l.data = g; } l.data[l.len++] = (long)maca_cat(it->d_name, \"\"); } closedir(d); if (l.len > 1) qsort(l.data, (size_t)l.len, sizeof(long), maca_cmp_cell_str); return l; }\n", 1), "static char* maca_real_path(const char* p) { char* r = malloc(4096); if (!realpath(p, r)) return maca_cat(p, \"\"); return r; }\n", 1), "static char* maca_path_join(const char* a, const char* b) { if (!*a) return maca_cat(b, \"\"); if (!*b) return maca_cat(a, \"\"); return a[strlen(a) - 1] == '/' ? maca_cat(a, b) : maca_cat(a, maca_cat(\"/\", b)); }\n", 1), "static long maca_now_ms(void) { struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts); return (long)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000); }\n", 1), "static char* maca_now_iso(void) { time_t t = time(NULL); struct tm g; gmtime_r(&t, &g); char* r = malloc(32); strftime(r, 32, \"%Y-%m-%dT%H:%M:%SZ\", &g); return r; }\n", 1), "static char* maca_format_time(long ms, const char* fmt) { time_t t = (time_t)(ms / 1000); struct tm g; gmtime_r(&t, &g); char* r = malloc(128); if (!strftime(r, 128, fmt, &g)) r[0] = 0; return r; }\n", 1), "static void maca_sleep_ms(int ms) { if (ms > 0) usleep((unsigned)ms * 1000); }\n", 1), "static char* maca_input(const char* prompt) { if (prompt && *prompt) { printf(\"%s\", prompt); fflush(stdout); } size_t cap = 128, n = 0; char* b = malloc(cap); int c; while ((c = fgetc(stdin)) != EOF && c != '\\n') { if (n + 1 >= cap) { cap *= 2; char* g = malloc(cap); memcpy(g, b, n); b = g; } b[n++] = (char)c; } b[n] = 0; return b; }\nstatic int maca_at_eof(void) { int c = fgetc(stdin); if (c == EOF) return 1; ungetc(c, stdin); return 0; }\n", 1), "static char* maca_attr(const char* name, const char* value) { if (!name || !*name) return maca_cat(\"\", \"\"); size_t n = strlen(name), v = strlen(value); char* r = malloc(n + v * 6 + 5); char* w = r; *w++ = ' '; memcpy(w, name, n); w += n; *w++ = '='; *w++ = '\"'; for (size_t i = 0; i < v; i++) { char c = value[i]; if (c == '&') { memcpy(w, \"&amp;\", 5); w += 5; } else if (c == '<') { memcpy(w, \"&lt;\", 4); w += 4; } else if (c == '>') { memcpy(w, \"&gt;\", 4); w += 4; } else if (c == '\"') { memcpy(w, \"&quot;\", 6); w += 6; } else { *w++ = c; } } *w++ = '\"'; *w = 0; return r; }\n", 1), "static char* maca_flag(const char* name, int on) { if (!on || !name || !*name) return maca_cat(\"\", \"\"); return maca_cat(\" \", name); }\n", 1), "static int maca_void_tag(const char* t) { const char* v[] = {\"area\", \"base\", \"br\", \"col\", \"embed\", \"hr\", \"img\", \"input\", \"link\", \"meta\", \"source\", \"track\", \"wbr\", 0}; for (int i = 0; v[i]; i++) if (strcmp(t, v[i]) == 0) return 1; return 0; }\n", 1), "static char* maca_element(const char* tag, const char* attrs, const char* kids) { size_t t = strlen(tag), a = strlen(attrs), k = strlen(kids); char* r = malloc(t * 2 + a + k + 6); char* w = r; *w++ = '<'; memcpy(w, tag, t); w += t; memcpy(w, attrs, a); w += a; *w++ = '>'; if (!maca_void_tag(tag)) { memcpy(w, kids, k); w += k; *w++ = '<'; *w++ = '/'; memcpy(w, tag, t); w += t; *w++ = '>'; } *w = 0; return r; }\n", 1);  }
const char* c_errors(const char* code) { return (c_uses(code, "maca_try_push") ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#include <setjmp.h>\n", "static jmp_buf maca_handlers[256];\nstatic int maca_handler_top = 0;\nstatic const char* maca_caught = \"\";\n"), "static jmp_buf* maca_try_push(void) { return &maca_handlers[maca_handler_top++]; }\n", 1), "static void maca_try_pop(void) { if (maca_handler_top > 0) maca_handler_top--; }\n", 1), "static char* maca_last_fail(void) { return maca_cat(maca_caught, \"\"); }\n", 1), "static int maca_fail(const char* s) { if (maca_handler_top > 0) { maca_caught = s ? s : \"\"; longjmp(maca_handlers[--maca_handler_top], 1); } fprintf(stderr, \"error: %s\\n\", s ? s : \"\"); exit(1); return 0; }\n", 1) : "static int maca_fail(const char* s) { fprintf(stderr, \"error: %s\\n\", s ? s : \"\"); exit(1); return 0; }\n");  }
long c_uses(const char* code, const char* name) { return (maca_str_index_of(code, name) >= 0);  }
const char* c_bare(const char* code) { return maca_list_join(c_unquoted(maca_split(code, "\""), 0, 1, maca_listv(0)), " ");  }
MacaList c_unquoted(MacaList parts, long i, long live, MacaList acc) { return ((i >= (parts.len)) ? acc : ({ const char* part = ((const char*)parts.data[i]); long next = (c_escaped(part) ? live : (!live)); c_unquoted(parts, (i + 1), next, (live ? maca_list_pushed(acc, (long)(part)) : acc)); }));  }
long c_escaped(const char* part) { return ((c_slashes(part, (((int)strlen(part)) - 1), 0) % 2) == 1);  }
long c_slashes(const char* s, long i, long n) { return (((i >= 0) && (strcmp(maca_str_at(s, i), "\\") == 0)) ? c_slashes(s, (i - 1), (n + 1)) : n);  }
const char* c_net_headers() { return maca_cat_own(maca_cat("#include <pthread.h>\n#include <sys/socket.h>\n", "#include <netinet/in.h>\n#include <netinet/tcp.h>\n"), "#include <arpa/inet.h>\n", 1);  }
const char* c_process(const char* code) { return ((c_uses(code, "maca_exec") || c_uses(code, "maca_capture")) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#include <sys/wait.h>\n", "static int maca_exec(const char* cmd, MacaList args) { char** argv = malloc((args.len + 2) * sizeof(char*)); argv[0] = (char*)cmd; for (int i = 0; i < args.len; i++) argv[i + 1] = (char*)args.data[i]; argv[args.len + 1] = NULL; pid_t pid = fork(); if (pid < 0) return -1; if (pid == 0) { execvp(cmd, argv); _exit(127); } int st = 0; if (waitpid(pid, &st, 0) < 0) return -1; if (WIFEXITED(st)) return WEXITSTATUS(st); if (WIFSIGNALED(st)) return 128 + WTERMSIG(st); return -1; }\n"), "static char* maca_capture_fd(const char* cmd, MacaList args, int child_fd) { int fd[2]; if (pipe(fd) < 0) return maca_cat(\"\", \"\"); char** argv = malloc((size_t)(args.len + 2) * sizeof(char*)); argv[0] = (char*)cmd; for (int i = 0; i < args.len; i++) argv[i + 1] = (char*)args.data[i]; argv[args.len + 1] = NULL; pid_t pid = fork(); if (pid < 0) { close(fd[0]); close(fd[1]); return maca_cat(\"\", \"\"); } if (pid == 0) { close(fd[0]); dup2(fd[1], child_fd); close(fd[1]); execvp(cmd, argv); _exit(127); } close(fd[1]); size_t cap = 4096, n = 0; char* b = malloc(cap); ssize_t got; while ((got = read(fd[0], b + n, cap - n - 1)) > 0) { n += (size_t)got; if (n + 1 >= cap) { cap *= 2; char* g = malloc(cap); memcpy(g, b, n); b = g; } } close(fd[0]); int st = 0; waitpid(pid, &st, 0); b[n] = 0; return b; }\n", 1), "static char* maca_capture(const char* cmd, MacaList args) { return maca_capture_fd(cmd, args, 1); }\n", 1), "static char* maca_capture_err(const char* cmd, MacaList args) { return maca_capture_fd(cmd, args, 2); }\n", 1) : "");  }
const char* c_threads(const char* code) { return ((c_uses(code, "maca_spawn") || c_uses(code, "MacaFuture")) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#include <pthread.h>\n", "typedef long (*MacaTask)(long);\ntypedef long (*MacaTask2)(long, long);\n"), "typedef struct { pthread_t th; MacaTask fn; MacaTask2 fn2; long a; long b; long out; int done; } MacaFuture;\n", 1), "static void* maca_task_run(void* p) { MacaFuture* f = (MacaFuture*)p; f->out = f->fn2 ? f->fn2(f->a, f->b) : f->fn(f->a); return NULL; }\n", 1), "static MacaFuture* maca_spawn(MacaTask fn, long a) { MacaFuture* f = malloc(sizeof(MacaFuture)); f->fn = fn; f->fn2 = 0; f->a = a; f->b = 0; f->out = 0; f->done = 0; pthread_create(&f->th, NULL, maca_task_run, f); return f; }\n", 1), "static MacaFuture* maca_spawn2(MacaTask2 fn, long a, long b) { MacaFuture* f = malloc(sizeof(MacaFuture)); f->fn = 0; f->fn2 = fn; f->a = a; f->b = b; f->out = 0; f->done = 0; pthread_create(&f->th, NULL, maca_task_run, f); return f; }\n", 1), "static long maca_await(MacaFuture* f) { if (!f) return 0; if (!f->done) { pthread_join(f->th, NULL); f->done = 1; } return f->out; }\n", 1) : "");  }
const char* c_sockets(const char* code) { return ((c_uses(code, "http_listen") || c_uses(code, "http_fetch")) ? maca_cat(c_net_headers(), c_http()) : "");  }
const char* c_http() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#define MACA_HTTP_MAX (1 << 20)\n", "#define MACA_HTTP_IDLE 30\n"), "typedef const char* (*MacaHttpFn)(const char*);\n", 1), "typedef const char* (*MacaHttpFat)(MacaList, const char*);\n", 1), "typedef struct { int fd; MacaFn fn; } MacaHttpConn;\n", 1), "typedef struct { char* buf; size_t cap, len; } MacaHttpBuf;\n", 1), "static int maca_http_write(int fd, const char* b, size_t n) { size_t off = 0; while (off < n) { ssize_t w = send(fd, b + off, n - off, MSG_NOSIGNAL); if (w <= 0) return -1; off += (size_t)w; } return 0; }\n", 1), "static long maca_http_head_end(const char* b, size_t n) { for (size_t i = 0; i + 3 < n; i++) if (b[i] == '\\r' && b[i+1] == '\\n' && b[i+2] == '\\r' && b[i+3] == '\\n') return (long)(i + 4); for (size_t i = 0; i + 1 < n; i++) if (b[i] == '\\n' && b[i+1] == '\\n') return (long)(i + 2); return -1; }\n", 1), "static long maca_http_length(const char* h, size_t n) { for (size_t i = 0; i + 15 < n; i++) { if (i && h[i-1] != '\\n') continue; if (strncasecmp(h + i, \"content-length:\", 15) != 0) continue; size_t j = i + 15; while (j < n && (h[j] == ' ' || h[j] == '\\t')) j++; if (j >= n || h[j] < '0' || h[j] > '9') return -1; long v = 0; while (j < n && h[j] >= '0' && h[j] <= '9') { if (v > (long)MACA_HTTP_MAX) return -1; v = v * 10 + (h[j] - '0'); j++; } return v; } return 0; }\n", 1), "static int maca_http_chunked(const char* h, size_t n) { for (size_t i = 0; i + 18 < n; i++) { if (i && h[i-1] != '\\n') continue; if (strncasecmp(h + i, \"transfer-encoding:\", 18) == 0) return 1; } return 0; }\n", 1), "static int maca_http_closing(const char* m) { for (const char* p = m; *p; p++) { if (p != m && p[-1] != '\\n') continue; if (p[0] == '\\r' || p[0] == '\\n') break; if (strncasecmp(p, \"connection:\", 11) != 0) continue; const char* v = p + 11; while (*v == ' ' || *v == '\\t') v++; return strncasecmp(v, \"close\", 5) == 0; } return 0; }\n", 1), "static void maca_http_refuse(int fd, const char* line, const char* body) { char out[256]; int n = snprintf(out, sizeof(out), \"HTTP/1.1 %s\\r\\nContent-Type: text/plain; charset=utf-8\\r\\nContent-Length: %d\\r\\nConnection: close\\r\\n\\r\\n%s\", line, (int)strlen(body), body); if (n > 0) maca_http_write(fd, out, (size_t)n); }\n", 1), "static int maca_http_take(int fd, MacaHttpBuf* b, char** out) { for (;;) { long head = maca_http_head_end(b->buf, b->len); if (head >= 0) { if (maca_http_chunked(b->buf, (size_t)head)) return 3; long want = maca_http_length(b->buf, (size_t)head); if (want < 0) return 4; if (head + want > (long)MACA_HTTP_MAX) return 2; if ((long)b->len >= head + want) { size_t take = (size_t)(head + want); char* req = malloc(take + 1); if (!req) return 1; memcpy(req, b->buf, take); req[take] = 0; memmove(b->buf, b->buf + take, b->len - take); b->len -= take; *out = req; return 0; } } if (b->len >= MACA_HTTP_MAX) return 2; if (b->len + 1 >= b->cap) { size_t cap = b->cap ? b->cap * 2 : 8192; char* g = realloc(b->buf, cap); if (!g) return 1; b->buf = g; b->cap = cap; } ssize_t r = recv(fd, b->buf + b->len, b->cap - b->len - 1, 0); if (r <= 0) return 1; b->len += (size_t)r; b->buf[b->len] = 0; } }\n", 1), "static void* maca_http_client(void* arg) { MacaHttpConn* c = (MacaHttpConn*)arg; int fd = c->fd; MacaFn fn = c->fn; free(c); struct timeval tv; tv.tv_sec = MACA_HTTP_IDLE; tv.tv_usec = 0; setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv)); setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &tv, sizeof(tv)); MacaHttpBuf b; b.buf = NULL; b.cap = 0; b.len = 0; for (;;) { char* req = NULL; int st = maca_http_take(fd, &b, &req); if (st == 2) { maca_http_refuse(fd, \"413 Payload Too Large\", \"413 Payload Too Large\\n\"); break; } if (st == 3) { maca_http_refuse(fd, \"501 Not Implemented\", \"501 Not Implemented\\n\"); break; } if (st == 4) { maca_http_refuse(fd, \"400 Bad Request\", \"400 Bad Request\\n\"); break; } if (st != 0) break; int done = maca_http_closing(req); const char* reply = fn.env.len ? ((MacaHttpFat)fn.fn)(fn.env, req) : ((MacaHttpFn)fn.fn)(req); free(req); if (!reply) break; if (maca_http_write(fd, reply, strlen(reply))) break; if (done || maca_http_closing(reply)) break; } free(b.buf); close(fd); return NULL; }\n", 1), "long http_listen(long port) { int srv = socket(AF_INET, SOCK_STREAM, 0); if (srv < 0) return -1; int one = 1; setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one)); struct sockaddr_in a; memset(&a, 0, sizeof(a)); a.sin_family = AF_INET; a.sin_addr.s_addr = INADDR_ANY; a.sin_port = htons((unsigned short)port); if (bind(srv, (struct sockaddr*)&a, sizeof(a)) < 0) { close(srv); return -2; } if (listen(srv, 512) < 0) { close(srv); return -3; } return srv; }\n", 1), "static int maca_http_serve(int srv, MacaFn fn) { if (!fn.fn) return -1; for (;;) { int fd = accept(srv, 0, 0); if (fd < 0) continue; int one = 1; setsockopt(fd, IPPROTO_TCP, TCP_NODELAY, &one, sizeof(one)); MacaHttpConn* c = malloc(sizeof(MacaHttpConn)); if (!c) { close(fd); continue; } c->fd = fd; c->fn = fn; pthread_t th; if (pthread_create(&th, 0, maca_http_client, c) != 0) { free(c); close(fd); continue; } pthread_detach(th); } return 0; }\n", 1), "const char* http_fetch(const char* host, long port, const char* request) { int fd = socket(AF_INET, SOCK_STREAM, 0); if (fd < 0) return \"\"; struct sockaddr_in a; memset(&a, 0, sizeof(a)); a.sin_family = AF_INET; a.sin_port = htons((unsigned short)port); inet_pton(AF_INET, host && *host ? host : \"127.0.0.1\", &a.sin_addr); if (connect(fd, (struct sockaddr*)&a, sizeof(a)) < 0) { close(fd); return \"\"; } if (maca_http_write(fd, request, strlen(request))) { close(fd); return \"\"; } shutdown(fd, SHUT_WR); size_t cap = 8192, n = 0; char* buf = malloc(cap); if (!buf) { close(fd); return \"\"; } for (;;) { if (n + 1 >= cap) { cap *= 2; char* g = realloc(buf, cap); if (!g) { free(buf); close(fd); return \"\"; } buf = g; } ssize_t r = recv(fd, buf + n, cap - n - 1, 0); if (r <= 0) break; n += (size_t)r; } buf[n] = 0; close(fd); return buf; }\n", 1), "long http_accept_loop(long srv, MacaFn handler) { return maca_http_serve(srv, handler); }\n", 1);  }
const char* c_mqtt(const char* protos) { return ((maca_str_index_of(protos, "mqtt_") < 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(c_net_headers(), "#define MACA_MQTT_MAX_SUBS 8192\n"), "typedef struct { int fd; char pat[256]; } MacaMqttSub;\n", 1), "static MacaMqttSub maca_mqtt_subs[MACA_MQTT_MAX_SUBS];\n", 1), "static int maca_mqtt_nsubs = 0;\n", 1), "static pthread_mutex_t maca_mqtt_lock = PTHREAD_MUTEX_INITIALIZER;\n", 1), "static int maca_mqtt_write_all(int fd, const unsigned char* b, int n) { int off = 0; while (off < n) { int w = write(fd, b + off, n - off); if (w <= 0) return -1; off += w; } return 0; }\n", 1), "static int maca_mqtt_read_n(int fd, unsigned char* b, int n) { int off = 0; while (off < n) { int r = read(fd, b + off, n - off); if (r <= 0) return -1; off += r; } return 0; }\n", 1), "static int maca_mqtt_read_remlen(int fd) { int mult = 1, value = 0; unsigned char c; do { if (maca_mqtt_read_n(fd, &c, 1)) return -1; value += (c & 127) * mult; mult *= 128; } while (c & 128); return value; }\n", 1), "static int maca_mqtt_enc_remlen(unsigned char* b, int len) { int i = 0; do { unsigned char c = len % 128; len /= 128; if (len) c |= 128; b[i++] = c; } while (len); return i; }\n", 1), "static int maca_mqtt_put_str(unsigned char* b, const char* s) { int n = (int)strlen(s); b[0] = (n >> 8) & 0xFF; b[1] = n & 0xFF; memcpy(b + 2, s, n); return n + 2; }\n", 1), "static int maca_mqtt_send(int fd, unsigned char type_flags, const unsigned char* body, int blen) { unsigned char hdr[5]; hdr[0] = type_flags; int hn = 1 + maca_mqtt_enc_remlen(hdr + 1, blen); if (maca_mqtt_write_all(fd, hdr, hn)) return -1; if (blen && maca_mqtt_write_all(fd, body, blen)) return -1; return 0; }\n", 1), "static int maca_mqtt_match(const char* f, const char* t) { if (*f == '#') return 1; if (*f == 0 && *t == 0) return 1; if (*f == 0 || *t == 0) return 0; const char* fe = f; while (*fe && *fe != '/') fe++; const char* te = t; while (*te && *te != '/') te++; int fl = (int)(fe - f); int ok = (fl == 1 && f[0] == '+') || ((int)(te - t) == fl && strncmp(f, t, fl) == 0); if (!ok) return 0; return maca_mqtt_match(*fe == '/' ? fe + 1 : fe, *te == '/' ? te + 1 : te); }\n", 1), "static void maca_mqtt_route(const char* topic, const unsigned char* payload, int plen) { pthread_mutex_lock(&maca_mqtt_lock); for (int i = 0; i < maca_mqtt_nsubs; i++) { if (maca_mqtt_match(maca_mqtt_subs[i].pat, topic)) { unsigned char body[1024]; int n = maca_mqtt_put_str(body, topic); if (plen > 0 && n + plen < (int)sizeof(body)) { memcpy(body + n, payload, plen); n += plen; } maca_mqtt_send(maca_mqtt_subs[i].fd, 0x30, body, n); } } pthread_mutex_unlock(&maca_mqtt_lock); }\n", 1), "static void* maca_mqtt_client(void* arg) { int fd = (int)(long)arg; unsigned char b1; while (maca_mqtt_read_n(fd, &b1, 1) == 0) { int rl = maca_mqtt_read_remlen(fd); if (rl < 0 || rl > 65535) break; unsigned char* body = (unsigned char*)malloc(rl > 0 ? rl : 1); if (rl > 0 && maca_mqtt_read_n(fd, body, rl)) { free(body); break; } int type = b1 >> 4; if (type == 1) { unsigned char ack[2] = { 0x00, 0x00 }; maca_mqtt_send(fd, 0x20, ack, 2); } else if (type == 8) { int p = 2; unsigned char subacks[64]; int na = 0; while (p + 2 <= rl) { int tl = (body[p] << 8) | body[p + 1]; p += 2; if (p + tl > rl) break; pthread_mutex_lock(&maca_mqtt_lock); if (maca_mqtt_nsubs < MACA_MQTT_MAX_SUBS && tl < 256) { maca_mqtt_subs[maca_mqtt_nsubs].fd = fd; memcpy(maca_mqtt_subs[maca_mqtt_nsubs].pat, body + p, tl); maca_mqtt_subs[maca_mqtt_nsubs].pat[tl] = 0; maca_mqtt_nsubs++; } pthread_mutex_unlock(&maca_mqtt_lock); p += tl + 1; if (na < 64) subacks[na++] = 0x00; } unsigned char sb[66]; sb[0] = body[0]; sb[1] = body[1]; memcpy(sb + 2, subacks, na); maca_mqtt_send(fd, 0x90, sb, 2 + na); } else if (type == 3) { int tl = (body[0] << 8) | body[1]; char topic[256]; int cl = tl < 255 ? tl : 255; memcpy(topic, body + 2, cl); topic[cl] = 0; int po = 2 + tl; maca_mqtt_route(topic, body + po, rl - po); } else if (type == 12) { maca_mqtt_send(fd, 0xD0, 0, 0); } else if (type == 14) { free(body); break; } free(body); } pthread_mutex_lock(&maca_mqtt_lock); int j = 0; for (int i = 0; i < maca_mqtt_nsubs; i++) if (maca_mqtt_subs[i].fd != fd) maca_mqtt_subs[j++] = maca_mqtt_subs[i]; maca_mqtt_nsubs = j; pthread_mutex_unlock(&maca_mqtt_lock); close(fd); return 0; }\n", 1), "long mqtt_broker_run(long port) { int srv = socket(AF_INET, SOCK_STREAM, 0); if (srv < 0) return -1; int one = 1; setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one)); struct sockaddr_in a; memset(&a, 0, sizeof(a)); a.sin_family = AF_INET; a.sin_addr.s_addr = INADDR_ANY; a.sin_port = htons((unsigned short)port); if (bind(srv, (struct sockaddr*)&a, sizeof(a)) < 0) return -2; if (listen(srv, 128) < 0) return -3; for (;;) { int c = accept(srv, 0, 0); if (c < 0) continue; pthread_t th; pthread_create(&th, 0, maca_mqtt_client, (void*)(long)c); pthread_detach(th); } return 0; }\n", 1), "long mqtt_connect(const char* host, long port) { int fd = socket(AF_INET, SOCK_STREAM, 0); if (fd < 0) return -1; struct sockaddr_in a; memset(&a, 0, sizeof(a)); a.sin_family = AF_INET; a.sin_port = htons((unsigned short)port); inet_pton(AF_INET, host ? host : \"127.0.0.1\", &a.sin_addr); if (connect(fd, (struct sockaddr*)&a, sizeof(a)) < 0) { close(fd); return -1; } unsigned char cx[] = { 0, 4, 'M', 'Q', 'T', 'T', 4, 2, 0, 60, 0, 0 }; maca_mqtt_send(fd, 0x10, cx, sizeof(cx)); unsigned char b1; maca_mqtt_read_n(fd, &b1, 1); int rl = maca_mqtt_read_remlen(fd); unsigned char tmp[8]; if (rl > 0 && rl <= 8) maca_mqtt_read_n(fd, tmp, rl); return fd; }\n", 1), "long mqtt_subscribe(long fd, const char* topic) { unsigned char body[512]; body[0] = 0; body[1] = 1; int n = 2 + maca_mqtt_put_str(body + 2, topic); body[n++] = 0; maca_mqtt_send((int)fd, 0x82, body, n); unsigned char b1; maca_mqtt_read_n((int)fd, &b1, 1); int rl = maca_mqtt_read_remlen((int)fd); unsigned char tmp[16]; if (rl > 0 && rl <= 16) maca_mqtt_read_n((int)fd, tmp, rl); return 0; }\n", 1), "long mqtt_publish(long fd, const char* topic, const char* payload) { unsigned char body[2048]; int n = maca_mqtt_put_str(body, topic); int pl = (int)strlen(payload ? payload : \"\"); if (n + pl < (int)sizeof(body)) { memcpy(body + n, payload, pl); n += pl; } maca_mqtt_send((int)fd, 0x30, body, n); return 0; }\n", 1), "const char* mqtt_receive(long fd) { unsigned char b1; while (maca_mqtt_read_n((int)fd, &b1, 1) == 0) { int rl = maca_mqtt_read_remlen((int)fd); if (rl < 0 || rl > 65535) return \"\"; unsigned char* body = (unsigned char*)malloc(rl > 0 ? rl : 1); if (rl > 0 && maca_mqtt_read_n((int)fd, body, rl)) { free(body); return \"\"; } if ((b1 >> 4) == 3) { int tl = (body[0] << 8) | body[1]; int po = 2 + tl; int pl = rl - po; char* out = (char*)malloc(pl > 0 ? pl + 1 : 1); if (pl > 0) memcpy(out, body + po, pl); out[pl > 0 ? pl : 0] = 0; free(body); return out; } free(body); } return \"\"; }\n", 1), "long mqtt_disconnect(long fd) { maca_mqtt_send((int)fd, 0xE0, 0, 0); close((int)fd); return 0; }\n", 1));  }
const char* emit_module(Module m) { const char* protos = emit_protos(m.items, 0); const char* code = maca_cat_own(maca_cat(protos, emit_starts(m.items)), emit_bodies(m.items, 0), 1); const char* bare = c_bare(code); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(c_preamble(), c_styles(m, code)), c_errors(bare), 1), c_process(bare), 1), c_threads(bare), 1), c_sockets(bare), 1), emit_types(m.items, 0), 1), emit_consts(m.items, 0), 1), code, 1), c_mqtt(c_bare(protos)), 1), c_ffi(protos), 1);  }
const char* c_styles(Module m, const char* code) { return ((maca_str_index_of(code, "MACA_STYLES") < 0) ? "" : maca_cat_own(maca_cat("#define MACA_STYLES \"", style_sheet(m.items)), "\"\n", 1));  }
const char* c_ffi(const char* protos) { return maca_cat(c_sqlite(protos), c_python(protos));  }
const char* c_sqlite(const char* protos) { return ((maca_str_index_of(protos, "sqlite_open(") < 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#include <sqlite3.h>\n", "static long maca_sq_handle(void* p) { return (long)(intptr_t)p; }\n"), "static sqlite3* maca_sq_db(long h) { return (sqlite3*)(intptr_t)h; }\n", 1), "static sqlite3_stmt* maca_sq_stmt(long h) { return (sqlite3_stmt*)(intptr_t)h; }\n", 1), "static const char* maca_sq_text(const unsigned char* t) { return t ? maca_cat((const char*)t, \"\") : \"\"; }\n", 1), "long sqlite_open(const char* path) { sqlite3* db = 0; if (sqlite3_open(path, &db) != SQLITE_OK) { if (db) sqlite3_close(db); return 0; } return maca_sq_handle(db); }\n", 1), "long sqlite_close(long db) { return sqlite3_close(maca_sq_db(db)); }\n", 1), "long sqlite_exec(long db, const char* sql) { char* err = 0; int rc = sqlite3_exec(maca_sq_db(db), sql, 0, 0, &err); if (err) sqlite3_free(err); return rc; }\n", 1), "long sqlite_prepare(long db, const char* sql) { sqlite3_stmt* st = 0; if (sqlite3_prepare_v2(maca_sq_db(db), sql, -1, &st, 0) != SQLITE_OK) return 0; return maca_sq_handle(st); }\n", 1), "long sqlite_step(long st) { return sqlite3_step(maca_sq_stmt(st)) == SQLITE_ROW; }\n", 1), "long sqlite_column_count(long st) { return sqlite3_column_count(maca_sq_stmt(st)); }\n", 1), "const char* sqlite_column_name(long st, long col) { return maca_sq_text((const unsigned char*)sqlite3_column_name(maca_sq_stmt(st), (int)col)); }\n", 1), "const char* sqlite_column_text(long st, long col) { return maca_sq_text(sqlite3_column_text(maca_sq_stmt(st), (int)col)); }\n", 1), "long sqlite_column_int(long st, long col) { return sqlite3_column_int64(maca_sq_stmt(st), (int)col); }\n", 1), "double sqlite_column_float(long st, long col) { return sqlite3_column_double(maca_sq_stmt(st), (int)col); }\n", 1), "long sqlite_finalize(long st) { return sqlite3_finalize(maca_sq_stmt(st)); }\n", 1));  }
const char* c_python(const char* protos) { return ((maca_str_index_of(protos, "py_call(") < 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#define PY_SSIZE_T_CLEAN\n#include <Python.h>\n", "static const char* maca_py_str(PyObject* r) { if (!r) return \"<py error>\"; const char* out = \"<py error>\"; PyObject* s = PyObject_Str(r); if (s) { const char* c = PyUnicode_AsUTF8(s); if (c) out = maca_cat(c, \"\"); Py_DECREF(s); } return out; }\n"), "static PyObject* maca_py_find(const char* module, const char* func) { PyObject* m = PyImport_ImportModule(module); if (!m) return 0; PyObject* f = PyObject_GetAttrString(m, func); Py_DECREF(m); if (f && PyCallable_Check(f)) return f; Py_XDECREF(f); return 0; }\n", 1), "const char* py_call(const char* module, const char* func) { Py_Initialize(); const char* out = \"<py error>\"; PyObject* f = maca_py_find(module, func); if (f) { PyObject* r = PyObject_CallObject(f, 0); out = maca_py_str(r); Py_XDECREF(r); Py_DECREF(f); } Py_Finalize(); return out; }\n", 1), "const char* py_call_s(const char* module, const char* func, const char* arg) { Py_Initialize(); const char* out = \"<py error>\"; PyObject* f = maca_py_find(module, func); if (f) { PyObject* r = PyObject_CallFunction(f, \"s\", arg ? arg : \"\"); out = maca_py_str(r); Py_XDECREF(r); Py_DECREF(f); } Py_Finalize(); return out; }\n", 1), "const char* py_call_i(const char* module, const char* func, long n) { Py_Initialize(); const char* out = \"<py error>\"; PyObject* f = maca_py_find(module, func); if (f) { PyObject* r = PyObject_CallFunction(f, \"L\", (long long)n); out = maca_py_str(r); Py_XDECREF(r); Py_DECREF(f); } Py_Finalize(); return out; }\n", 1));  }
const char* emit_consts(MacaList items, long i) { return ((i >= (items.len)) ? "" : (((*(Stmt*)items.data[i]).kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat("", emit_const((*(Stmt*)items.data[i]))), "\n", 1), emit_consts(items, (i + 1)), 1) : emit_consts(items, (i + 1))));  }
const char* emit_const(Stmt s) { return (c_constant(s.value) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("static ", c_held(s)), bind_c_decl(s), 1), " = ", 1), maca_cat_own(maca_cat("", emit_expr(s.value)), ";", 1), 3) : maca_cat_own(maca_cat("static ", bind_c_decl(s)), ";", 1));  }
const char* c_held(Stmt s) { return (c_lower_name(s.name) ? "" : "const ");  }
long c_constant(Expr e) { return ((((e.kind == EInt) || (e.kind == EFloat)) || (e.kind == EStr)) || (e.kind == EBool));  }
const char* emit_starts(MacaList items) { const char* sets = emit_start_sets(items, 0); return ((strcmp(sets, "") == 0) ? "" : maca_cat_own("__attribute__((constructor)) static void maca_module_init(void)", maca_cat_own(maca_cat(" { ", sets), "}\n", 1), 2));  }
const char* emit_start_sets(MacaList items, long i) { return ((i >= (items.len)) ? "" : ((((*(Stmt*)items.data[i]).kind != SBind) || c_constant((*(Stmt*)items.data[i]).value)) ? emit_start_sets(items, (i + 1)) : ({ Stmt s = (*(Stmt*)items.data[i]); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_id(s.name)), " = ", 1), emit_expr(s.value), 1), "; ", 1), emit_start_sets(items, (i + 1)), 1); })));  }
long is_type_item(Stmt s) { return ((s.kind == SRecord) || (s.kind == SSum));  }
const char* emit_types(MacaList items, long i) { return emit_ranked(items, i, 0);  }
const char* emit_ranked(MacaList items, long i, long r) { return ((r > (items.len)) ? "" : maca_cat(emit_rank(items, i, r), emit_ranked(items, i, (r + 1))));  }
const char* emit_rank(MacaList items, long i, long r) { return ((i >= (items.len)) ? "" : ((is_type_item((*(Stmt*)items.data[i])) && (type_rank(items, (*(Stmt*)items.data[i]), (items.len), 0) == r)) ? maca_cat_own(maca_cat_own(maca_cat("", emit_item((*(Stmt*)items.data[i]))), "\n", 1), emit_rank(items, (i + 1), r), 1) : emit_rank(items, (i + 1), r)));  }
long type_rank(MacaList items, Stmt s, long fuel, long i) { return (((fuel <= 0) || (i >= (s.params.len))) ? 0 : wider(node_rank(items, (*(Expr*)s.params.data[i]), fuel), type_rank(items, s, fuel, (i + 1))));  }
long node_rank(MacaList items, Expr p, long fuel) { return wider(named_rank(items, p.ty, fuel, 0), payload_rank(items, p.children, fuel, 0));  }
long payload_rank(MacaList items, MacaList cs, long fuel, long i) { return ((i >= (cs.len)) ? 0 : wider(named_rank(items, (*(Expr*)cs.data[i]).ty, fuel, 0), payload_rank(items, cs, fuel, (i + 1))));  }
long named_rank(MacaList items, const char* ty, long fuel, long i) { return ((i >= (items.len)) ? 0 : ((is_type_item((*(Stmt*)items.data[i])) && (strcmp((*(Stmt*)items.data[i]).name, ty) == 0)) ? (1 + type_rank(items, (*(Stmt*)items.data[i]), (fuel - 1), 0)) : named_rank(items, ty, fuel, (i + 1))));  }
long is_declared_only(Stmt s) { return ((s.kind == SFn) && ((s.body.len) == 0));  }
const char* emit_protos(MacaList items, long i) { return ((i >= (items.len)) ? "" : (((*(Stmt*)items.data[i]).kind != SFn) ? emit_protos(items, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat("", emit_proto((*(Stmt*)items.data[i]))), "\n", 1), emit_protos(items, (i + 1)), 1)));  }
const char* emit_bodies(MacaList items, long i) { return ((i >= (items.len)) ? "" : ((((*(Stmt*)items.data[i]).kind != SFn) || is_declared_only((*(Stmt*)items.data[i]))) ? emit_bodies(items, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat("", emit_fn((*(Stmt*)items.data[i]))), "\n", 1), emit_bodies(items, (i + 1)), 1)));  }
const char* emit_proto(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", c_ret(s)), " ", 1), c_name(s), 1), "(", 1), c_sig_params(s), 1), ");", 1);  }
const char* emit_item(Stmt s) { return ((s.kind == SRecord) ? emit_struct(s) : emit_sum(s));  }
const char* emit_struct(Stmt s) { const char* fields = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(emit_struct_field((*(Expr*)_m.data[_i]))); _r; }), " "); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("typedef struct { ", fields), " } ", 1), s.name, 1), ";", 1);  }
const char* emit_struct_field(Expr f) { return maca_cat_own(maca_cat("", c_decl(f.ty, c_id(f.text))), ";", 1);  }
const char* emit_sum(Stmt s) { return ((payload_arity(s.params, 0) == 0) ? ({ const char* variants = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(variant_name((*(Expr*)_m.data[_i]))); _r; }), ", "); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("typedef enum { ", variants), " } ", 1), s.name, 1), ";", 1); }) : emit_tagged_sum(s));  }
const char* variant_name(Expr v) { return v.text;  }
const char* emit_tagged_sum(Stmt s) { const char* tags = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(tag_name((*(Expr*)_m.data[_i]))); _r; }), ", "); const char* slots = emit_slots(s, payload_arity(s.params, 0), 0); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("typedef enum { ", tags), " } ", 1), s.name, 1), "_tag;\n", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("typedef struct { ", s.name), "_tag tag; ", 1), slots, 1), "} ", 1), s.name, 1), ";\n", 1), 3), emit_ctors(s, 0), 1);  }
const char* tag_name(Expr v) { return maca_cat_own(maca_cat("", v.text), "_tag", 1);  }
long payload_arity(MacaList vs, long i) { return ((i >= (vs.len)) ? 0 : wider(((*(Expr*)vs.data[i]).children.len), payload_arity(vs, (i + 1))));  }
long wider(long a, long b) { return ((a > b) ? a : b);  }
const char* emit_slots(Stmt s, long n, long i) { return ((i >= n) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", slot_type(s, i, 0)), " _", 1), maca_int_to_str(i), 3), "; ", 1), emit_slots(s, n, (i + 1)), 1));  }
const char* slot_type(Stmt s, long at, long i) { return ((i >= (s.params.len)) ? "long" : ((((*(Expr*)s.params.data[i]).children.len) <= at) ? slot_type(s, at, (i + 1)) : ((strcmp((*(Expr*)(*(Expr*)s.params.data[i]).children.data[at]).ty, s.name) == 0) ? "long" : agreed(type_c((*(Expr*)(*(Expr*)s.params.data[i]).children.data[at]).ty), slot_type(s, at, (i + 1))))));  }
const char* agreed(const char* a, const char* b) { return (((strcmp(b, "long") == 0) || (strcmp(a, b) == 0)) ? a : "long");  }
const char* emit_ctors(Stmt s, long i) { return ((i >= (s.params.len)) ? "" : maca_cat(emit_ctor(s, (*(Expr*)s.params.data[i])), emit_ctors(s, (i + 1))));  }
const char* emit_ctor(Stmt s, Expr v) { return (((v.children.len) == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("static const ", s.name), " ", 1), v.text, 1), " = { ", 1), v.text, 1), "_tag };\n", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("static inline ", s.name), " ", 1), v.text, 1), "(", 1), ctor_params(s, v.children, 0), 1), ") ", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ ", s.name), " _v; _v.tag = ", 1), v.text, 1), "_tag; ", 1), 3), ctor_assigns(s, v.children, 0), 1), "return _v; }\n", 1));  }
const char* ctor_params(Stmt s, MacaList ps, long i) { return (((ps.len) == 0) ? "void" : ((i >= (ps.len)) ? "" : ((i == ((ps.len) - 1)) ? maca_cat_own(maca_cat_own(maca_cat("", ctor_slot(s, ps, i)), " _", 1), maca_int_to_str(i), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ctor_slot(s, ps, i)), " _", 1), maca_int_to_str(i), 3), ", ", 1), ctor_params(s, ps, (i + 1)), 1))));  }
const char* ctor_slot(Stmt s, MacaList ps, long i) { return ((strcmp((*(Expr*)ps.data[i]).ty, s.name) == 0) ? s.name : slot_type(s, i, 0));  }
const char* ctor_assigns(Stmt s, MacaList ps, long i) { return ((i >= (ps.len)) ? "" : ((strcmp((*(Expr*)ps.data[i]).ty, s.name) == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), "* _b", 1), maca_int_to_str(i), 3), " = (", 1), s.name, 1), "*)malloc(sizeof(", 1), s.name, 1), "));", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(" *_b", maca_int_to_str(i), 2), " = _", 1), maca_int_to_str(i), 3), "; _v._", 1), maca_int_to_str(i), 3), " = (long)_b", 1), maca_int_to_str(i), 3), ";", 1), 3), maca_cat(" ", ctor_assigns(s, ps, (i + 1))), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("_v._", maca_int_to_str(i), 2), " = _", 1), maca_int_to_str(i), 3), "; ", 1), ctor_assigns(s, ps, (i + 1)), 1)));  }
const char* rust_int(long n) { return maca_cat_own(maca_cat_own("", maca_int_to_str(n), 2), "i64", 1);  }
const char* rust_str(const char* s) { return maca_cat_own(maca_cat("\"", s), "\".to_string()", 1);  }
const char* rid(const char* name) { return ((maca_str_index_of(RustReserved, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0) ? maca_cat_own(maca_cat("", name), "_mc", 1) : name);  }
long rcopy(const char* ty) { return ((((((strcmp(ty, "") == 0) || (strcmp(ty, "int") == 0)) || (strcmp(ty, "float") == 0)) || (strcmp(ty, "bool") == 0)) || r_is_fn(ty)) || (strcmp(ty, "Future") == 0));  }
const char* rowned(const char* ty) { return (rcopy(ty) ? "" : ".clone()");  }
const char* remit_expr(Expr e) { return (e.kind == EInt ? rust_int(e.ival) : (e.kind == EFloat ? maca_cat_own(maca_cat("", e.text), "_f64", 1) : (e.kind == EStr ? rust_str(e.text) : (e.kind == EBool ? e.text : (e.kind == EIdent ? maca_cat_own(maca_cat("", rid(e.text)), rowned(e.ty), 1) : (e.kind == ECall ? remit_call(e) : (e.kind == EBinary ? remit_binary(e) : (e.kind == ETernary ? remit_ternary(e) : (e.kind == EIf ? remit_ternary(e) : (e.kind == EUnary ? remit_unary(e) : (e.kind == ERecord ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), " { ", 1), remit_lit_fields(e.children), 1), " }", 1) : (e.kind == EWith ? remit_with(e) : (e.kind == EBlock ? remit_block(e) : (e.kind == EField ? maca_cat("", rfield(e)) : (e.kind == EMatch ? remit_match(e) : (e.kind == EMethod ? remit_method(e) : (e.kind == EList ? maca_cat_own(maca_cat("Rc::new(vec![", remit_args(e.children, 0)), "])", 1) : (e.kind == EJump ? rjump(e) : (e.kind == EWhile ? remit_while(e) : (e.kind == EFor ? remit_for(e) : (e.kind == ELambda ? remit_lambda(e) : "0i64")))))))))))))))))))));  }
const char* rfield(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", rplace((*(Expr*)e.children.data[0]))), ".", 1), rid(e.text), 1), rowned(e.ty), 1);  }
const char* rplace(Expr e) { return ((e.kind == EIdent) ? rid(e.text) : ((e.kind == EField) ? maca_cat_own(maca_cat_own(maca_cat("", rplace((*(Expr*)e.children.data[0]))), ".", 1), rid(e.text), 1) : remit_expr(e)));  }
const char* rjump(Expr e) { return (((e.children.len) == 0) ? e.text : maca_cat_own(maca_cat_own(maca_cat("", e.text), " ", 1), remit_expr((*(Expr*)e.children.data[0])), 1));  }
const char* remit_lambda(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own("|", maca_list_join(({ MacaList _m = lambda_params(e); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(binder_name((*(Expr*)_m.data[_i]))); _r; }), ", "), 2), "|", 1), maca_cat(" ", remit_expr(lambda_body(e))), 3);  }
const char* remit_while(Expr e) { return maca_cat_own(maca_cat("while ", remit_expr((*(Expr*)e.children.data[0]))), maca_cat_own(maca_cat(" { ", rblock_stmts(e.stmts, 0)), "}", 1), 3);  }
const char* remit_for(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("for ", rid(e.text)), " in (", 1), remit_expr((*(Expr*)e.children.data[0])), 1), ").iter().cloned()", 1), maca_cat_own(maca_cat(" { ", rblock_stmts(e.stmts, 0)), "}", 1), 3);  }
const char* remit_unary(Expr e) { return ((strcmp(e.text, "fail") == 0) ? maca_cat_own(maca_cat("panic!(\"{}\", ", remit_expr((*(Expr*)e.children.data[0]))), ")", 1) : ((strcmp(e.text, "try") == 0) ? maca_cat_own(maca_cat("maca_try(|| { let _ = ", remit_expr((*(Expr*)e.children.data[0]))), "; })", 1) : ((strcmp(e.text, "spawn") == 0) ? maca_cat_own(maca_cat("std::thread::spawn(move || ", remit_expr((*(Expr*)e.children.data[0]))), ")", 1) : ((strcmp(e.text, "await") == 0) ? maca_cat_own(maca_cat("(", remit_expr((*(Expr*)e.children.data[0]))), ").join().unwrap()", 1) : maca_cat_own(maca_cat_own(maca_cat("(", e.text), remit_expr((*(Expr*)e.children.data[0])), 1), ")", 1)))));  }
const char* remit_ternary(Expr e) { return (rmissing_else((*(Expr*)e.children.data[2])) ? rif_stmt(e) : ({ const char* cond = remit_expr((*(Expr*)e.children.data[0])); const char* then = remit_expr((*(Expr*)e.children.data[1])); const char* els = remit_expr((*(Expr*)e.children.data[2])); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(if ", cond), " { ", 1), then, 1), " } else { ", 1), els, 1), " })", 1); }));  }
long rmissing_else(Expr e) { return ((e.kind == EIdent) && (strcmp(e.text, "?") == 0));  }
const char* rif_stmt(Expr e) { const char* cond = remit_expr((*(Expr*)e.children.data[0])); const char* then = rbranch((*(Expr*)e.children.data[1])); Expr els = (*(Expr*)e.children.data[2]); return (rmissing_else(els) ? maca_cat_own(maca_cat_own(maca_cat("if ", cond), " ", 1), then, 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("if ", cond), " ", 1), then, 1), " else ", 1), rbranch(els), 1));  }
const char* rbranch(Expr e) { return ((e.kind == EIf) ? rif_stmt(e) : ((e.kind == EBlock) ? maca_cat_own(maca_cat_own(maca_cat("{ ", rblock_stmts(e.stmts, 0)), remit_expr((*(Expr*)e.children.data[0])), 1), "; }", 1) : maca_cat_own(maca_cat("{ ", remit_expr(e)), "; }", 1)));  }
const char* remit_match(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("match ", remit_match_on(e)), " {", 1), maca_cat_own(maca_cat(" ", remit_arms(e.children, 1, remit_expr((*(Expr*)e.children.data[0])))), " }", 1), 3);  }
const char* remit_match_on(Expr e) { const char* scrut = remit_expr((*(Expr*)e.children.data[0])); return (has_str_arm(e.children, 1) ? maca_cat_own(maca_cat("", scrut), ".as_str()", 1) : scrut);  }
long has_str_arm(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? 0 : (((*(Expr*)cs.data[i]).kind == EStr) ? 1 : has_str_arm(cs, (i + 2))));  }
const char* rchar_pred(const char* name) { return ((strcmp(name, "is_whitespace") == 0) ? "is_whitespace" : ((strcmp(name, "is_ascii_digit") == 0) ? "is_ascii_digit" : ((strcmp(name, "is_alpha") == 0) ? "is_ascii_alphabetic" : "")));  }
long rstr_helper(const char* name) { return (((((((((((((strcmp(name, "upper") == 0) || (strcmp(name, "lower") == 0)) || (strcmp(name, "trim") == 0)) || (strcmp(name, "contains") == 0)) || (strcmp(name, "ends_with") == 0)) || (strcmp(name, "starts_with") == 0)) || (strcmp(name, "replace") == 0)) || (strcmp(name, "split") == 0)) || (strcmp(name, "repeat") == 0)) || (strcmp(name, "substr") == 0)) || (strcmp(name, "pad_start") == 0)) || (strcmp(name, "pad_end") == 0)) || (strcmp(rchar_pred(name), "") != 0));  }
const char* remit_str_method(Expr e, const char* recv) { const char* pred = rchar_pred(e.text); return ((strcmp(pred, "") != 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").chars().next().map_or(false, |c| c.", 1), pred, 1), "())", 1) : ((strcmp(e.text, "upper") == 0) ? maca_cat_own(maca_cat("(", recv), ").to_uppercase()", 1) : ((strcmp(e.text, "lower") == 0) ? maca_cat_own(maca_cat("(", recv), ").to_lowercase()", 1) : ((strcmp(e.text, "trim") == 0) ? maca_cat_own(maca_cat("(", recv), ").trim().to_string()", 1) : ((strcmp(e.text, "repeat") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").repeat((", 1), rarg(e, 1), 1), ") as usize)", 1) : ((strcmp(e.text, "substr") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_substr(", recv), ", ", 1), rarg(e, 1), 1), ", ", 1), rarg(e, 2), 1), ")", 1) : (((strcmp(e.text, "pad_start") == 0) || (strcmp(e.text, "pad_end") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_", e.text), "(", 1), recv, 1), ", ", 1), rarg(e, 1), 1), ")", 1) : ((strcmp(e.text, "replace") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").replace(", 1), rarg(e, 1), 1), ".as_str(), ", 1), rarg(e, 2), 1), ".as_str())", 1) : ((strcmp(e.text, "split") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::new((", recv), ").split(", 1), rarg(e, 1), 1), ".as_str())", 1), ".map(|p| p.to_string()).collect::<Vec<String>>())", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").", 1), e.text, 1), "(", 1), rarg(e, 1), 1), ".as_str())", 1))))))))));  }
const char* rmap_method_of(Expr e, const char* recv) { return (((strcmp(e.text, "get") == 0) && ((e.children.len) > 2)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").get(&", 1), rarg(e, 1), 1), ").cloned().unwrap_or(", 1), rarg(e, 2), 1), ")", 1) : ((strcmp(e.text, "get") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").get(&", 1), rarg(e, 1), 1), ").cloned().unwrap_or_default()", 1) : (((strcmp(e.text, "has") == 0) || (strcmp(e.text, "contains") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").contains_key(&", 1), rarg(e, 1), 1), ")", 1) : ((strcmp(e.text, "keys") == 0) ? maca_cat_own(maca_cat("Rc::new((", recv), ").keys().cloned().collect::<Vec<_>>())", 1) : ((strcmp(e.text, "values") == 0) ? maca_cat_own(maca_cat("Rc::new((", recv), ").values().cloned().collect::<Vec<_>>())", 1) : ((strcmp(e.text, "set") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _m = ", recv), ";", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" Rc::make_mut(&mut _m).insert(", rarg(e, 1)), ", ", 1), rarg(e, 2), 1), ");", 1), 3), " _m }", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat("{ let mut _m = ", recv), ";", 1), maca_cat_own(maca_cat(" Rc::make_mut(&mut _m).remove(&", rarg(e, 1)), "); _m }", 1), 3) : maca_cat_own(maca_cat("((", recv), ").len() as i64)", 1))))))));  }
long rlist_method(const char* name) { return (((((((((((((strcmp(name, "sum") == 0) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) || (strcmp(name, "first") == 0)) || (strcmp(name, "last") == 0)) || (strcmp(name, "pop") == 0)) || (strcmp(name, "reverse") == 0)) || (strcmp(name, "sort") == 0)) || (strcmp(name, "set") == 0)) || (strcmp(name, "insert") == 0)) || (strcmp(name, "remove") == 0)) || (strcmp(name, "filter") == 0)) || (strcmp(name, "reduce") == 0));  }
const char* rlist_method_of(Expr e, const char* recv) { const char* el = relem((*(Expr*)e.children.data[0]).ty); return ((strcmp(e.text, "sum") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").iter().cloned().sum::<", 1), rtype(el), 1), ">()", 1) : (((strcmp(e.text, "min") == 0) || (strcmp(e.text, "max") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").iter().cloned().", 1), e.text, 1), "().unwrap_or_default()", 1) : ((strcmp(e.text, "first") == 0) ? maca_cat_own(maca_cat("(", recv), ").first().cloned().unwrap_or_default()", 1) : ((strcmp(e.text, "last") == 0) ? maca_cat_own(maca_cat("(", recv), ").last().cloned().unwrap_or_default()", 1) : ((strcmp(e.text, "pop") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::new((", recv), ")[..(", 1), recv, 1), ").len().saturating_sub(1)].to_vec())", 1) : ((strcmp(e.text, "reverse") == 0) ? maca_cat_own(maca_cat("{ let mut _v = ", recv), "; Rc::make_mut(&mut _v).reverse(); _v }", 1) : ((strcmp(e.text, "sort") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", recv), ";", 1), " Rc::make_mut(&mut _v).sort_by(|a, b| a.partial_cmp(b)", 1), ".unwrap_or(std::cmp::Ordering::Equal)); _v }", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", recv), "; let _i = (", 1), rarg(e, 1), 1), ") as usize;", 1), " if _i < _v.len() { Rc::make_mut(&mut _v).remove(_i); } _v }", 1) : ((strcmp(e.text, "insert") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", recv), ";", 1), maca_cat_own(maca_cat(" let _i = ((", rarg(e, 1)), ") as usize).min(_v.len());", 1), 3), maca_cat_own(maca_cat(" Rc::make_mut(&mut _v).insert(_i, ", rarg(e, 2)), "); _v }", 1), 3) : ((strcmp(e.text, "set") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", recv), "; let _i = (", 1), rarg(e, 1), 1), ") as usize;", 1), maca_cat_own(maca_cat(" if _i < _v.len() { Rc::make_mut(&mut _v)[_i] = ", rarg(e, 2)), "; }", 1), 3), " _v }", 1) : ((strcmp(e.text, "filter") == 0) ? maca_cat_own(maca_cat_own(maca_cat("Rc::new((", recv), ").iter().cloned().filter(|_x|", 1), maca_cat_own(maca_cat(" (", rarg(e, 1)), ")(_x.clone())).collect::<Vec<_>>())", 1), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").iter().cloned().fold(", 1), rarg(e, 2), 1), ", ", 1), rarg(e, 1), 1), ")", 1))))))))))));  }
const char* rarg(Expr e, long i) { return ((i >= (e.children.len)) ? "Default::default()" : remit_expr((*(Expr*)e.children.data[i])));  }
long rstr_index(Expr e) { return (((strcmp(e.text, "at") == 0) || (strcmp(e.text, "get") == 0)) && (strcmp((*(Expr*)e.children.data[0]).ty, "str") == 0));  }
const char* remit_method(Expr e) { const char* recv = remit_expr((*(Expr*)e.children.data[0])); return (rstr_helper(e.text) ? remit_str_method(e, recv) : ((strcmp(e.text, "chars") == 0) ? maca_cat_own(maca_cat_own(maca_cat("Rc::new((", recv), ").chars().map(|c| c.to_string())", 1), ".collect::<Vec<String>>())", 1) : ((strcmp(e.text, "map") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::new((", recv), ").iter().cloned()", 1), maca_cat_own(maca_cat(".map(", remit_expr((*(Expr*)e.children.data[1]))), ")", 1), 3), ".collect::<Vec<_>>())", 1) : (((strcmp(e.text, "length") == 0) || (strcmp(e.text, "count") == 0)) ? maca_cat_own(maca_cat("((", recv), ").len() as i64)", 1) : (rstr_index(e) ? ({ const char* idx = remit_expr((*(Expr*)e.children.data[1])); maca_cat_own(maca_cat_own(maca_cat("{ let _i = (", idx), ") as usize;", 1), maca_cat_own(maca_cat(" (", recv), ").get(_i.._i + 1).unwrap_or(\"\").to_string() }", 1), 3); }) : ((strcmp(e.text, "at") == 0) ? ({ const char* idx = remit_expr((*(Expr*)e.children.data[1])); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", recv), ").as_bytes()[(", 1), idx, 1), ") as usize] as i64)", 1); }) : ((strcmp(e.text, "get") == 0) ? ({ const char* cell = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), "[(", 1), remit_expr((*(Expr*)e.children.data[1])), 1), ") as usize]", 1); maca_cat(cell, rclone((*(Expr*)e.children.data[0]).ty)); }) : (((strcmp(e.text, "slice") == 0) && rlist((*(Expr*)e.children.data[0]).ty)) ? ({ const char* lo = remit_expr((*(Expr*)e.children.data[1])); const char* hi = remit_expr((*(Expr*)e.children.data[2])); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::new(", recv), "[(", 1), lo, 1), ") as usize..(", 1), hi, 1), ") as usize].to_vec())", 1); }) : ((strcmp(e.text, "slice") == 0) ? ({ const char* lo = remit_expr((*(Expr*)e.children.data[1])); const char* hi = remit_expr((*(Expr*)e.children.data[2])); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").get((", 1), lo, 1), ") as usize..(", 1), hi, 1), ") as usize)", 1), ".unwrap_or(\"\").to_string()", 1); }) : (((strcmp(e.text, "index_of") == 0) && (strcmp((*(Expr*)e.children.data[0]).ty, "str") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").find(", 1), remit_expr((*(Expr*)e.children.data[1])), 1), ".as_str())", 1), ".map(|i| i as i64).unwrap_or(-1)", 1) : ((strcmp(e.text, "index_of") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").iter().position(|v| *v == ", 1), remit_expr((*(Expr*)e.children.data[1])), 1), ")", 1), ".map(|i| i as i64).unwrap_or(-1)", 1) : ((strcmp(e.text, "join") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").join(", 1), remit_expr((*(Expr*)e.children.data[1])), 1), ".as_str())", 1) : ((strcmp(rmap_key((*(Expr*)e.children.data[0]).ty), "") != 0) ? rmap_method_of(e, recv) : ((rlist((*(Expr*)e.children.data[0]).ty) && rlist_method(e.text)) ? rlist_method_of(e, recv) : ((strcmp(e.text, "push") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", recv), ";", 1), maca_cat_own(maca_cat(" Rc::make_mut(&mut _v).push(", remit_expr((*(Expr*)e.children.data[1]))), ");", 1), 3), " _v }", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".", 1), e.text, 1), "(", 1), remit_args(e.children, 1), 1), ")", 1))))))))))))))));  }
const char* remit_call(Expr e) { const char* args = remit_args(e.children, 0); return ((strcmp(e.text, "info") == 0) ? maca_cat_own(maca_cat("println!(\"{}\", ", rstr_of(rarg_ty(e.children), args)), ")", 1) : ((strcmp(e.text, "print") == 0) ? maca_cat_own(maca_cat("print!(\"{}\", ", rstr_of(rarg_ty(e.children), args)), ")", 1) : ((strcmp(e.text, "str") == 0) ? rstr_of(rarg_ty(e.children), args) : ((strcmp(e.text, "int") == 0) ? rint_of(rarg_ty(e.children), args) : ((strcmp(e.text, "float") == 0) ? maca_cat_own(maca_cat("((", args), ") as f64)", 1) : ((strcmp(e.text, "len") == 0) ? rlen_of(rarg_ty(e.children), args) : (((strcmp(e.text, "err") == 0) || (strcmp(e.text, "warn") == 0)) ? maca_cat_own(maca_cat("eprintln!(\"{}\", ", rstr_of(rarg_ty(e.children), args)), ")", 1) : ((strcmp(e.text, "assert_eq") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_assert_eq(", rshown(e.children, 0)), ", ", 1), rshown(e.children, 1), 1), ",", 1), maca_cat_own(maca_cat(" ", rshown(e.children, 2)), ")", 1), 3) : (((strcmp(e.text, "map") == 0) && ((e.children.len) == 0)) ? "Rc::new(std::collections::BTreeMap::new())" : ((strcmp(e.text, "read_line") == 0) ? "maca_input(String::new())" : ((strcmp(e.text, "clamp") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", rarg(e, 0)), ").clamp(", 1), rarg(e, 1), 1), ", ", 1), rarg(e, 2), 1), ")", 1) : (((strcmp(e.text, "abs") == 0) || (strcmp(e.text, "signum") == 0)) ? maca_cat_own(maca_cat("(", args), ").abs()", 1) : ((strcmp(e.text, "sign") == 0) ? maca_cat_own(maca_cat("(", args), ").signum()", 1) : ((strcmp(e.text, "pow") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", rarg(e, 0)), " as f64).powf(", 1), rarg(e, 1), 1), " as f64)", 1) : ((strcmp(e.text, "log") == 0) ? maca_cat_own(maca_cat("((", args), ") as f64).ln()", 1) : (rmath_call(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", args), ") as f64).", 1), e.text, 1), "()", 1) : ((rpicking(e.text) && ((e.children.len) == 2)) ? rpick2(e) : (rruntime_call(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca_", e.text), "(", 1), args, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", rid(e.text)), "(", 1), args, 1), ")", 1)))))))))))))))))));  }
const char* rlen_of(const char* ty, const char* args) { return maca_cat_own(maca_cat("((", args), ").len() as i64)", 1);  }
const char* rstr_of(const char* ty, const char* args) { return (rlist(ty) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("format!(\"[{}]\", (", args), ").iter()", 1), ".map(|_c| format!(\"{}\", _c))", 1), ".collect::<Vec<String>>().join(\", \"))", 1) : maca_cat_own(maca_cat("format!(\"{}\", ", args), ")", 1));  }
long rmath_call(const char* name) { return ((((((((strcmp(name, "sqrt") == 0) || (strcmp(name, "floor") == 0)) || (strcmp(name, "ceil") == 0)) || (strcmp(name, "round") == 0)) || (strcmp(name, "sin") == 0)) || (strcmp(name, "cos") == 0)) || (strcmp(name, "tan") == 0)) || (strcmp(name, "exp") == 0));  }
long rpicking(const char* name) { return ((strcmp(name, "min") == 0) || (strcmp(name, "max") == 0));  }
const char* rpick2(Expr e) { const char* a = remit_expr((*(Expr*)e.children.data[0])); const char* b = remit_expr((*(Expr*)e.children.data[1])); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", a), ").", 1), e.text, 1), "(", 1), b, 1), ")", 1);  }
const char* rshown(MacaList cs, long i) { return ((i >= (cs.len)) ? "String::new()" : ((((*(Expr*)cs.data[i]).kind == EList) && (((*(Expr*)cs.data[i]).children.len) == 0)) ? "\"[]\".to_string()" : rstr_of((*(Expr*)cs.data[i]).ty, remit_expr((*(Expr*)cs.data[i])))));  }
long rruntime_call(const char* name) { return (((((((((((((((((((((strcmp(name, "read_file") == 0) || (strcmp(name, "write_file") == 0)) || (strcmp(name, "exec") == 0)) || (strcmp(name, "assert") == 0)) || (strcmp(name, "failures") == 0)) || (strcmp(name, "sleep_ms") == 0)) || (strcmp(name, "make_dir") == 0)) || (strcmp(name, "remove_dir") == 0)) || (strcmp(name, "remove_file") == 0)) || (strcmp(name, "file_exists") == 0)) || (strcmp(name, "is_dir") == 0)) || (strcmp(name, "env") == 0)) || (strcmp(name, "cwd") == 0)) || (strcmp(name, "panic") == 0)) || (strcmp(name, "is_tty") == 0)) || (strcmp(name, "now_ms") == 0)) || (strcmp(name, "list_dir") == 0)) || (strcmp(name, "file_size") == 0)) || (strcmp(name, "chr") == 0)) || (strcmp(name, "ord") == 0)) || (strcmp(name, "chdir") == 0));  }
const char* rarg_ty(MacaList cs) { return (((cs.len) == 0) ? "" : (*(Expr*)cs.data[0]).ty);  }
const char* rint_of(const char* ty, const char* args) { return ((strcmp(ty, "int") == 0) ? args : (((strcmp(ty, "float") == 0) || (strcmp(ty, "bool") == 0)) ? maca_cat_own(maca_cat("((", args), ") as i64)", 1) : maca_cat_own(maca_cat("(", args), ").trim().parse::<i64>().unwrap_or(0)", 1)));  }
const char* remit_binary(Expr e) { const char* l = remit_expr((*(Expr*)e.children.data[0])); const char* r = remit_expr((*(Expr*)e.children.data[1])); long on_list = (rlist((*(Expr*)e.children.data[0]).ty) || rlist((*(Expr*)e.children.data[1]).ty)); Expr lhs = (*(Expr*)e.children.data[0]); return ((((strcmp(e.text, "=") == 0) && (lhs.kind == EMethod)) && (strcmp(lhs.text, "get") == 0)) ? rstore(lhs, r) : ((strcmp(e.text, "..") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::new(((", l), ")..(", 1), r, 1), ")).collect::<Vec<i64>>())", 1) : ((rjoins(e) && on_list) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ let mut _v = ", l), ";", 1), maca_cat_own(maca_cat(" Rc::make_mut(&mut _v).extend((", r), ").iter().cloned());", 1), 3), " _v }", 1) : (rjoins(e) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " + &", 1), r, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " ", 1), e.text, 1), " ", 1), r, 1), ")", 1)))));  }
const char* rstore(Expr l, const char* value) { const char* holder = rplace((*(Expr*)l.children.data[0])); const char* ix = remit_expr((*(Expr*)l.children.data[1])); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Rc::make_mut(&mut ", holder), ")[(", 1), ix, 1), ") as usize] = ", 1), value, 1);  }
long rjoins(Expr e) { Expr l = (*(Expr*)e.children.data[0]); Expr r = (*(Expr*)e.children.data[1]); return ((strcmp(e.text, "++") == 0) || ((strcmp(e.text, "+") == 0) && (((strcmp(l.ty, "str") == 0) && (strcmp(r.ty, "str") == 0)) || (rlist(l.ty) && rlist(r.ty)))));  }
long rlist(const char* ty) { return ((((int)strlen(ty)) > 2) && (strcmp(maca_str_slice(ty, (((int)strlen(ty)) - 2), ((int)strlen(ty))), "[]") == 0));  }
const char* remit_arms(MacaList cs, long i, const char* scrut) { return (((i + 1) >= (cs.len)) ? "" : (rfields_pat((*(Expr*)cs.data[i])) ? maca_cat_own(maca_cat("_ => { ", rbind_fields(scrut, (*(Expr*)cs.data[i]).children, 0)), maca_cat_own(maca_cat("", remit_expr((*(Expr*)cs.data[(i + 1)]))), " }, ", 1), 3) : ({ const char* arm = maca_cat_own(maca_cat_own(maca_cat("", remit_arm_pat((*(Expr*)cs.data[i]), (*(Expr*)cs.data[(i + 1)]))), " =>", 1), maca_cat_own(maca_cat(" ", remit_expr((*(Expr*)cs.data[(i + 1)]))), ", ", 1), 3); maca_cat(arm, remit_arms(cs, (i + 2), scrut)); })));  }
const char* remit_arm_pat(Expr p, Expr body) { return ((p.kind == EGuard) ? maca_cat_own(maca_cat("", remit_arm_pat((*(Expr*)p.children.data[0]), body)), maca_cat(" if ", remit_expr((*(Expr*)p.children.data[1]))), 3) : ((((p.kind == EIdent) && ((p.children.len) == 0)) && rassigned_deep(body, p.text)) ? maca_cat("mut ", remit_pat(p)) : remit_pat(p)));  }
long rfields_pat(Expr p) { return ((p.kind == EIdent) && (strcmp(p.text, "{}") == 0));  }
const char* rbind_fields(const char* scrut, MacaList fs, long i) { return ((i >= (fs.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("let ", rid((*(Expr*)fs.data[i]).text)), " = ", 1), scrut, 1), ".", 1), rid((*(Expr*)fs.data[i]).text), 1), ".clone(); ", 1), rbind_fields(scrut, fs, (i + 1)), 1));  }
const char* remit_pat(Expr p) { return (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("", remit_pat((*(Expr*)p.children.data[0]))), " | ", 1), remit_pat((*(Expr*)p.children.data[1])), 1) : ((p.kind == EGuard) ? maca_cat_own(maca_cat_own(maca_cat("", remit_pat((*(Expr*)p.children.data[0]))), " if ", 1), remit_expr((*(Expr*)p.children.data[1])), 1) : ((p.kind == EStr) ? maca_cat_own(maca_cat("\"", p.text), "\"", 1) : (((p.children.len) == 0) ? p.text : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", p.text), "(", 1), maca_list_join(({ MacaList _m = p.children; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(binder_name((*(Expr*)_m.data[_i]))); _r; }), ", "), 3), ")", 1)))));  }
const char* binder_name(Expr b) { return rid(b.text);  }
const char* remit_block(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("{ ", rblock_stmts(e.stmts, 0)), remit_expr((*(Expr*)e.children.data[0])), 1), " }", 1);  }
const char* rblock_stmts(MacaList body, long i) { return ((i >= (body.len)) ? "" : ({ const char* here = remit_stmt((*(Stmt*)body.data[i]), 0, rmut(body, (i + 1), (*(Stmt*)body.data[i]))); maca_cat_own(maca_cat_own(maca_cat("", here), " ", 1), rblock_stmts(body, (i + 1)), 1); }));  }
long rmut(MacaList body, long i, Stmt s) { return ((s.kind != SBind) ? 0 : rassigned_in(body, i, s.name));  }
long rassigned_in(MacaList body, long i, const char* name) { return ((i >= (body.len)) ? 0 : ((((*(Stmt*)body.data[i]).kind == SSet) && (strcmp((*(Stmt*)body.data[i]).name, name) == 0)) ? 1 : (rassigned_deep((*(Stmt*)body.data[i]).value, name) ? 1 : rassigned_in(body, (i + 1), name))));  }
long rassigned_deep(Expr e, const char* name) { return ((rstored_into(e, name) || rassigned_in(e.stmts, 0, name)) || rassigned_any(e.children, 0, name));  }
long rstored_into(Expr e, const char* name) { return (((e.kind != EBinary) || (strcmp(e.text, "=") != 0)) ? 0 : ({ Expr lhs = (*(Expr*)e.children.data[0]); ((((lhs.kind == EMethod) && (strcmp(lhs.text, "get") == 0)) && ((*(Expr*)lhs.children.data[0]).kind == EIdent)) && (strcmp((*(Expr*)lhs.children.data[0]).text, name) == 0)); }));  }
long rassigned_any(MacaList cs, long i, const char* name) { return ((i >= (cs.len)) ? 0 : (rassigned_deep((*(Expr*)cs.data[i]), name) ? 1 : rassigned_any(cs, (i + 1), name)));  }
const char* remit_with(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("{ let mut _w = ", remit_expr((*(Expr*)e.children.data[0]))), ";", 1), maca_cat_own(maca_cat(" ", remit_updates(e.children, 1)), "_w }", 1), 3);  }
const char* remit_updates(MacaList fs, long i) { return ((i >= (fs.len)) ? "" : ({ Expr f = (*(Expr*)fs.data[i]); const char* set = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_w.", rid((*(Expr*)f.children.data[0]).text)), " = ", 1), remit_expr((*(Expr*)f.children.data[1])), 1), "; ", 1); maca_cat(set, remit_updates(fs, (i + 1))); }));  }
const char* remit_lit_fields(MacaList fs) { return maca_list_join(({ MacaList _m = fs; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_lit_field((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* remit_lit_field(Expr f) { return maca_cat_own(maca_cat_own(maca_cat("", rid((*(Expr*)f.children.data[0]).text)), ": ", 1), remit_expr((*(Expr*)f.children.data[1])), 1);  }
const char* remit_args(MacaList xs, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(xs, i, (xs.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_expr((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* relem(const char* ty) { return (maca_ends_with(ty, "[]") ? maca_str_slice(ty, 0, (((int)strlen(ty)) - 2)) : ty);  }
const char* rclone(const char* ty) { const char* el = rtype(relem(ty)); return ((((strcmp(el, "i64") == 0) || (strcmp(el, "f64") == 0)) || (strcmp(el, "bool") == 0)) ? "" : ".clone()");  }
const char* rmap_key(const char* ty) { return map_type_key(ty);  }
const char* rmap_val(const char* ty) { return map_type_val(ty);  }
long r_is_fn(const char* ty) { return (maca_str_index_of(ty, ") -> ") >= 0);  }
const char* r_fn_ret(const char* ty) { return maca_str_slice(ty, (maca_str_index_of(ty, ") -> ") + 5), ((int)strlen(ty)));  }
const char* r_fn_params(const char* ty) { return maca_str_slice(ty, 1, maca_str_index_of(ty, ") -> "));  }
const char* r_param_types(const char* list, long at, const char* acc) { long cut = maca_str_index_of(list, ", "); return ((strcmp(list, "") == 0) ? acc : ((cut < 0) ? maca_cat(acc, rtype(list)) : ({ const char* head = rtype(maca_str_slice(list, 0, cut)); const char* rest = maca_str_slice(list, (cut + 2), ((int)strlen(list))); r_param_types(rest, (at + 1), maca_cat_own(maca_cat(acc, head), ", ", 1)); })));  }
const char* rtype(const char* ty) { return (r_is_fn(ty) ? maca_cat_own(maca_cat_own(maca_cat("fn(", r_param_types(r_fn_params(ty), 0, "")), ") -> ", 1), rtype(r_fn_ret(ty)), 1) : (maca_ends_with(ty, "[]") ? maca_cat_own(maca_cat("Rc<Vec<", rtype(relem(ty))), ">>", 1) : ((strcmp(rmap_key(ty), "") != 0) ? maca_cat_own(maca_cat_own(maca_cat("Rc<std::collections::BTreeMap<", rtype(rmap_key(ty))), ",", 1), maca_cat_own(maca_cat(" ", rtype(rmap_val(ty))), ">>", 1), 3) : ((strcmp(ty, "float") == 0) ? "f64" : (((strcmp(ty, "str") == 0) || (strcmp(ty, "Element") == 0)) ? "String" : ((strcmp(ty, "bool") == 0) ? "bool" : (((strcmp(ty, "") == 0) || (strcmp(ty, "int") == 0)) ? "i64" : ((strcmp(ty, "Future") == 0) ? "std::thread::JoinHandle<i64>" : ty))))))));  }
const char* remit_fn(Stmt s) { const char* body = remit_stmts(s.body, 0); return (((s.body.len) == 0) ? rforeign(s) : ((strcmp(s.name, "main") == 0) ? ({ const char* params = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_param((*(Expr*)_m.data[_i]))); _r; }), ", "); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("fn __maca_main(", params), ") -> i64 { ", 1), body, 1), " }\n", 1), "fn main() { std::process::exit(__maca_main(", 1), maca_cat_own(maca_cat("", rust_argv(s.params)), ") as i32); }", 1), 3); }) : ({ const char* params = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_param((*(Expr*)_m.data[_i]))); _r; }), ", "); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("fn ", rid(s.name)), "(", 1), params, 1), ") -> ", 1), rret(s), 1), " { ", 1), body, 1), " }", 1); })));  }
const char* rforeign(Stmt s) { const char* params = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_param((*(Expr*)_m.data[_i]))); _r; }), ", "); const char* args = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(binder_name((*(Expr*)_m.data[_i]))); _r; }), ", "); const char* ret = rtype(s.ret); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("unsafe extern \"C\" { fn ", rid(s.name)), "_raw(", 1), params, 1), ") -> ", 1), ret, 1), "; }\n", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("fn ", rid(s.name)), "(", 1), params, 1), ") -> ", 1), ret, 1), 3), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" { unsafe { ", rid(s.name)), "_raw(", 1), args, 1), ") } }", 1), 3);  }
const char* rret(Stmt s) { return ((strcmp(s.ret, "") != 0) ? rtype(s.ret) : rtype(rtail_ty(s.body)));  }
const char* rtail_ty(MacaList body) { return (((body.len) == 0) ? "" : (((*(Stmt*)body.data[((body.len) - 1)]).kind != SExpr) ? "" : (*(Stmt*)body.data[((body.len) - 1)]).value.ty));  }
const char* rust_argv(MacaList params) { return (((params.len) == 0) ? "" : "Rc::new(std::env::args().skip(1).collect())");  }
const char* remit_param(Expr p) { return maca_cat_own(maca_cat_own(maca_cat("", rid(p.text)), ": ", 1), rtype(p.ty), 1);  }
const char* remit_stmts(MacaList body, long i) { return ((i >= (body.len)) ? "" : ({ long last = (i == ((body.len) - 1)); const char* here = remit_stmt((*(Stmt*)body.data[i]), last, rmut(body, (i + 1), (*(Stmt*)body.data[i]))); maca_cat_own(maca_cat_own(maca_cat("", here), " ", 1), remit_stmts(body, (i + 1)), 1); }));  }
const char* remit_stmt(Stmt s, long is_last, long movable) { return ((s.kind == SSet) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", rid(s.name)), " = ", 1), remit_expr(s.value), 1), ";", 1) : (((s.value.kind == EIf) && rmissing_else((*(Expr*)s.value.children.data[2]))) ? rif_stmt(s.value) : (((s.kind == SBind) && (strcmp(s.ret, "") != 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("let ", rmut_word(movable)), rid(s.name), 1), ": ", 1), rtype(s.ret), 1), maca_cat_own(maca_cat(" = ", remit_expr(s.value)), ";", 1), 3) : ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("let ", rmut_word(movable)), rid(s.name), 1), " = ", 1), remit_expr(s.value), 1), ";", 1) : (is_last ? remit_expr(s.value) : maca_cat_own(maca_cat("", remit_expr(s.value)), ";", 1))))));  }
const char* rmut_word(long movable) { return (movable ? "mut " : "");  }
const char* remit_module(Module m) { return maca_cat(rust_preamble(), remit_items(m.items, rlazy_names(m.items, 0, maca_listv(0)), declared_types(m.items, 0, maca_listv(0)), 0));  }
const char* remit_items(MacaList items, MacaList lz, MacaList own, long i) { return ((i >= (items.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat("", remit_item(rlazy_stmt((*(Stmt*)items.data[i]), lz), own)), "\n", 1), remit_items(items, lz, own, (i + 1)), 1));  }
MacaList rlazy_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (is_impl_block((*(Stmt*)items.data[i])) ? rlazy_names(items, (i + 1), acc) : ((((*(Stmt*)items.data[i]).kind == SBind) && (!rscalar((*(Stmt*)items.data[i]).value))) ? rlazy_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : rlazy_names(items, (i + 1), acc))));  }
long rscalar(Expr e) { return (((e.kind == EInt) || (e.kind == EFloat)) || (e.kind == EBool));  }
Stmt rlazy_stmt(Stmt s, MacaList lz) { return ({ __typeof__(s) _w = s; _w.value = rlazy_expr(s.value, lz); _w.body = rlazy_body(s.body, lz, 0, maca_listv(0)); _w; });  }
MacaList rlazy_body(MacaList body, MacaList lz, long i, MacaList acc) { return ((i >= (body.len)) ? acc : rlazy_body(body, lz, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ rlazy_stmt((*(Stmt*)body.data[i]), lz) })))));  }
Expr rlazy_expr(Expr e, MacaList lz) { return (((e.kind == EIdent) && (maca_list_index_of_str(lz, e.text) >= 0)) ? ({ __typeof__(e_call(e.text, maca_listv(0))) _w = e_call(e.text, maca_listv(0)); _w.ty = e.ty; _w; }) : ({ __typeof__(e) _w = e; _w.children = rlazy_children(e.children, lz, 0, maca_listv(0)); _w.stmts = rlazy_body(e.stmts, lz, 0, maca_listv(0)); _w; }));  }
MacaList rlazy_children(MacaList cs, MacaList lz, long i, MacaList acc) { return ((i >= (cs.len)) ? acc : rlazy_children(cs, lz, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ rlazy_expr((*(Expr*)cs.data[i]), lz) })))));  }
const char* rust_preamble() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("use std::rc::Rc;\n", "#[allow(dead_code)]\nfn maca_attr(name: String, value: String) -> String { if name.is_empty() { return String::new(); } let q = value.replace('&', \"&amp;\").replace('<', \"&lt;\").replace('>', \"&gt;\").replace('\"', \"&quot;\"); format!(\" {}={}{}{}\", name, '\"', q, '\"') }\n"), "#[allow(dead_code)]\nfn maca_flag(name: String, on: bool) -> String { if on && !name.is_empty() { format!(\" {}\", name) } else { String::new() } }\n", 1), "#[allow(dead_code)]\nfn maca_element(tag: String, attrs: String, kids: String) -> String { if [\"area\", \"base\", \"br\", \"col\", \"embed\", \"hr\", \"img\", \"input\", \"link\", \"meta\", \"source\", \"track\", \"wbr\"].contains(&tag.as_str()) { format!(\"<{}{}>\", tag, attrs) } else { format!(\"<{}{}>{}</{}>\", tag, attrs, kids, tag) } }\n", 1), "#[allow(dead_code)]\nfn maca_read_file(path: String) -> String { std::fs::read_to_string(path).unwrap_or_default() }\n", 1), "#[allow(dead_code)]\nfn maca_write_file(path: String, text: String) -> bool { std::fs::write(path, text).is_ok() }\n", 1), "#[allow(dead_code)]\nfn maca_exec(cmd: String, args: Rc<Vec<String>>) -> i64 { match std::process::Command::new(cmd).args(args.iter()).status() { Ok(s) => s.code().unwrap_or(-1) as i64, Err(_) => -1 } }\n", 1), "use std::cell::Cell;\nthread_local!(static MACA_FAILED: Cell<i64> = Cell::new(0));\n", 1), "#[allow(dead_code)]\nfn maca_failures() -> i64 { MACA_FAILED.with(|c| c.get()) }\n", 1), "#[allow(dead_code)]\nfn maca_assert(cond: bool, msg: String) -> bool { if cond { return true; } MACA_FAILED.with(|c| c.set(c.get() + 1)); eprintln!(\"assertion failed: {}\", if msg.is_empty() { \"(no message)\".to_string() } else { msg }); false }\n", 1), "#[allow(dead_code)]\nfn maca_assert_eq(got: String, want: String, msg: String) -> bool { if got == want { return true; } MACA_FAILED.with(|c| c.set(c.get() + 1)); eprintln!(\"assertion failed: {}\\n  got:  {}\\n  want: {}\", if msg.is_empty() { \"(no message)\".to_string() } else { msg }, got, want); false }\n", 1), "#[allow(dead_code)]\nfn maca_panic(msg: String) { eprintln!(\"{}\", msg); std::process::exit(1); }\n", 1), "#[allow(dead_code)]\nfn maca_sleep_ms(ms: i64) { if ms > 0 { std::thread::sleep(std::time::Duration::from_millis(ms as u64)); } }\n", 1), "#[allow(dead_code)]\nfn maca_make_dir(p: String) -> bool { std::fs::create_dir_all(p).is_ok() }\n", 1), "#[allow(dead_code)]\nfn maca_remove_dir(p: String) -> bool { std::fs::remove_dir(p).is_ok() }\n", 1), "#[allow(dead_code)]\nfn maca_remove_file(p: String) -> bool { std::fs::remove_file(p).is_ok() }\n", 1), "#[allow(dead_code)]\nfn maca_file_exists(p: String) -> bool { std::path::Path::new(&p).exists() }\n", 1), "#[allow(dead_code)]\nfn maca_is_dir(p: String) -> bool { std::path::Path::new(&p).is_dir() }\n", 1), "#[allow(dead_code)]\nfn maca_env(name: String) -> String { std::env::var(name).unwrap_or_default() }\n", 1), "#[allow(dead_code)]\nfn maca_cwd() -> String { std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default() }\n", 1), "#[allow(dead_code)]\nfn maca_substr(s: String, at: i64, n: i64) -> String { let a = (at.max(0) as usize).min(s.len()); let b = (a + n.max(0) as usize).min(s.len()); s.get(a..b).unwrap_or(\"\").to_string() }\n", 1), "#[allow(dead_code)]\nfn maca_pad_start(s: String, w: i64) -> String { let w = w.max(0) as usize; if s.len() >= w { s } else { format!(\"{}{}\", \" \".repeat(w - s.len()), s) } }\n", 1), "#[allow(dead_code)]\nfn maca_pad_end(s: String, w: i64) -> String { let w = w.max(0) as usize; if s.len() >= w { s } else { format!(\"{}{}\", s, \" \".repeat(w - s.len())) } }\n", 1), "#[allow(dead_code)]\nfn maca_try<F: FnOnce()>(f: F) -> String { let hushed = std::panic::take_hook(); std::panic::set_hook(Box::new(|_| {})); let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)); std::panic::set_hook(hushed); match out { Ok(_) => String::new(), Err(e) => e.downcast_ref::<String>().cloned().unwrap_or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()).unwrap_or_default()) } }\n", 1), "#[allow(dead_code)]\nfn maca_is_tty() -> bool { false }\n", 1), "#[allow(dead_code)]\nfn maca_chdir(p: String) -> bool { std::env::set_current_dir(p).is_ok() }\n", 1), "#[allow(dead_code)]\nfn maca_now_ms() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0) }\n", 1), "#[allow(dead_code)]\nfn maca_file_size(p: String) -> i64 { std::fs::metadata(p).map(|m| m.len() as i64).unwrap_or(-1) }\n", 1), "#[allow(dead_code)]\nfn maca_chr(b: i64) -> String { char::from_u32(b as u32).map(|c| c.to_string()).unwrap_or_default() }\n", 1), "#[allow(dead_code)]\nfn maca_ord(s: String) -> i64 { s.bytes().next().map(|b| b as i64).unwrap_or(-1) }\n", 1), "#[allow(dead_code)]\nfn maca_input(prompt: String) -> String { use std::io::Write; if !prompt.is_empty() { print!(\"{}\", prompt); let _ = std::io::stdout().flush(); } let mut s = String::new(); let _ = std::io::stdin().read_line(&mut s); s.trim_end_matches('\\n').to_string() }\n", 1), "#[allow(dead_code)]\nfn maca_list_dir(p: String) -> Rc<Vec<String>> { let mut v: Vec<String> = std::fs::read_dir(p).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default(); v.sort(); Rc::new(v) }\n", 1);  }
const char* remit_item(Stmt s, MacaList own) { return ((s.kind == SRecord) ? remit_struct(s) : ((s.kind == SSum) ? remit_sum(s) : (is_impl_block(s) ? remit_impl(s, own) : ((s.kind == SBind) ? remit_const(s) : remit_fn(s)))));  }
const char* remit_impl(Stmt s, MacaList own) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("impl ", rtype(s.ret)), " for ", 1), s.name, 1), " {\n", 1), remit_methods(s.value.children, own, 0, ""), 1), "}", 1);  }
const char* remit_methods(MacaList fs, MacaList own, long i, const char* acc) { return ((i >= (fs.len)) ? acc : remit_methods(fs, own, (i + 1), maca_cat(acc, rimpl_method((*(Expr*)fs.data[i]), own))));  }
const char* rimpl_method(Expr f, MacaList own) { Expr lam = (*(Expr*)f.children.data[1]); MacaList ps = maca_list_slice(lam.children, 0, ((lam.children.len) - 1)); const char* head = maca_list_join(rmethod_params(ps, own, 0, maca_listv(0)), ", "); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    fn ", rid((*(Expr*)f.children.data[0]).text)), "(", 1), head, 1), ")", 1), rmethod_ret(lam), 1), maca_cat_own(maca_cat(" { ", remit_expr(lambda_body(lam))), " }\n", 1), 3);  }
MacaList rmethod_params(MacaList ps, MacaList own, long i, MacaList acc) { return ((i >= (ps.len)) ? acc : (((i == 0) && (strcmp((*(Expr*)ps.data[i]).text, "self") == 0)) ? rmethod_params(ps, own, (i + 1), maca_list_cat(acc, maca_listv(1, (long)("&mut self")))) : rmethod_params(ps, own, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(rmethod_param((*(Expr*)ps.data[i]), own)))))));  }
const char* rmethod_param(Expr p, MacaList own) { return (foreign_type(p.ty, own) ? maca_cat_own(maca_cat_own(maca_cat("", rid(p.text)), ": &mut ", 1), rtype(p.ty), 1) : maca_cat_own(maca_cat_own(maca_cat("mut ", rid(p.text)), ": ", 1), rtype(p.ty), 1));  }
const char* rmethod_ret(Expr lam) { const char* said = rmethod_answer(lam); return ((strcmp(said, "") == 0) ? "" : maca_cat(" -> ", rtype(said)));  }
const char* rmethod_answer(Expr lam) { Expr body = lambda_body(lam); return (((body.kind == EWhile) || (body.kind == EFor)) ? "" : (((body.kind == EBinary) && (strcmp(body.text, "=") == 0)) ? "" : ((strcmp(lam.ty, "") != 0) ? lam.ty : body.ty)));  }
const char* remit_const(Stmt s) { return (rscalar(s.value) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("const ", rid(s.name)), ": ", 1), rconst_type(s.value), 1), " = ", 1), remit_expr(s.value), 1), ";", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("fn ", rid(s.name)), "() -> ", 1), rconst_type(s.value), 1), " { ", 1), remit_expr(s.value), 1), " }", 1));  }
const char* rconst_type(Expr e) { return ((e.kind == EFloat) ? "f64" : ((e.kind == EBool) ? "bool" : ((e.kind == EStr) ? "String" : ((e.kind == EList) ? maca_cat_own(maca_cat("Rc<Vec<", rcell_type(e.children)), ">>", 1) : ((strcmp(e.ty, "") != 0) ? rtype(e.ty) : "i64")))));  }
const char* rcell_type(MacaList cs) { return (((cs.len) == 0) ? "i64" : rconst_type((*(Expr*)cs.data[0])));  }
const char* remit_struct(Stmt s) { const char* fields = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_struct_field((*(Expr*)_m.data[_i]))); _r; }), ", "); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#[derive(Clone, PartialEq)]\nstruct ", s.name), " { ", 1), fields, 1), " }", 1);  }
const char* remit_struct_field(Expr f) { return maca_cat_own(maca_cat_own(maca_cat("", rid(f.text)), ": ", 1), rtype(f.ty), 1);  }
const char* remit_sum(Stmt s) { const char* variants = maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(remit_variant((*(Expr*)_m.data[_i]))); _r; }), ", "); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#[derive(Clone, PartialEq)]\nenum ", s.name), " { ", 1), variants, 1), " }\n", 1), maca_cat_own(maca_cat("use ", s.name), "::*;", 1), 3);  }
const char* remit_variant(Expr v) { return (((v.children.len) == 0) ? v.text : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", v.text), "(", 1), maca_list_join(({ MacaList _m = v.children; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(payload_type((*(Expr*)_m.data[_i]))); _r; }), ", "), 3), ")", 1));  }
const char* payload_type(Expr p) { return rtype(p.ty);  }
const char* jid(const char* name) { return ((maca_str_index_of(JsReserved, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0) ? maca_cat(name, "_") : name);  }
const char* js_str(const char* s) { return maca_cat_own(maca_cat("\"", s), "\"", 1);  }
long js_list(const char* ty) { return ((((int)strlen(ty)) > 2) && (strcmp(maca_str_slice(ty, (((int)strlen(ty)) - 2), ((int)strlen(ty))), "[]") == 0));  }
long js_scalar_ty(const char* ty) { return ((((strcmp(ty, "int") == 0) || (strcmp(ty, "float") == 0)) || (strcmp(ty, "bool") == 0)) || (strcmp(ty, "str") == 0));  }
long js_scalar(Expr e) { return (((((e.kind == EInt) || (e.kind == EFloat)) || (e.kind == EBool)) || (e.kind == EStr)) || js_scalar_ty(e.ty));  }
long js_own_type(const char* ty) { return ((((((strcmp(ty, "") != 0) && (strcmp(ty, "Element") != 0)) && (!js_scalar_ty(ty))) && (!js_list(ty))) && (strcmp(map_type_key(ty), "") == 0)) && (maca_str_index_of(ty, ") -> ") < 0));  }
long js_upper(const char* name) { return ((strcmp(name, "") != 0) && (strcmp(maca_lower(maca_str_at(name, 0)), maca_str_at(name, 0)) != 0));  }
long js_binder(const char* name) { return (((((strcmp(name, "") != 0) && (strcmp(name, "true") != 0)) && (strcmp(name, "false") != 0)) && (!js_upper(name))) && (!(isdigit((unsigned char)(maca_str_at(name, 0))[0]) != 0)));  }
const char* jemit_expr(Expr e) { return (e.kind == EInt ? maca_int_to_str(e.ival) : (e.kind == EFloat ? e.text : (e.kind == EStr ? js_str(e.text) : (e.kind == EBool ? e.text : (e.kind == EIdent ? jid(e.text) : (e.kind == ECall ? jemit_call(e) : (e.kind == EBinary ? jemit_binary(e) : (e.kind == ETernary ? jemit_ternary(e) : (e.kind == EIf ? jemit_ternary(e) : (e.kind == EUnary ? jemit_unary(e) : (e.kind == ERecord ? maca_cat_own(maca_cat("{ ", jemit_fields(e.children, 0)), " }", 1) : (e.kind == EWith ? jemit_with(e) : (e.kind == EBlock ? jemit_block(e) : (e.kind == EField ? maca_cat_own(maca_cat_own(maca_cat("", jemit_expr((*(Expr*)e.children.data[0]))), ".", 1), jid(e.text), 1) : (e.kind == EMethod ? jemit_method(e) : (e.kind == EList ? maca_cat_own(maca_cat("[", jemit_args(e.children, 0)), "]", 1) : (e.kind == ELambda ? jemit_lambda(e) : (e.kind == EMatch ? jemit_match(e) : (e.kind == EWhile ? jemit_while(e) : (e.kind == EFor ? jemit_for(e) : (e.kind == EJump ? jemit_jump(e) : "undefined")))))))))))))))))))));  }
const char* jemit_jump(Expr e) { return (((e.children.len) == 0) ? e.text : maca_cat_own(maca_cat_own(maca_cat("", e.text), " ", 1), jemit_expr((*(Expr*)e.children.data[0])), 1));  }
const char* jemit_unary(Expr e) { const char* inner = jemit_expr((*(Expr*)e.children.data[0])); return ((strcmp(e.text, "fail") == 0) ? maca_cat_own(maca_cat("_mfail(", inner), ")", 1) : ((strcmp(e.text, "try") == 0) ? maca_cat_own(maca_cat("_mtry(() => ", inner), ")", 1) : ((strcmp(e.text, "spawn") == 0) ? maca_cat_own(maca_cat("Promise.resolve().then(() => ", inner), ")", 1) : ((strcmp(e.text, "await") == 0) ? maca_cat_own(maca_cat("(await ", inner), ")", 1) : maca_cat_own(maca_cat_own(maca_cat("(", e.text), inner, 1), ")", 1)))));  }
const char* jemit_while(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("while (", jemit_expr((*(Expr*)e.children.data[0]))), ") { ", 1), jemit_stmts(e.stmts, 0), 1), " }", 1);  }
const char* jemit_for(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("for (let ", jid(e.text)), " of ", 1), jemit_expr((*(Expr*)e.children.data[0])), 1), ")", 1), maca_cat_own(maca_cat(" { ", jemit_stmts(e.stmts, 0)), " }", 1), 3);  }
const char* jemit_match(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(() => { const _s = ", jemit_expr((*(Expr*)e.children.data[0]))), ";", 1), maca_cat(" ", jemit_arms(e.children, 1)), 3), "throw new Error(\"no match\"); })()", 1);  }
const char* jemit_arms(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? "" : maca_cat(jemit_arm((*(Expr*)cs.data[i]), (*(Expr*)cs.data[(i + 1)])), jemit_arms(cs, (i + 2))));  }
const char* jemit_arm(Expr p, Expr body) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("if (", jpat_test(p, "_s")), ") { ", 1), jpat_binds(p, "_s"), 1), maca_cat_own(maca_cat("return ", jemit_expr(body)), "; } ", 1), 3);  }
const char* jpat_test(Expr p, const char* sv) { return ((p.kind == EGuard) ? jguard_test(p, sv) : (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? maca_cat_own(maca_cat("(", jpat_test((*(Expr*)p.children.data[0]), sv)), maca_cat_own(maca_cat(" || ", jpat_test((*(Expr*)p.children.data[1]), sv)), ")", 1), 3) : ((p.kind == EStr) ? maca_cat_own(maca_cat_own(maca_cat("", sv), " === ", 1), js_str(p.text), 1) : (((strcmp(p.text, "_") == 0) || (strcmp(p.text, "{}") == 0)) ? "true" : (((strcmp(p.text, "[]") == 0) || (strcmp(p.text, "[..]") == 0)) ? jcells_test(p, sv) : (((p.children.len) > 0) ? maca_cat_own(maca_cat_own(maca_cat("", sv), ".$ === ", 1), js_str(p.text), 1) : (js_binder(p.text) ? "true" : maca_cat_own(maca_cat_own(maca_cat("", sv), " === ", 1), jid(p.text), 1))))))));  }
const char* jguard_test(Expr p, const char* sv) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jpat_test((*(Expr*)p.children.data[0]), sv)), " && (() => {", 1), maca_cat(" ", jpat_binds((*(Expr*)p.children.data[0]), sv)), 3), maca_cat_own(maca_cat("return ", jemit_expr((*(Expr*)p.children.data[1]))), "; })()", 1), 3);  }
const char* jcells_test(Expr p, const char* sv) { long n = js_cell_count(p); const char* how = ((strcmp(p.text, "[..]") == 0) ? ">=" : "==="); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Array.isArray(", sv), ") && ", 1), sv, 1), ".length ", 1), how, 1), " ", 1), maca_int_to_str(n), 3), jcell_tests(p, sv, 0, n), 1);  }
long js_cell_count(Expr p) { return ((strcmp(p.text, "[..]") == 0) ? ((p.children.len) - 1) : (p.children.len));  }
const char* jcell_tests(Expr p, const char* sv, long i, long n) { return ((i >= n) ? "" : ({ const char* one = jpat_test((*(Expr*)p.children.data[i]), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", sv), "[", 1), maca_int_to_str(i), 3), "]", 1)); const char* here = ((strcmp(one, "true") == 0) ? "" : maca_cat_own(maca_cat(" && (", one), ")", 1)); maca_cat(here, jcell_tests(p, sv, (i + 1), n)); }));  }
const char* jpat_binds(Expr p, const char* sv) { return ((p.kind == EGuard) ? jpat_binds((*(Expr*)p.children.data[0]), sv) : (((p.kind == EStr) || ((p.kind == EBinary) && (strcmp(p.text, "|") == 0))) ? "" : ((strcmp(p.text, "{}") == 0) ? jfield_binds(p.children, sv, 0) : (((strcmp(p.text, "[]") == 0) || (strcmp(p.text, "[..]") == 0)) ? jcell_binds(p, sv, 0, js_cell_count(p)) : (((p.children.len) > 0) ? jpayload_binds(p.children, sv, 0) : (((strcmp(p.text, "_") != 0) && js_binder(p.text)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("var ", jid(p.text)), " = ", 1), sv, 1), "; ", 1) : ""))))));  }
const char* jfield_binds(MacaList fs, const char* sv, long i) { return ((i >= (fs.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("var ", jid((*(Expr*)fs.data[i]).text)), " = ", 1), sv, 1), ".", 1), jid((*(Expr*)fs.data[i]).text), 1), "; ", 1), jfield_binds(fs, sv, (i + 1)), 1));  }
const char* jpayload_binds(MacaList cs, const char* sv, long i) { return ((i >= (cs.len)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("var ", jid((*(Expr*)cs.data[i]).text)), " = ", 1), sv, 1), "._", 1), maca_int_to_str(i), 3), "; ", 1), jpayload_binds(cs, sv, (i + 1)), 1));  }
const char* jcell_binds(Expr p, const char* sv, long i, long n) { return ((i >= (p.children.len)) ? "" : ((i >= n) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("var ", jid((*(Expr*)p.children.data[i]).text)), " = ", 1), sv, 1), ".slice(", 1), maca_int_to_str(n), 3), "); ", 1) : maca_cat(jpat_binds((*(Expr*)p.children.data[i]), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", sv), "[", 1), maca_int_to_str(i), 3), "]", 1)), jcell_binds(p, sv, (i + 1), n))));  }
const char* jemit_ternary(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", jemit_expr((*(Expr*)e.children.data[0]))), " ? ", 1), jemit_expr((*(Expr*)e.children.data[1])), 1), maca_cat_own(maca_cat(" : ", jemit_else((*(Expr*)e.children.data[2]))), ")", 1), 3);  }
const char* jemit_else(Expr e) { return (((e.kind == EIdent) && (strcmp(e.text, "?") == 0)) ? "undefined" : jemit_expr(e));  }
const char* jemit_binary(Expr e) { Expr lhs = (*(Expr*)e.children.data[0]); const char* l = jemit_expr(lhs); const char* r = jemit_expr((*(Expr*)e.children.data[1])); return ((((strcmp(e.text, "=") == 0) && (lhs.kind == EMethod)) && (strcmp(lhs.text, "get") == 0)) ? jstore(lhs, r) : ((strcmp(e.text, "..") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mrange(", l), ", ", 1), r, 1), ")", 1) : (js_joins(e) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), ").concat(", 1), r, 1), ")", 1) : (js_deep(e) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", js_not(e.text)), "_meq(", 1), l, 1), ", ", 1), r, 1), ")", 1) : (((strcmp(e.text, "==") == 0) || (strcmp(e.text, "!=") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " ", 1), e.text, 1), "= ", 1), r, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " ", 1), e.text, 1), " ", 1), r, 1), ")", 1))))));  }
const char* js_not(const char* op) { return ((strcmp(op, "!=") == 0) ? "!" : "");  }
long js_joins(Expr e) { return ((strcmp(e.text, "++") == 0) || (((strcmp(e.text, "+") == 0) && js_list((*(Expr*)e.children.data[0]).ty)) && js_list((*(Expr*)e.children.data[1]).ty)));  }
long js_deep(Expr e) { return ((((strcmp(e.text, "==") == 0) || (strcmp(e.text, "!=") == 0)) && (!js_scalar((*(Expr*)e.children.data[0])))) && (!js_scalar((*(Expr*)e.children.data[1]))));  }
const char* jstore(Expr l, const char* value) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jemit_expr((*(Expr*)l.children.data[0]))), "[", 1), jemit_expr((*(Expr*)l.children.data[1])), 1), "]", 1), maca_cat(" = ", value), 3);  }
const char* jarg(Expr e, long i) { return ((i >= (e.children.len)) ? "undefined" : jemit_expr((*(Expr*)e.children.data[i])));  }
const char* js_math(const char* name) { return (((strcmp(name, "signum") == 0) || (strcmp(name, "sign") == 0)) ? "sign" : ((((((((((((((strcmp(name, "abs") == 0) || (strcmp(name, "sqrt") == 0)) || (strcmp(name, "floor") == 0)) || (strcmp(name, "ceil") == 0)) || (strcmp(name, "round") == 0)) || (strcmp(name, "sin") == 0)) || (strcmp(name, "cos") == 0)) || (strcmp(name, "tan") == 0)) || (strcmp(name, "exp") == 0)) || (strcmp(name, "log") == 0)) || (strcmp(name, "pow") == 0)) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) ? name : ""));  }
const char* jemit_call(Expr e) { const char* args = jemit_args(e.children, 0); const char* one = jarg(e, 0); return ((strcmp(e.text, "str") == 0) ? maca_cat_own(maca_cat("_mstr(", one), ")", 1) : ((strcmp(e.text, "int") == 0) ? maca_cat_own(maca_cat("_mint(", one), ")", 1) : ((strcmp(e.text, "float") == 0) ? maca_cat_own(maca_cat("Number(", one), ")", 1) : ((strcmp(e.text, "len") == 0) ? maca_cat_own(maca_cat("(", one), ").length", 1) : ((strcmp(e.text, "chr") == 0) ? maca_cat_own(maca_cat("_mchr(", one), ")", 1) : ((strcmp(e.text, "ord") == 0) ? maca_cat_own(maca_cat("_mord(", one), ")", 1) : ((strcmp(e.text, "info") == 0) ? maca_cat_own(maca_cat("_minfo(", one), ")", 1) : ((strcmp(e.text, "print") == 0) ? maca_cat_own(maca_cat("_mprint(", one), ")", 1) : (((strcmp(e.text, "err") == 0) || (strcmp(e.text, "warn") == 0)) ? maca_cat_own(maca_cat("_merr(", one), ")", 1) : ((strcmp(e.text, "assert") == 0) ? maca_cat_own(maca_cat("_massert(", args), ")", 1) : ((strcmp(e.text, "assert_eq") == 0) ? maca_cat_own(maca_cat("_massert_eq(", args), ")", 1) : ((strcmp(e.text, "failures") == 0) ? "_mfailures()" : ((strcmp(e.text, "now_ms") == 0) ? "Date.now()" : ((strcmp(e.text, "sleep_ms") == 0) ? maca_cat_own(maca_cat("_msleep(", one), ")", 1) : ((strcmp(e.text, "panic") == 0) ? maca_cat_own(maca_cat("_mfail(", one), ")", 1) : (((strcmp(e.text, "map") == 0) && ((e.children.len) == 0)) ? "new Map()" : ((strcmp(e.text, "clamp") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Math.min(Math.max(", one), ", ", 1), jarg(e, 1), 1), "), ", 1), jarg(e, 2), 1), ")", 1) : ((strcmp(e.text, "gcd") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mgcd(", one), ", ", 1), jarg(e, 1), 1), ")", 1) : ((strcmp(js_math(e.text), "") != 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Math.", js_math(e.text)), "(", 1), args, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jid(e.text)), "(", 1), args, 1), ")", 1))))))))))))))))))));  }
long js_map_method(const char* name) { return (((((((((strcmp(name, "get") == 0) || (strcmp(name, "set") == 0)) || (strcmp(name, "has") == 0)) || (strcmp(name, "keys") == 0)) || (strcmp(name, "values") == 0)) || (strcmp(name, "remove") == 0)) || (strcmp(name, "contains") == 0)) || (strcmp(name, "length") == 0)) || (strcmp(name, "count") == 0));  }
const char* jemit_map_method(Expr e, const char* recv) { const char* key = jarg(e, 1); return ((strcmp(e.text, "keys") == 0) ? maca_cat_own(maca_cat("_msort([...(", recv), ").keys()])", 1) : ((strcmp(e.text, "values") == 0) ? maca_cat_own(maca_cat("[...(", recv), ").values()]", 1) : (((strcmp(e.text, "length") == 0) || (strcmp(e.text, "count") == 0)) ? maca_cat_own(maca_cat("(", recv), ").size", 1) : (((strcmp(e.text, "has") == 0) || (strcmp(e.text, "contains") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").has(", 1), key, 1), ")", 1) : ((strcmp(e.text, "set") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("new Map(", recv), ").set(", 1), key, 1), ", ", 1), jarg(e, 2), 1), ")", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mdel(", recv), ", ", 1), key, 1), ")", 1) : (((e.children.len) > 2) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mor(", recv), ", ", 1), key, 1), ", ", 1), jarg(e, 2), 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mor(", recv), ", ", 1), key, 1), ", ", 1), js_zero(map_type_val((*(Expr*)e.children.data[0]).ty)), 1), ")", 1))))))));  }
const char* js_zero(const char* ty) { return (((strcmp(ty, "int") == 0) || (strcmp(ty, "float") == 0)) ? "0" : ((strcmp(ty, "bool") == 0) ? "false" : ((strcmp(ty, "str") == 0) ? "\"\"" : "undefined")));  }
const char* jemit_method(Expr e) { const char* recv = jemit_expr((*(Expr*)e.children.data[0])); const char* rty = (*(Expr*)e.children.data[0]).ty; return (js_own_type(rty) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jid(e.text)), "(", 1), jemit_args(e.children, 0), 1), ")", 1) : (((strcmp(map_type_key(rty), "") != 0) && js_map_method(e.text)) ? jemit_map_method(e, recv) : jtext_method(e, recv)));  }
const char* jtext_method(Expr e, const char* recv) { const char* a0 = jarg(e, 1); const char* a1 = jarg(e, 2); return (((strcmp(e.text, "length") == 0) || (strcmp(e.text, "count") == 0)) ? maca_cat_own(maca_cat("(", recv), ").length", 1) : (((strcmp(e.text, "get") == 0) || (strcmp(e.text, "at") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ")[", 1), a0, 1), "]", 1) : ((strcmp(e.text, "slice") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").slice(", 1), a0, 1), ", ", 1), a1, 1), ")", 1) : ((strcmp(e.text, "index_of") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").indexOf(", 1), a0, 1), ")", 1) : (((strcmp(e.text, "contains") == 0) || (strcmp(e.text, "has") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mhas(", recv), ", ", 1), a0, 1), ")", 1) : ((strcmp(e.text, "chars") == 0) ? maca_cat_own(maca_cat("Array.from(", recv), ")", 1) : ((strcmp(e.text, "upper") == 0) ? maca_cat_own(maca_cat("(", recv), ").toUpperCase()", 1) : ((strcmp(e.text, "lower") == 0) ? maca_cat_own(maca_cat("(", recv), ").toLowerCase()", 1) : ((strcmp(e.text, "trim") == 0) ? maca_cat_own(maca_cat("(", recv), ").trim()", 1) : ((strcmp(e.text, "starts_with") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").startsWith(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "ends_with") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").endsWith(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "replace") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").split(", 1), a0, 1), ").join(", 1), a1, 1), ")", 1) : ((strcmp(e.text, "repeat") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").repeat(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "substr") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_msubstr(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ")", 1) : ((strcmp(e.text, "split") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").split(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "join") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").join(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "fixed") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Number(", recv), ").toFixed(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "pad_start") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mpad(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ", 0)", 1) : ((strcmp(e.text, "pad_end") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mpad(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ", 1)", 1) : ((strcmp(e.text, "pad_center") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mpad(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ", 2)", 1) : ((strcmp(e.text, "is_whitespace") == 0) ? maca_cat_own(maca_cat("_mclass(", recv), ", 0)", 1) : ((strcmp(e.text, "is_ascii_digit") == 0) ? maca_cat_own(maca_cat("_mclass(", recv), ", 1)", 1) : ((strcmp(e.text, "is_alpha") == 0) ? maca_cat_own(maca_cat("_mclass(", recv), ", 2)", 1) : jlist_method(e, recv))))))))))))))))))))))));  }
const char* jlist_method(Expr e, const char* recv) { const char* a0 = jarg(e, 1); const char* a1 = jarg(e, 2); return (((strcmp(e.text, "map") == 0) || (strcmp(e.text, "parallel") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").map(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "filter") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").filter(", 1), a0, 1), ")", 1) : (((strcmp(e.text, "reduce") == 0) || (strcmp(e.text, "fold") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mfold(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ")", 1) : ((strcmp(e.text, "index_of_by") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", recv), ").findIndex(", 1), a0, 1), ")", 1) : ((strcmp(e.text, "sort") == 0) ? maca_cat_own(maca_cat("_msort(", recv), ")", 1) : ((strcmp(e.text, "sort_by") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_msortby(", recv), ", ", 1), a0, 1), ")", 1) : ((strcmp(e.text, "reverse") == 0) ? maca_cat_own(maca_cat("[...(", recv), ")].reverse()", 1) : ((strcmp(e.text, "push") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("[...(", recv), "), ", 1), a0, 1), "]", 1) : ((strcmp(e.text, "pop") == 0) ? maca_cat_own(maca_cat("(", recv), ").slice(0, -1)", 1) : ((strcmp(e.text, "set") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mset(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ")", 1) : ((strcmp(e.text, "insert") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mins(", recv), ", ", 1), a0, 1), ", ", 1), a1, 1), ")", 1) : ((strcmp(e.text, "remove") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_mrem(", recv), ", ", 1), a0, 1), ")", 1) : ((strcmp(e.text, "enumerate") == 0) ? maca_cat_own(maca_cat("(", recv), ").map((v, i) => ({ index: i, value: v }))", 1) : ((strcmp(e.text, "sum") == 0) ? maca_cat_own(maca_cat("(", recv), ").reduce((_a, _b) => _a + _b, 0)", 1) : ((strcmp(e.text, "min") == 0) ? maca_cat_own(maca_cat("_mpick(", recv), ", -1)", 1) : ((strcmp(e.text, "max") == 0) ? maca_cat_own(maca_cat("_mpick(", recv), ", 1)", 1) : ((strcmp(e.text, "first") == 0) ? maca_cat_own(maca_cat("(", recv), ")[0]", 1) : ((strcmp(e.text, "last") == 0) ? maca_cat_own(maca_cat("_mlast(", recv), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jid(e.text)), "(", 1), jemit_args(e.children, 0), 1), ")", 1)))))))))))))))))));  }
const char* jemit_lambda(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", jemit_params(lambda_params(e), 0)), ") => ", 1), jemit_expr(lambda_body(e)), 1), ")", 1);  }
const char* jemit_with(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("{ ...", jemit_expr((*(Expr*)e.children.data[0]))), ",", 1), maca_cat_own(maca_cat(" ", jemit_fields(e.children, 1)), " }", 1), 3);  }
const char* jemit_fields(MacaList fs, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(fs, i, (fs.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(jemit_field((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* jemit_field(Expr f) { return maca_cat_own(maca_cat_own(maca_cat("", jid((*(Expr*)f.children.data[0]).text)), ": ", 1), jemit_expr((*(Expr*)f.children.data[1])), 1);  }
const char* jemit_args(MacaList xs, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(xs, i, (xs.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(jemit_expr((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* jemit_params(MacaList ps, long i) { return maca_list_join(({ MacaList _m = maca_list_slice(ps, i, (ps.len)); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(js_binder_name((*(Expr*)_m.data[_i]))); _r; }), ", ");  }
const char* js_binder_name(Expr p) { return jid(p.text);  }
const char* jemit_block(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("(() => { ", jemit_stmts(e.stmts, 0)), " return", 1), maca_cat_own(maca_cat(" ", jemit_expr((*(Expr*)e.children.data[0]))), "; })()", 1), 3);  }
const char* jemit_stmts(MacaList body, long i) { return ((i >= (body.len)) ? "" : ((i == ((body.len) - 1)) ? jemit_stmt((*(Stmt*)body.data[i])) : maca_cat_own(maca_cat_own(maca_cat("", jemit_stmt((*(Stmt*)body.data[i]))), " ", 1), jemit_stmts(body, (i + 1)), 1)));  }
const char* jemit_stmt(Stmt s) { return ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("let ", jid(s.name)), " = ", 1), jemit_expr(s.value), 1), ";", 1) : ((s.kind == SSet) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jid(s.name)), " = ", 1), jemit_expr(s.value), 1), ";", 1) : jstmt_of(s.value)));  }
const char* jstmt_of(Expr e) { return ((e.kind == EIf) ? jif_stmt(e) : ((e.kind == EMatch) ? jmatch_stmt(e) : ((e.kind == EBlock) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{ ", jemit_stmts(e.stmts, 0)), " ", 1), jstmt_of((*(Expr*)e.children.data[0])), 1), " }", 1) : (js_loop(e) ? jemit_expr(e) : maca_cat_own(maca_cat("", jemit_expr(e)), ";", 1)))));  }
const char* jmatch_stmt(Expr e) { return maca_cat_own(maca_cat_own(maca_cat("{ const _s = ", jemit_expr((*(Expr*)e.children.data[0]))), ";", 1), maca_cat_own(maca_cat(" ", jarms_stmt(e.children, 1)), " }", 1), 3);  }
const char* jarms_stmt(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? "" : ({ const char* rest = jarms_stmt(cs, (i + 2)); const char* here = maca_cat_own(maca_cat_own(maca_cat("if (", jpat_test((*(Expr*)cs.data[i]), "_s")), ") {", 1), maca_cat_own(maca_cat_own(maca_cat(" ", jpat_binds((*(Expr*)cs.data[i]), "_s")), jstmt_of((*(Expr*)cs.data[(i + 1)])), 1), " }", 1), 3); ((strcmp(rest, "") == 0) ? here : maca_cat_own(maca_cat_own(maca_cat("", here), " else ", 1), rest, 1)); }));  }
const char* jif_stmt(Expr e) { const char* cond = jemit_expr((*(Expr*)e.children.data[0])); const char* then = jbranch((*(Expr*)e.children.data[1])); Expr els = (*(Expr*)e.children.data[2]); return (((els.kind == EIdent) && (strcmp(els.text, "?") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("if (", cond), ") ", 1), then, 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("if (", cond), ") ", 1), then, 1), " else ", 1), jbranch(els), 1));  }
const char* jbranch(Expr e) { return ((e.kind == EIf) ? jif_stmt(e) : ((e.kind == EBlock) ? jstmt_of(e) : maca_cat_own(maca_cat("{ ", jstmt_of(e)), " }", 1)));  }
long js_loop(Expr e) { return ((e.kind == EWhile) || (e.kind == EFor));  }
const char* jemit_fn(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", js_waiting(s.body)), "function ", 1), jid(s.name), 1), "(", 1), jemit_params(s.params, 0), 1), ")", 1), maca_cat_own(maca_cat(" { ", jemit_body(s.body, 0)), " }", 1), 3);  }
const char* js_waiting(MacaList body) { return (js_waits_in(body, 0) ? "async " : "");  }
long js_waits_in(MacaList body, long i) { return ((i >= (body.len)) ? 0 : (js_waits((*(Stmt*)body.data[i]).value) || js_waits_in(body, (i + 1))));  }
long js_waits(Expr e) { return ((((e.kind == EUnary) && (strcmp(e.text, "await") == 0)) || js_waits_in(e.stmts, 0)) || js_waits_any(e.children, 0));  }
long js_waits_any(MacaList cs, long i) { return ((i >= (cs.len)) ? 0 : (js_waits((*(Expr*)cs.data[i])) || js_waits_any(cs, (i + 1))));  }
const char* jemit_body(MacaList body, long i) { return ((i >= (body.len)) ? "" : ((((i == ((body.len) - 1)) && ((*(Stmt*)body.data[i]).kind == SExpr)) && (!js_loop((*(Stmt*)body.data[i]).value))) ? maca_cat_own(maca_cat("return ", jemit_expr((*(Stmt*)body.data[i]).value)), ";", 1) : maca_cat_own(maca_cat_own(maca_cat("", jemit_stmt((*(Stmt*)body.data[i]))), " ", 1), jemit_body(body, (i + 1)), 1)));  }
const char* jemit_module(Module m) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(JsPreamble, jemit_variants(m.items, 0)), jemit_consts(m.items, 0), 1), jemit_items(m.items, 0), 1), jemit_exports(m.items), 1), jemit_entry(m.items, 0), 1);  }
const char* jemit_variants(MacaList items, long i) { return ((i >= (items.len)) ? "" : (((*(Stmt*)items.data[i]).kind != SSum) ? jemit_variants(items, (i + 1)) : maca_cat_own(maca_list_join(({ MacaList _m = (*(Stmt*)items.data[i]).params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(jemit_variant((*(Expr*)_m.data[_i]))); _r; }), ""), jemit_variants(items, (i + 1)), 1)));  }
const char* jemit_variant(Expr v) { return (((v.children.len) == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("const ", jid(v.text)), " = ", 1), js_str(v.text), 1), ";\n", 1) : ({ const char* ps = js_payload_names(v.children, 0); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("function ", jid(v.text)), "(", 1), ps, 1), ")", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" { return { $: ", js_str(v.text)), ", ", 1), ps, 1), " }; }\n", 1), 3); }));  }
const char* js_payload_names(MacaList cs, long i) { return ((i >= (cs.len)) ? "" : ((i == ((cs.len) - 1)) ? maca_cat_own("_", maca_int_to_str(i), 2) : maca_cat_own(maca_cat_own(maca_cat_own("_", maca_int_to_str(i), 2), ", ", 1), js_payload_names(cs, (i + 1)), 1)));  }
const char* jemit_consts(MacaList items, long i) { return ((i >= (items.len)) ? "" : ((((*(Stmt*)items.data[i]).kind != SBind) || (maca_str_index_of((*(Stmt*)items.data[i]).name, ".") >= 0)) ? jemit_consts(items, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("let ", jid((*(Stmt*)items.data[i]).name)), " = ", 1), jemit_expr((*(Stmt*)items.data[i]).value), 1), ";\n", 1), jemit_consts(items, (i + 1)), 1)));  }
const char* jemit_exports(MacaList items) { const char* names = js_exported(items, 0); return ((strcmp(names, "") == 0) ? "" : maca_cat_own("if (typeof module !== \"undefined\")", maca_cat_own(maca_cat(" Object.assign(module.exports, { ", names), "});\n", 1), 2));  }
const char* js_exported(MacaList items, long i) { return ((i >= (items.len)) ? "" : maca_cat(js_export_of((*(Stmt*)items.data[i])), js_exported(items, (i + 1))));  }
const char* js_export_of(Stmt s) { return (((s.kind == SFn) && ((s.body.len) > 0)) ? maca_cat_own(maca_cat("", jid(s.name)), ", ", 1) : ((s.kind == SSum) ? maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(js_variant_ref((*(Expr*)_m.data[_i]))); _r; }), "") : ""));  }
const char* js_variant_ref(Expr v) { return maca_cat_own(maca_cat("", jid(v.text)), ", ", 1);  }
const char* jemit_entry(MacaList items, long i) { return ((i >= (items.len)) ? "" : ((((*(Stmt*)items.data[i]).kind == SFn) && (strcmp((*(Stmt*)items.data[i]).name, "main") == 0)) ? "\nmain();\n" : jemit_entry(items, (i + 1))));  }
const char* jemit_items(MacaList items, long i) { return ((i >= (items.len)) ? "" : (((*(Stmt*)items.data[i]).kind != SFn) ? jemit_items(items, (i + 1)) : ((((*(Stmt*)items.data[i]).body.len) == 0) ? jemit_items(items, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat("", jemit_fn((*(Stmt*)items.data[i]))), "\n", 1), jemit_items(items, (i + 1)), 1))));  }
const char* nemit_module(Module m) { return maca_cat_own(maca_cat_own(maca_cat("{ config, pkgs, lib, ... }:\n{\n", nix_block(nix_binds(m.items, 0, 0, maca_listv(0)), 1, 0, "")), nix_home(nix_binds(m.items, 0, 1, maca_listv(0)), NixUser), 1), "}\n", 1);  }
const char* nix_home(MacaList ls, const char* user) { return (((ls.len) == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("  home-manager.users.", user), " = {\n", 1), nix_block(ls, 2, 0, ""), 1), "  };\n", 1));  }
MacaList nix_binds(MacaList items, long i, long home, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind != SBind) ? nix_binds(items, (i + 1), home, acc) : ((nix_at_home((*(Stmt*)items.data[i])) == home) ? nix_binds(items, (i + 1), home, maca_list_cat(acc, maca_listv(1, (long)(nemit_bind((*(Stmt*)items.data[i])))))) : nix_binds(items, (i + 1), home, acc))));  }
const char* nix_block(MacaList ls, long n, long i, const char* acc) { return ((i >= (ls.len)) ? acc : nix_block(ls, n, (i + 1), maca_cat(acc, nix_indent(((const char*)ls.data[i]), n))));  }
const char* nix_indent(const char* s, long n) { return nix_padded(maca_split(s, "\n"), nix_pad(n, ""), 0, "");  }
const char* nix_pad(long n, const char* acc) { return ((n <= 0) ? acc : nix_pad((n - 1), maca_cat(acc, "  ")));  }
const char* nix_padded(MacaList ls, const char* pad, long i, const char* acc) { return ((i >= (ls.len)) ? acc : nix_padded(ls, pad, (i + 1), maca_cat_own(maca_cat_own(maca_cat(acc, pad), ((const char*)ls.data[i]), 1), "\n", 1)));  }
long nix_at_home(Stmt s) { MacaList p = maca_split(s.name, "."); return ((nix_program(s) || nix_path2(p, "user", "packages")) || nix_path3(p, "user", "home", "dirs"));  }
long nix_program(Stmt s) { return ((strcmp(s.ret, "") != 0) && ((maca_split(s.name, ".").len) == 1));  }
const char* nemit_bind(Stmt s) { MacaList p = maca_split(s.name, "."); return (nix_program(s) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("programs.", ((const char*)p.data[0])), " = ", 1), nix_enabled(s.value), 1), ";", 1) : (nix_path2(p, "system", "packages") ? maca_cat_own(maca_cat("environment.systemPackages = ", nix_pkg_list(s.value)), ";", 1) : (nix_path2(p, "system", "fonts") ? maca_cat_own(maca_cat("fonts.packages = ", nix_pkg_list(s.value)), ";", 1) : (nix_path2(p, "user", "packages") ? maca_cat_own(maca_cat("home.packages = ", nix_pkg_list(s.value)), ";", 1) : (nix_path3(p, "user", "home", "dirs") ? nix_xdg_dirs(s.value) : ((((p.len) == 2) && (strcmp(((const char*)p.data[0]), "services") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("services.", ((const char*)p.data[1])), " = ", 1), nix_enabled(s.value), 1), ";", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), " = ", 1), nvalue(s.value), 1), ";", 1)))))));  }
long nix_path2(MacaList p, const char* a, const char* b) { return ((((p.len) == 2) && (strcmp(((const char*)p.data[0]), a) == 0)) && (strcmp(((const char*)p.data[1]), b) == 0));  }
long nix_path3(MacaList p, const char* a, const char* b, const char* c) { return (((((p.len) == 3) && (strcmp(((const char*)p.data[0]), a) == 0)) && (strcmp(((const char*)p.data[1]), b) == 0)) && (strcmp(((const char*)p.data[2]), c) == 0));  }
const char* nix_enabled(Expr v) { return ((v.kind != ERecord) ? nvalue(v) : maca_cat_own(maca_cat("{\n  enable = true;\n", nix_body(nix_fields(v.children, 0, maca_listv(0)), 0, "")), "}", 1));  }
const char* nix_body(MacaList ls, long i, const char* acc) { return ((i >= (ls.len)) ? acc : nix_body(ls, (i + 1), maca_cat_own(maca_cat_own(maca_cat(acc, "  "), ((const char*)ls.data[i]), 1), "\n", 1)));  }
const char* nix_pkg_list(Expr v) { return maca_cat_own(maca_cat_own("[ ", maca_list_join(({ MacaList _m = nix_elems(v); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(nix_pkg_ref((*(Expr*)_m.data[_i]))); _r; }), " "), 2), " ]", 1);  }
MacaList nix_elems(Expr v) { return ((v.kind == EList) ? v.children : maca_listv(1, maca_box(sizeof(Expr), (Expr[]){ v })));  }
const char* nix_pkg_ref(Expr e) { return (((e.kind == EIdent) || (e.kind == EField)) ? maca_cat("pkgs.", e.text) : ((e.kind == EStr) ? nix_string(e.text) : "pkgs.unknown"));  }
const char* nvalue(Expr e) { return (e.kind == EStr ? nix_string(e.text) : (e.kind == EInt ? maca_int_to_str(e.ival) : (e.kind == EFloat ? e.text : (e.kind == EBool ? e.text : (e.kind == EIdent ? maca_cat("pkgs.", e.text) : (e.kind == EField ? maca_cat("pkgs.", e.text) : (e.kind == EList ? maca_cat_own(maca_cat_own("[ ", maca_list_join(({ MacaList _m = e.children; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(nvalue((*(Expr*)_m.data[_i]))); _r; }), " "), 2), " ]", 1) : (e.kind == ERecord ? nix_attrs(e.children) : (e.kind == EUnary ? nix_unary(e) : (e.kind == EBinary ? nix_binary(e) : (e.kind == ETernary ? nix_cond(e) : "null")))))))))));  }
const char* nix_attrs(MacaList fs) { MacaList ls = nix_fields(fs, 0, maca_listv(0)); return (((ls.len) == 0) ? "{ }" : maca_cat_own(maca_cat_own("{\n  ", maca_list_join(ls, "\n  "), 2), "\n}", 1));  }
MacaList nix_fields(MacaList fs, long i, MacaList acc) { return ((i >= (fs.len)) ? acc : nix_fields(fs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(nix_field((*(Expr*)fs.data[i])))))));  }
const char* nix_field(Expr f) { const char* name = (*(Expr*)f.children.data[0]).text; Expr v = (*(Expr*)f.children.data[1]); return (((v.kind == EIdent) && (strcmp(v.text, name) == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", name), " = ", 1), name, 1), ";", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", name), " = ", 1), nvalue(v), 1), ";", 1));  }
const char* nix_unary(Expr e) { return (((strcmp(e.text, "-") == 0) || (strcmp(e.text, "!") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("(", e.text), nvalue((*(Expr*)e.children.data[0])), 1), ")", 1) : "null");  }
const char* nix_cond(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(if ", nvalue((*(Expr*)e.children.data[0]))), " then ", 1), nvalue((*(Expr*)e.children.data[1])), 1), maca_cat_own(maca_cat(" else ", nvalue((*(Expr*)e.children.data[2]))), ")", 1), 3);  }
const char* nix_binary(Expr e) { const char* l = nvalue((*(Expr*)e.children.data[0])); const char* r = nvalue((*(Expr*)e.children.data[1])); return ((strcmp(e.text, "/") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(builtins.div ", l), " ", 1), r, 1), ")", 1) : ((strcmp(e.text, "%") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " - (builtins.div ", 1), l, 1), " ", 1), r, 1), ") * ", 1), r, 1), ")", 1) : ((strcmp(e.text, "++") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " + ", 1), r, 1), ")", 1) : (nix_infix(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " ", 1), e.text, 1), " ", 1), r, 1), ")", 1) : "null"))));  }
long nix_infix(const char* op) { return (((((((((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "*") == 0)) || (strcmp(op, "==") == 0)) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">=") == 0)) || (strcmp(op, "&&") == 0)) || (strcmp(op, "||") == 0));  }
const char* nix_xdg_dirs(Expr v) { return maca_cat_own(maca_cat("xdg.userDirs = {\n  enable = true;\n  createDirectories = true;\n", nix_dir_lines(nix_elems(v), 0, "")), "};", 1);  }
const char* nix_dir_lines(MacaList xs, long i, const char* acc) { return ((i >= (xs.len)) ? acc : (((*(Expr*)xs.data[i]).kind != EStr) ? nix_dir_lines(xs, (i + 1), acc) : nix_dir_lines(xs, (i + 1), maca_cat(acc, nix_dir_line((*(Expr*)xs.data[i]).text)))));  }
const char* nix_dir_line(const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("  ", nix_xdg_key(name)), " = \"$HOME/", 1), name, 1), "\";\n", 1);  }
const char* nix_xdg_key(const char* name) { const char* low = maca_lower(name); return ((strcmp(low, "downloads") == 0) ? "download" : ((strcmp(low, "public") == 0) ? "publicShare" : low));  }
const char* nix_string(const char* s) { return maca_cat_own(maca_cat("\"", nix_escaped(maca_chars(s), 0, "")), "\"", 1);  }
const char* nix_escaped(MacaList cs, long i, const char* acc) { return ((i >= (cs.len)) ? acc : nix_escaped(cs, (i + 1), maca_cat(acc, nix_escape(((const char*)cs.data[i])))));  }
const char* nix_escape(const char* c) { return ((strcmp(c, "\"") == 0) ? "\\\"" : ((strcmp(c, "\\") == 0) ? "\\\\" : ((strcmp(c, "\n") == 0) ? "\\n" : c)));  }
Mcu emb_mcu(const char* name) { return (((strcmp(name, "cortex-m0") == 0) || (strcmp(name, "cortex-m0plus") == 0)) ? (Mcu){ .name = "cortex-m0", .triple = "thumbv6m-none-eabi", .cpu = "cortex-m0", .flash = "0x08000000", .flash_k = 64, .ram = "0x20000000", .ram_k = 8 } : ((strcmp(name, "cortex-m3") == 0) ? (Mcu){ .name = "cortex-m3", .triple = "thumbv7m-none-eabi", .cpu = "cortex-m3", .flash = "0x08000000", .flash_k = 256, .ram = "0x20000000", .ram_k = 64 } : ((((strcmp(name, "cortex-m4") == 0) || (strcmp(name, "") == 0)) || (strcmp(name, "default") == 0)) ? (Mcu){ .name = "cortex-m4", .triple = "thumbv7em-none-eabi", .cpu = "cortex-m4", .flash = "0x08000000", .flash_k = 512, .ram = "0x20000000", .ram_k = 128 } : ((strcmp(name, "riscv32") == 0) ? (Mcu){ .name = "riscv32", .triple = "riscv32-none-elf", .cpu = "generic-rv32", .flash = "0x20000000", .flash_k = 512, .ram = "0x80000000", .ram_k = 128 } : (Mcu){ .name = "", .triple = "", .cpu = "", .flash = "", .flash_k = 0, .ram = "", .ram_k = 0 }))));  }
const char* emb_linker_script(Mcu m) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("ENTRY(Reset_Handler)\nMEMORY {\n", maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("  FLASH (rx) : ORIGIN = ", m.flash), ", LENGTH = ", 1), maca_int_to_str(m.flash_k), 3), "K\n", 1), 2), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("  RAM  (rwx) : ORIGIN = ", m.ram), ", LENGTH = ", 1), maca_int_to_str(m.ram_k), 3), "K\n}\n", 1), 3), "_estack = ORIGIN(RAM) + LENGTH(RAM);\nSECTIONS {\n", 1), "  .isr_vector : { KEEP(*(.isr_vector)) } > FLASH\n", 1), "  .text : { *(.text*) *(.rodata*) } > FLASH\n", 1), "  _sidata = LOADADDR(.data);\n", 1), "  .data : { _sdata = .; *(.data*) . = ALIGN(4); _edata = .; }", 1), " > RAM AT> FLASH\n", 1), "  .bss  : { _sbss = .; *(.bss* COMMON) . = ALIGN(4); _ebss = .; }", 1), " > RAM\n}\n", 1);  }
const char* eemit_module(Module m) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(EmbedPreamble, "\n"), emb_consts(m.items, 0, ""), 1), emb_fns(m.items, 0, ""), 1), EmbedStartup, 1);  }
MacaList eemit_errors(Module m) { return emb_item_errors(m.items, 0, maca_listv(0));  }
const char* emb_consts(MacaList items, long i, const char* acc) { return ((i >= (items.len)) ? ((strcmp(acc, "") == 0) ? "" : maca_cat(acc, "\n")) : (((*(Stmt*)items.data[i]).kind != SBind) ? emb_consts(items, (i + 1), acc) : emb_consts(items, (i + 1), maca_cat(acc, emb_const((*(Stmt*)items.data[i]))))));  }
const char* emb_const(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("static const uint32_t ", s.name), " = ", 1), emb_expr(s.value), 1), ";\n", 1);  }
const char* emb_fns(MacaList items, long i, const char* acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind != SFn) ? emb_fns(items, (i + 1), acc) : emb_fns(items, (i + 1), maca_cat_own(maca_cat(acc, emb_fn((*(Stmt*)items.data[i]))), "\n", 1))));  }
const char* emb_fn(Stmt s) { const char* ret = ((strcmp(s.ret, "") == 0) ? "void" : "uint32_t"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ret), " ", 1), s.name, 1), "(", 1), emb_params(s.params, 0, ""), 1), ") {\n", 1), emb_block(s.body, (strcmp(s.ret, "") != 0), 1), 1), "}\n", 1);  }
const char* emb_params(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? ((strcmp(acc, "") == 0) ? "void" : acc) : ((strcmp(acc, "") == 0) ? emb_params(ps, (i + 1), maca_cat("uint32_t ", (*(Expr*)ps.data[i]).text)) : emb_params(ps, (i + 1), maca_cat_own(acc, maca_cat(", uint32_t ", (*(Expr*)ps.data[i]).text), 2))));  }
const char* emb_block(MacaList body, long wants_value, long ind) { return emb_stmts(body, 0, wants_value, ind, "");  }
const char* emb_stmts(MacaList body, long i, long wants_value, long ind, const char* acc) { return ((i >= (body.len)) ? acc : emb_stmts(body, (i + 1), wants_value, ind, maca_cat(acc, emb_stmt((*(Stmt*)body.data[i]), ((i + 1) == (body.len)), wants_value, ind))));  }
const char* emb_stmt(Stmt s, long last, long wants_value, long ind) { const char* pad = emb_pad(ind, ""); return ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "uint32_t ", 1), s.name, 1), " = ", 1), emb_expr(s.value), 1), ";\n", 1) : ((s.kind == SSet) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), s.name, 1), " = ", 1), emb_expr(s.value), 1), ";\n", 1) : ((s.value.kind == EFor) ? emb_for(s.value, ind) : ((s.value.kind == EWhile) ? emb_while(s.value, ind) : ((s.value.kind == EJump) ? emb_jump(s.value, pad) : ((s.value.kind == EIf) ? emb_if(s.value, ind) : ((last && wants_value) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "return ", 1), emb_expr(s.value), 1), ";\n", 1) : (emb_pure(s.value) ? "" : maca_cat_own(maca_cat_own(maca_cat("", pad), emb_expr(s.value), 1), ";\n", 1)))))))));  }
const char* emb_while(Expr e, long ind) { const char* pad = emb_pad(ind, ""); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "while (", 1), emb_expr((*(Expr*)e.children.data[0])), 1), ") {\n", 1), emb_block(e.stmts, 0, (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3);  }
const char* emb_for(Expr e, long ind) { const char* pad = emb_pad(ind, ""); Expr over = (*(Expr*)e.children.data[0]); return (((over.kind == ECall) && (strcmp(over.text, "forever") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "while (1) {\n", 1), emb_block(e.stmts, 0, (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "for (uint32_t ", 1), e.text, 1), " = 0; ", 1), e.text, 1), " < (", 1), emb_expr(over), 1), ");", 1), maca_cat_own(maca_cat(" ", e.text), "++) {\n", 1), 3), emb_block(e.stmts, 0, (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3));  }
const char* emb_if(Expr e, long ind) { const char* pad = emb_pad(ind, ""); const char* head = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "if (", 1), emb_expr((*(Expr*)e.children.data[0])), 1), ") {\n", 1), emb_branch((*(Expr*)e.children.data[1]), (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}", 1), 3); Expr els = (*(Expr*)e.children.data[2]); return (emb_no_else(els) ? maca_cat(head, "\n") : maca_cat_own(maca_cat_own(maca_cat(head, " else {\n"), emb_branch(els, (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3));  }
long emb_no_else(Expr e) { return ((e.kind == EIdent) && (strcmp(e.text, "?") == 0));  }
const char* emb_branch(Expr e, long ind) { return ((e.kind == EBlock) ? emb_block(maca_list_cat(e.stmts, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr((*(Expr*)e.children.data[0])) }))), 0, ind) : emb_block(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(e) })), 0, ind));  }
const char* emb_jump(Expr e, const char* pad) { return ((strcmp(e.text, "return") != 0) ? maca_cat_own(maca_cat_own(maca_cat("", pad), e.text, 1), ";\n", 1) : (((e.children.len) == 0) ? maca_cat_own(maca_cat("", pad), "return;\n", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "return ", 1), emb_expr((*(Expr*)e.children.data[0])), 1), ";\n", 1)));  }
const char* emb_pad(long n, const char* acc) { return ((n <= 0) ? acc : emb_pad((n - 1), maca_cat(acc, "    ")));  }
long emb_pure(Expr e) { return (((e.kind == EInt) || (e.kind == EBool)) || (e.kind == EIdent));  }
const char* emb_expr(Expr e) { return (e.kind == EInt ? emb_int(e.ival) : (e.kind == EBool ? emb_bool(e) : (e.kind == EIdent ? e.text : (e.kind == EUnary ? emb_unary(e) : (e.kind == EBinary ? emb_binary(e) : (e.kind == ETernary ? emb_ternary(e) : (e.kind == ECall ? emb_call(e) : (e.kind == EField ? maca_cat_own(maca_cat_own(maca_cat("", emb_expr((*(Expr*)e.children.data[0]))), ".", 1), e.text, 1) : "0u"))))))));  }
const char* emb_int(long n) { return ((n >= 0) ? maca_cat_own(maca_cat_own("", maca_int_to_str(n), 2), "u", 1) : maca_int_to_str(n));  }
const char* emb_bool(Expr e) { return ((strcmp(e.text, "true") == 0) ? "1u" : "0u");  }
const char* emb_ternary(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emb_expr((*(Expr*)e.children.data[0]))), " ? ", 1), emb_expr((*(Expr*)e.children.data[1])), 1), maca_cat_own(maca_cat(" : ", emb_expr((*(Expr*)e.children.data[2]))), ")", 1), 3);  }
const char* emb_unary(Expr e) { return (emb_prefix(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), "(", 1), emb_expr((*(Expr*)e.children.data[0])), 1), ")", 1) : "0u");  }
const char* emb_binary(Expr e) { const char* l = emb_expr((*(Expr*)e.children.data[0])); const char* r = emb_expr((*(Expr*)e.children.data[1])); return ((strcmp(e.text, "=") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " = ", 1), r, 1), ")", 1) : (emb_infix(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", l), " ", 1), e.text, 1), " ", 1), r, 1), ")", 1) : "0u"));  }
long emb_prefix(const char* op) { return ((strcmp(op, "-") == 0) || (strcmp(op, "!") == 0));  }
long emb_infix(const char* op) { return (((((((((((((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "*") == 0)) || (strcmp(op, "/") == 0)) || (strcmp(op, "%") == 0)) || (strcmp(op, "==") == 0)) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">=") == 0)) || (strcmp(op, "&&") == 0)) || (strcmp(op, "||") == 0)) || (strcmp(op, "<<") == 0)) || (strcmp(op, ">>") == 0));  }
const char* emb_call(Expr e) { const char* a = emb_arg(e, 0); const char* b = emb_arg(e, 1); return ((strcmp(e.text, "mmio_write") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emb_reg(a)), " = (uint32_t)(", 1), b, 1), "))", 1) : ((strcmp(e.text, "mmio_read") == 0) ? emb_reg(a) : ((strcmp(e.text, "set_bits") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emb_reg(a)), " |= (uint32_t)(", 1), b, 1), "))", 1) : ((strcmp(e.text, "clear_bits") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emb_reg(a)), " &= ~(uint32_t)(", 1), b, 1), "))", 1) : ((strcmp(e.text, "toggle_bits") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", emb_reg(a)), " ^= (uint32_t)(", 1), b, 1), "))", 1) : ((strcmp(e.text, "bit") == 0) ? maca_cat_own(maca_cat("(1u << (", a), "))", 1) : ((strcmp(e.text, "shl") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", a), ") << (", 1), b, 1), "))", 1) : ((strcmp(e.text, "shr") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", a), ") >> (", 1), b, 1), "))", 1) : ((strcmp(e.text, "bit_or") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", a), ") | (", 1), b, 1), "))", 1) : ((strcmp(e.text, "bit_and") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((", a), ") & (", 1), b, 1), "))", 1) : ((strcmp(e.text, "delay") == 0) ? maca_cat_own(maca_cat("maca_delay(", a), ")", 1) : ((strcmp(e.text, "nop") == 0) ? "__asm__ volatile(\"nop\")" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), "(", 1), emb_args(e.children, 0, ""), 1), ")", 1)))))))))))));  }
const char* emb_reg(const char* addr) { return maca_cat_own(maca_cat("(*(volatile uint32_t *)(uintptr_t)(", addr), "))", 1);  }
const char* emb_arg(Expr e, long i) { return ((i >= (e.children.len)) ? "0u" : emb_expr((*(Expr*)e.children.data[i])));  }
const char* emb_args(MacaList xs, long i, const char* acc) { return ((i >= (xs.len)) ? acc : ((i == 0) ? emb_args(xs, (i + 1), emb_expr((*(Expr*)xs.data[i]))) : emb_args(xs, (i + 1), maca_cat_own(maca_cat(acc, ", "), emb_expr((*(Expr*)xs.data[i])), 1))));  }
MacaList emb_item_errors(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : emb_item_errors(items, (i + 1), emb_item_error((*(Stmt*)items.data[i]), acc)));  }
MacaList emb_item_error(Stmt s, MacaList acc) { return ((s.kind == SSum) ? maca_list_cat(acc, maca_listv(1, (long)(emb_sum_refusal(s.name)))) : ((s.kind == SRecord) ? maca_list_cat(acc, maca_listv(1, (long)(emb_record_refusal(s.name)))) : ((s.kind == SFn) ? emb_stmt_errors(s.body, 0, acc) : ((s.kind == SBind) ? emb_value_errors(s.value, acc) : acc))));  }
const char* emb_sum_refusal(const char* name) { return maca_cat_own(maca_cat_own(maca_cat("`", name), "` is a sum type; the embedded target has no tagged values;", 1), " use integer constants", 1);  }
const char* emb_record_refusal(const char* name) { return maca_cat_own(maca_cat_own(maca_cat("`", name), "` is a record type; the embedded target has no structs;", 1), " use separate values", 1);  }
MacaList emb_stmt_errors(MacaList body, long i, MacaList acc) { return ((i >= (body.len)) ? acc : emb_stmt_errors(body, (i + 1), emb_one_errors((*(Stmt*)body.data[i]), acc)));  }
MacaList emb_one_errors(Stmt s, MacaList acc) { return (((s.kind == SBind) || (s.kind == SSet)) ? emb_value_errors(s.value, acc) : (((s.value.kind == EWhile) || (s.value.kind == EFor)) ? emb_stmt_errors(s.value.stmts, 0, emb_value_errors((*(Expr*)s.value.children.data[0]), acc)) : ((s.value.kind == EIf) ? emb_if_errors(s.value, acc) : ((s.value.kind == EJump) ? emb_jump_errors(s.value, acc) : emb_value_errors(s.value, acc)))));  }
MacaList emb_if_errors(Expr e, MacaList acc) { return emb_branch_errors((*(Expr*)e.children.data[2]), emb_branch_errors((*(Expr*)e.children.data[1]), emb_value_errors((*(Expr*)e.children.data[0]), acc)));  }
MacaList emb_branch_errors(Expr e, MacaList acc) { return (emb_no_else(e) ? acc : ((e.kind == EBlock) ? emb_one_errors(s_expr((*(Expr*)e.children.data[0])), emb_stmt_errors(e.stmts, 0, acc)) : emb_one_errors(s_expr(e), acc)));  }
MacaList emb_jump_errors(Expr e, MacaList acc) { return (((e.children.len) == 0) ? acc : emb_value_errors((*(Expr*)e.children.data[0]), acc));  }
MacaList emb_value_errors(Expr e, MacaList acc) { const char* refused = emb_refusal(e); return ((strcmp(refused, "") != 0) ? maca_list_cat(acc, maca_listv(1, (long)(refused))) : emb_child_errors(e.children, 0, acc));  }
MacaList emb_child_errors(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : emb_child_errors(xs, (i + 1), emb_value_errors((*(Expr*)xs.data[i]), acc)));  }
const char* emb_refusal(Expr e) { const char* named = emb_named(e); return (((e.kind == EBinary) && (strcmp(e.text, "++") == 0)) ? maca_cat("`++` needs an allocator; the embedded target is freestanding,", " with no heap") : ((strcmp(named, "") == 0) ? "" : maca_cat_own(maca_cat("", named), " is not lowered on the embedded target", 1)));  }
const char* emb_named(Expr e) { return (emb_lowered(e) ? "" : ((e.kind == EMatch) ? "`match`" : ((e.kind == EFloat) ? "a float literal (the target is integer-only)" : ((e.kind == EStr) ? "a string (the target has no allocator)" : ((e.kind == EList) ? "a list (the target has no allocator)" : ((e.kind == ERecord) ? "a record or sum value" : ((e.kind == EWith) ? "a record update" : ((e.kind == ELambda) ? "a closure" : ((e.kind == EIf) ? "`if` in value position" : ((e.kind == EBlock) ? "a block in value position" : ((e.kind == EJump) ? maca_cat_own(maca_cat("`", e.text), "` in value position", 1) : ((e.kind == EMethod) ? "a method call (the target has no runtime)" : (((e.kind == EWhile) || (e.kind == EFor)) ? "a loop in value position" : ((e.kind == EUnary) ? emb_effect_named(e.text) : ((e.kind == EBinary) ? maca_cat_own(maca_cat("`", e.text), "`", 1) : "this construct")))))))))))))));  }
const char* emb_effect_named(const char* op) { return (((strcmp(op, "spawn") == 0) || (strcmp(op, "await") == 0)) ? "`await`/`spawn` (there is no scheduler)" : "the error operators (`?`, `fail`)");  }
long emb_lowered(Expr e) { return (((((((e.kind == EInt) || (e.kind == EBool)) || (e.kind == EIdent)) || (e.kind == ECall)) || (e.kind == EField)) || (e.kind == ETernary)) ? 1 : ((e.kind == EUnary) ? emb_prefix(e.text) : ((e.kind == EBinary) ? ((strcmp(e.text, "=") == 0) || emb_infix(e.text)) : 0)));  }
const char* jv_id(const char* name) { return ((maca_str_index_of(JavaReserved, maca_cat_own(maca_cat(" ", name), " ", 1)) >= 0) ? maca_cat_own(maca_cat("", name), "_mc", 1) : name);  }
const char* jvmemit_module(Module m, const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(jv_types(m.items, 0, ""), maca_cat_own(maca_cat("public final class ", name), " {\n", 1), 2), jvm_helpers(), 1), jv_members(m.items, jv_fn_names(m.items, 0, maca_listv(0)), 0, ""), 1), "}\n", 1);  }
const char* jvm_helpers() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    static <T> java.util.List<T> _cat(java.util.List<T> a,", " java.util.List<T> b) { var o = new java.util.ArrayList<T>(a);"), " o.addAll(b); return java.util.List.copyOf(o); }\n", 1), "    static <T> java.util.List<T> _push(java.util.List<T> a, T v)", 1), " { var o = new java.util.ArrayList<T>(a); o.add(v);", 1), " return java.util.List.copyOf(o); }\n", 1), "    static String _at(String s, long i)", 1), " { return s.substring((int) i, (int) i + 1); }\n", 1), "    static boolean _alpha(String s)", 1), " { return !s.isEmpty() && Character.isLetter(s.charAt(0)); }\n", 1), "    static boolean _digit(String s)", 1), " { return !s.isEmpty() && Character.isDigit(s.charAt(0)); }\n", 1), "    static boolean _space(String s)", 1), " { return !s.isEmpty() && Character.isWhitespace(s.charAt(0)); }\n\n", 1);  }
const char* jv_types(MacaList items, long i, const char* acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind == SRecord) ? jv_types(items, (i + 1), maca_cat(acc, jv_class((*(Stmt*)items.data[i])))) : (((*(Stmt*)items.data[i]).kind == SSum) ? jv_types(items, (i + 1), maca_cat(acc, jv_enum((*(Stmt*)items.data[i])))) : jv_types(items, (i + 1), acc))));  }
const char* jv_enum(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("enum ", s.name), " { ", 1), maca_list_join(({ MacaList _m = s.params; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(jv_variant_name((*(Expr*)_m.data[_i]))); _r; }), ", "), 3), " }\n\n", 1);  }
const char* jv_variant_name(Expr v) { return v.text;  }
const char* jv_class(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("final class ", s.name), " {\n", 1), jv_fields(s.params, 0, ""), 1), jv_setters(s.name, s.params, 0, ""), 1), "}\n\n", 1);  }
const char* jv_fields(MacaList fs, long i, const char* acc) { return ((i >= (fs.len)) ? acc : jv_fields(fs, (i + 1), maca_cat_own(maca_cat_own(acc, maca_cat("    ", jv_type((*(Expr*)fs.data[i]).ty)), 2), maca_cat_own(maca_cat(" ", jv_id((*(Expr*)fs.data[i]).text)), ";\n", 1), 3)));  }
const char* jv_setters(const char* owner, MacaList fs, long i, const char* acc) { return ((i >= (fs.len)) ? acc : ({ const char* name = jv_id((*(Expr*)fs.data[i]).text); const char* one = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    ", owner), " ", 1), name, 1), "(", 1), jv_type((*(Expr*)fs.data[i]).ty), 1), " v)", 1), maca_cat_own(maca_cat(" { this.", name), " = v; return this; }\n", 1), 3); jv_setters(owner, fs, (i + 1), maca_cat(acc, one)); }));  }
const char* jv_members(MacaList items, MacaList fns, long i, const char* acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind == SBind) ? jv_members(items, fns, (i + 1), maca_cat(acc, jv_bind((*(Stmt*)items.data[i]), items, fns))) : ((((*(Stmt*)items.data[i]).kind != SFn) || (((*(Stmt*)items.data[i]).body.len) == 0)) ? jv_members(items, fns, (i + 1), acc) : ((strcmp((*(Stmt*)items.data[i]).name, "main") == 0) ? jv_members(items, fns, (i + 1), maca_cat(acc, jv_main((*(Stmt*)items.data[i])))) : jv_members(items, fns, (i + 1), maca_cat(acc, jv_method((*(Stmt*)items.data[i]))))))));  }
const char* jv_bind(Stmt s, MacaList items, MacaList fns) { return (jv_is_impl(s, fns) ? jv_impl(s, items) : jv_const(s));  }
long jv_is_impl(Stmt s, MacaList fns) { return (((jv_user_type(s.ret) && (s.value.kind == ERecord)) && ((s.value.children.len) > 0)) && jv_all_fns(s.value.children, fns, 0));  }
long jv_all_fns(MacaList fs, MacaList fns, long i) { return ((i >= (fs.len)) ? 1 : ((((*(Expr*)fs.data[i]).children.len) < 2) ? 0 : ((((*(Expr*)(*(Expr*)fs.data[i]).children.data[1]).kind == EIdent) && (maca_list_index_of_str(fns, (*(Expr*)(*(Expr*)fs.data[i]).children.data[1]).text) >= 0)) ? jv_all_fns(fs, fns, (i + 1)) : 0)));  }
const char* jv_impl(Stmt s, MacaList items) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    public static class ", jv_id(s.name)), " implements ", 1), s.ret, 1), " {\n", 1), jv_impl_methods(s.value.children, items, 0, ""), 1), "    }\n\n", 1);  }
const char* jv_impl_methods(MacaList fs, MacaList items, long i, const char* acc) { return ((i >= (fs.len)) ? acc : ({ Stmt made = jv_named_fn(items, 0, (*(Expr*)(*(Expr*)fs.data[i]).children.data[1]).text); const char* one = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("        public void ", jv_id((*(Expr*)(*(Expr*)fs.data[i]).children.data[0]).text)), maca_cat_own(maca_cat("(", jv_params(made.params, 0, "")), ") {\n            ", 1), 3), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jv_id(made.name)), "(", 1), jv_names(made.params, 0, ""), 1), ");\n", 1), 3), "        }\n", 1); jv_impl_methods(fs, items, (i + 1), maca_cat(acc, one)); }));  }
Stmt jv_named_fn(MacaList items, long i, const char* name) { return ((i >= (items.len)) ? s_fn(name, "", maca_listv(0), maca_listv(0)) : ((((*(Stmt*)items.data[i]).kind == SFn) && (strcmp((*(Stmt*)items.data[i]).name, name) == 0)) ? (*(Stmt*)items.data[i]) : jv_named_fn(items, (i + 1), name)));  }
const char* jv_names(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? acc : ({ const char* one = jv_id((*(Expr*)ps.data[i]).text); jv_names(ps, (i + 1), ((strcmp(acc, "") == 0) ? one : maca_cat_own(maca_cat(acc, ", "), one, 1))); }));  }
const char* jv_const(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    static final ", jv_const_type(s)), " ", 1), jv_id(s.name), 1), maca_cat_own(maca_cat(" = ", jv_expr(s.value)), ";\n\n", 1), 3);  }
const char* jv_const_type(Stmt s) { return ((strcmp(s.ret, "") != 0) ? jv_type(s.ret) : jv_type(s.value.ty));  }
const char* jv_method(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    static ", jv_ret(s)), " ", 1), jv_id(s.name), 1), "(", 1), jv_params(s.params, 0, ""), 1), ")", 1), " {\n", 1), jv_body(s.body, s.ret, 2), 1), "    }\n\n", 1);  }
const char* jv_ret(Stmt s) { return ((strcmp(s.ret, "") == 0) ? "void" : jv_type(s.ret));  }
const char* jv_params(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? acc : ({ const char* one = maca_cat_own(maca_cat_own(maca_cat("", jv_type((*(Expr*)ps.data[i]).ty)), " ", 1), jv_id((*(Expr*)ps.data[i]).text), 1); jv_params(ps, (i + 1), ((strcmp(acc, "") == 0) ? one : maca_cat_own(maca_cat(acc, ", "), one, 1))); }));  }
const char* jv_main(Stmt s) { return maca_cat_own(maca_cat_own(maca_cat("    public static void main(String[] _argv) {\n", jv_argv(s.params)), jv_body(s.body, "", 2), 1), "    }\n\n", 1);  }
const char* jv_argv(MacaList ps) { return (((ps.len) == 0) ? "" : maca_cat_own(maca_cat("        java.util.List<String> ", jv_id((*(Expr*)ps.data[0]).text)), " = java.util.List.of(_argv);\n", 1));  }
const char* jv_body(MacaList body, const char* ret, long ind) { return jv_stmts(body, 0, ret, ind, "");  }
const char* jv_stmts(MacaList body, long i, const char* ret, long ind, const char* acc) { return ((i >= (body.len)) ? acc : jv_stmts(body, (i + 1), ret, ind, maca_cat(acc, jv_stmt((*(Stmt*)body.data[i]), ((i + 1) == (body.len)), ret, ind))));  }
const char* jv_stmt(Stmt s, long last, const char* ret, long ind) { const char* pad = jv_pad(ind, ""); long wants = (last && (strcmp(ret, "") != 0)); return ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), jv_local(s), 1), " ", 1), jv_id(s.name), 1), " = ", 1), jv_expr(s.value), 1), ";\n", 1) : ((s.kind == SSet) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), jv_id(s.name), 1), " = ", 1), jv_expr(s.value), 1), ";\n", 1) : ((s.value.kind == EWhile) ? jv_while(s.value, ind) : ((s.value.kind == EFor) ? jv_for(s.value, ind) : ((s.value.kind == EJump) ? jv_jump(s.value, pad) : (((s.value.kind == EIf) && jv_no_else((*(Expr*)s.value.children.data[2]))) ? maca_cat(jv_if_stmt(s.value, ind), jv_fallback(wants, ret, pad)) : (((s.value.kind == EIf) && (!wants)) ? jv_if_stmt(s.value, ind) : (((s.value.kind == EMatch) && (!wants)) ? jv_match_stmt(s.value, ind) : (wants ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "return ", 1), jv_expr(s.value), 1), ";\n", 1) : (jv_pure(s.value) ? "" : maca_cat_own(maca_cat_own(maca_cat("", pad), jv_expr(s.value), 1), ";\n", 1)))))))))));  }
const char* jv_fallback(long wants, const char* ret, const char* pad) { return (wants ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "return ", 1), jv_zero(ret), 1), ";\n", 1) : "");  }
const char* jv_local(Stmt s) { return ((strcmp(s.ret, "") == 0) ? "var" : jv_type(s.ret));  }
const char* jv_while(Expr e, long ind) { const char* pad = jv_pad(ind, ""); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "while (", 1), jv_expr((*(Expr*)e.children.data[0])), 1), ") {\n", 1), jv_body(e.stmts, "", (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3);  }
const char* jv_for(Expr e, long ind) { const char* pad = jv_pad(ind, ""); Expr over = (*(Expr*)e.children.data[0]); const char* name = jv_id(e.text); return (((over.kind == EBinary) && (strcmp(over.text, "..") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "for (long ", 1), name, 1), " = ", 1), jv_expr((*(Expr*)over.children.data[0])), 1), ";", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(" ", name), " < ", 1), jv_expr((*(Expr*)over.children.data[1])), 1), "; ", 1), name, 1), "++) {\n", 1), 3), jv_body(e.stmts, "", (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "for (var ", 1), name, 1), " : ", 1), jv_expr(over), 1), ") {\n", 1), jv_body(e.stmts, "", (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3));  }
const char* jv_jump(Expr e, const char* pad) { return ((strcmp(e.text, "return") != 0) ? maca_cat_own(maca_cat_own(maca_cat("", pad), e.text, 1), ";\n", 1) : (((e.children.len) == 0) ? maca_cat_own(maca_cat("", pad), "return;\n", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "return ", 1), jv_expr((*(Expr*)e.children.data[0])), 1), ";\n", 1)));  }
const char* jv_if_stmt(Expr e, long ind) { const char* pad = jv_pad(ind, ""); const char* head = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", pad), "if (", 1), jv_expr((*(Expr*)e.children.data[0])), 1), ") {\n", 1), jv_branch((*(Expr*)e.children.data[1]), (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}", 1), 3); Expr els = (*(Expr*)e.children.data[2]); return (jv_no_else(els) ? maca_cat(head, "\n") : maca_cat_own(maca_cat_own(maca_cat(head, " else {\n"), jv_branch(els, (ind + 1)), 1), maca_cat_own(maca_cat("", pad), "}\n", 1), 3));  }
long jv_no_else(Expr e) { return ((e.kind == EIdent) && (strcmp(e.text, "?") == 0));  }
const char* jv_branch(Expr e, long ind) { return ((e.kind == EBlock) ? jv_body(maca_list_cat(e.stmts, maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr((*(Expr*)e.children.data[0])) }))), "", ind) : jv_body(maca_listv(1, maca_box(sizeof(Stmt), (Stmt[]){ s_expr(e) })), "", ind));  }
const char* jv_pad(long n, const char* acc) { return ((n <= 0) ? acc : jv_pad((n - 1), maca_cat(acc, "    ")));  }
long jv_pure(Expr e) { return (((((e.kind == EInt) || (e.kind == EFloat)) || (e.kind == EBool)) || (e.kind == EStr)) || (e.kind == EIdent));  }
const char* jv_zero(const char* ret) { const char* t = jv_type(ret); return ((strcmp(t, "long") == 0) ? "0L" : ((strcmp(t, "double") == 0) ? "0.0" : ((strcmp(t, "boolean") == 0) ? "false" : ((strcmp(t, "String") == 0) ? "\"\"" : "null"))));  }
const char* jv_expr(Expr e) { return (e.kind == EInt ? maca_cat_own(maca_cat_own("", maca_int_to_str(e.ival), 2), "L", 1) : (e.kind == EFloat ? e.text : (e.kind == EStr ? jv_string(e.text) : (e.kind == EBool ? e.text : (e.kind == EIdent ? jv_name(e) : (e.kind == ECall ? jv_call(e) : (e.kind == EBinary ? jv_binary(e) : (e.kind == EUnary ? jv_unary(e) : (e.kind == ETernary ? jv_ternary(e) : (e.kind == EIf ? jv_ternary(e) : (e.kind == EField ? maca_cat_own(maca_cat_own(maca_cat("", jv_expr((*(Expr*)e.children.data[0]))), ".", 1), jv_id(e.text), 1) : (e.kind == ERecord ? jv_new(e) : (e.kind == EList ? maca_cat_own(maca_cat("java.util.List.of(", jv_args(e.children, 0, "")), ")", 1) : (e.kind == EMethod ? jv_method_call(e) : (e.kind == EMatch ? jv_match(e) : (e.kind == EJump ? jv_jump_value(e) : "null"))))))))))))))));  }
const char* jv_string(const char* s) { return maca_cat_own(maca_cat("\"", s), "\"", 1);  }
const char* jv_name(Expr e) { return (((jv_upper(e.text) && jv_user_type(e.ty)) && (strcmp(e.ty, e.text) != 0)) ? maca_cat_own(maca_cat_own(maca_cat("", e.ty), ".", 1), e.text, 1) : jv_id(e.text));  }
long jv_upper(const char* w) { return (((((int)strlen(w)) > 0) && (isalpha((unsigned char)(maca_str_at(w, 0))[0]) != 0)) && (strcmp(maca_upper(maca_str_at(w, 0)), maca_str_at(w, 0)) == 0));  }
long jv_user_type(const char* ty) { return ((strcmp(ty, "") != 0) && (strcmp(jv_type(ty), ty) == 0));  }
const char* jv_jump_value(Expr e) { return (((e.children.len) == 0) ? e.text : maca_cat_own(maca_cat_own(maca_cat("", e.text), " ", 1), jv_expr((*(Expr*)e.children.data[0])), 1));  }
const char* jv_ternary(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", jv_expr((*(Expr*)e.children.data[0]))), " ? ", 1), jv_expr((*(Expr*)e.children.data[1])), 1), maca_cat_own(maca_cat(" : ", jv_else((*(Expr*)e.children.data[2]))), ")", 1), 3);  }
const char* jv_else(Expr e) { return (jv_no_else(e) ? "null" : jv_expr(e));  }
const char* jv_unary(Expr e) { return (((strcmp(e.text, "-") == 0) || (strcmp(e.text, "!") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("(", e.text), jv_expr((*(Expr*)e.children.data[0])), 1), ")", 1) : "null");  }
const char* jv_binary(Expr e) { Expr l = (*(Expr*)e.children.data[0]); Expr r = (*(Expr*)e.children.data[1]); const char* lhs = jv_expr(l); const char* rhs = jv_expr(r); return ((strcmp(e.text, "..") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("java.util.stream.LongStream.range(", lhs), ", ", 1), rhs, 1), ").boxed().toList()", 1) : (((strcmp(e.text, "++") == 0) && (jv_listy(l.ty) || jv_listy(r.ty))) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_cat(", lhs), ", ", 1), rhs, 1), ")", 1) : ((strcmp(e.text, "++") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", lhs), " + ", 1), rhs, 1), ")", 1) : (((strcmp(e.text, "==") == 0) && jv_by_value(l, r)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("java.util.Objects.equals(", lhs), ", ", 1), rhs, 1), ")", 1) : (((strcmp(e.text, "!=") == 0) && jv_by_value(l, r)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("!java.util.Objects.equals(", lhs), ", ", 1), rhs, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("(", lhs), " ", 1), e.text, 1), " ", 1), rhs, 1), ")", 1))))));  }
long jv_by_value(Expr l, Expr r) { return ((((strcmp(l.ty, "str") == 0) || (strcmp(r.ty, "str") == 0)) || jv_listy(l.ty)) || jv_listy(r.ty));  }
long jv_listy(const char* ty) { return maca_ends_with(ty, "[]");  }
const char* jv_new(Expr e) { return ((strcmp(e.text, "") == 0) ? "null" : maca_cat_own(maca_cat_own(maca_cat("new ", e.text), "()", 1), jv_writes(e.children, 0, ""), 1));  }
const char* jv_writes(MacaList fs, long i, const char* acc) { return ((i >= (fs.len)) ? acc : ({ Expr f = (*(Expr*)fs.data[i]); const char* one = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(".", jv_id((*(Expr*)f.children.data[0]).text)), "(", 1), jv_expr((*(Expr*)f.children.data[1])), 1), ")", 1); jv_writes(fs, (i + 1), maca_cat(acc, one)); }));  }
const char* jv_args(MacaList xs, long i, const char* acc) { return ((i >= (xs.len)) ? acc : ((i == 0) ? jv_args(xs, (i + 1), jv_expr((*(Expr*)xs.data[i]))) : jv_args(xs, (i + 1), maca_cat_own(maca_cat(acc, ", "), jv_expr((*(Expr*)xs.data[i])), 1))));  }
const char* jv_arg(Expr e, long i) { return ((i >= (e.children.len)) ? "null" : jv_expr((*(Expr*)e.children.data[i])));  }
const char* jv_arg_ty(Expr e) { return (((e.children.len) == 0) ? "" : (*(Expr*)e.children.data[0]).ty);  }
const char* jv_call(Expr e) { const char* a = jv_args(e.children, 0, ""); return ((((strcmp(e.text, "info") == 0) || (strcmp(e.text, "err") == 0)) || (strcmp(e.text, "warn") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("System.", jv_stream(e.text)), ".println(", 1), a, 1), ")", 1) : ((strcmp(e.text, "print") == 0) ? maca_cat_own(maca_cat("System.out.print(", a), ")", 1) : ((strcmp(e.text, "str") == 0) ? maca_cat_own(maca_cat("String.valueOf(", a), ")", 1) : ((strcmp(e.text, "int") == 0) ? jv_to_int(e, a) : ((strcmp(e.text, "float") == 0) ? maca_cat_own(maca_cat("(double)(", a), ")", 1) : ((strcmp(e.text, "len") == 0) ? jv_size(a, jv_arg_ty(e)) : ((strcmp(e.text, "abs") == 0) ? maca_cat_own(maca_cat("Math.abs(", a), ")", 1) : (((strcmp(e.text, "min") == 0) || (strcmp(e.text, "max") == 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Math.", e.text), "(", 1), jv_arg(e, 0), 1), ", ", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(e.text, "pow") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Math.pow(", jv_arg(e, 0)), ", ", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(e.text, "log") == 0) ? maca_cat_own(maca_cat("Math.log(", a), ")", 1) : (jv_math_call(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("Math.", e.text), "(", 1), a, 1), ")", 1) : (jv_upper(e.text) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("new ", e.text), "(", 1), a, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jv_id(e.text)), "(", 1), a, 1), ")", 1)))))))))))));  }
const char* jv_stream(const char* name) { return ((strcmp(name, "info") == 0) ? "out" : "err");  }
long jv_math_call(const char* name) { return ((((((((strcmp(name, "sqrt") == 0) || (strcmp(name, "floor") == 0)) || (strcmp(name, "ceil") == 0)) || (strcmp(name, "round") == 0)) || (strcmp(name, "sin") == 0)) || (strcmp(name, "cos") == 0)) || (strcmp(name, "tan") == 0)) || (strcmp(name, "exp") == 0));  }
const char* jv_to_int(Expr e, const char* a) { return ((strcmp(jv_arg_ty(e), "str") == 0) ? maca_cat_own(maca_cat("Long.parseLong((", a), ").trim())", 1) : maca_cat_own(maca_cat("(long)(", a), ")", 1));  }
const char* jv_size(const char* recv, const char* ty) { return (jv_listy(ty) ? maca_cat_own(maca_cat("((long) ", recv), ".size())", 1) : maca_cat_own(maca_cat("((long) ", recv), ".length())", 1));  }
const char* jv_method_call(Expr e) { const char* recv = jv_expr((*(Expr*)e.children.data[0])); const char* ty = (*(Expr*)e.children.data[0]).ty; return (((strcmp(e.text, "length") == 0) || (strcmp(e.text, "count") == 0)) ? jv_size(recv, ty) : ((strcmp(e.text, "push") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_push(", recv), ", ", 1), jv_arg(e, 1), 1), ")", 1) : ((((strcmp(e.text, "get") == 0) || (strcmp(e.text, "at") == 0)) && (!jv_listy(ty))) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("_at(", recv), ", ", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(e.text, "get") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".get((int)(", 1), jv_arg(e, 1), 1), "))", 1) : (((strcmp(e.text, "slice") == 0) && jv_listy(ty)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".subList((int)(", 1), jv_arg(e, 1), 1), "), (int)(", 1), jv_arg(e, 2), 1), "))", 1) : ((strcmp(e.text, "slice") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".substring((int)(", 1), jv_arg(e, 1), 1), "), (int)(", 1), jv_arg(e, 2), 1), "))", 1) : ((strcmp(e.text, "index_of") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("((long) ", recv), ".indexOf(", 1), jv_arg(e, 1), 1), "))", 1) : ((strcmp(e.text, "chars") == 0) ? maca_cat_own(maca_cat("", recv), ".chars().mapToObj(Character::toString).toList()", 1) : ((strcmp(e.text, "split") == 0) ? maca_cat_own(maca_cat_own(maca_cat("java.util.List.of(", recv), ".split(java.util.regex.Pattern", 1), maca_cat_own(maca_cat(".quote(", jv_arg(e, 1)), "), -1))", 1), 3) : ((strcmp(e.text, "join") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("String.join(", jv_arg(e, 1)), ", ", 1), recv, 1), ")", 1) : ((strcmp(e.text, "upper") == 0) ? maca_cat_own(maca_cat("", recv), ".toUpperCase()", 1) : ((strcmp(e.text, "lower") == 0) ? maca_cat_own(maca_cat("", recv), ".toLowerCase()", 1) : ((strcmp(e.text, "trim") == 0) ? maca_cat_own(maca_cat("", recv), ".trim()", 1) : ((strcmp(e.text, "repeat") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".repeat((int)(", 1), jv_arg(e, 1), 1), "))", 1) : ((strcmp(e.text, "starts_with") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".startsWith(", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(e.text, "ends_with") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".endsWith(", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(e.text, "contains") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".contains(", 1), jv_arg(e, 1), 1), ")", 1) : ((strcmp(jv_char_test(e.text), "") != 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jv_char_test(e.text)), "(", 1), recv, 1), ")", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", recv), ".", 1), jv_id(e.text), 1), "(", 1), jv_args(e.children, 1, ""), 1), ")", 1)))))))))))))))))));  }
const char* jv_char_test(const char* name) { return ((strcmp(name, "is_alpha") == 0) ? "_alpha" : ((strcmp(name, "is_ascii_digit") == 0) ? "_digit" : ((strcmp(name, "is_whitespace") == 0) ? "_space" : "")));  }
const char* jv_scrutinee(Expr e) { Expr on = (*(Expr*)e.children.data[0]); return ((strcmp(on.ty, "int") == 0) ? maca_cat_own(maca_cat("(int)(", jv_expr(on)), ")", 1) : jv_expr(on));  }
const char* jv_match(Expr e) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("switch (", jv_scrutinee(e)), ") { ", 1), jv_arms(e.children, 1), 1), "}", 1);  }
const char* jv_arms(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? "default -> throw new IllegalStateException(\"no match\"); " : ((strcmp((*(Expr*)cs.data[i]).text, "_") == 0) ? maca_cat_own(maca_cat("default -> ", jv_expr((*(Expr*)cs.data[(i + 1)]))), "; ", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("case ", jv_label((*(Expr*)cs.data[i]))), " -> ", 1), jv_expr((*(Expr*)cs.data[(i + 1)])), 1), "; ", 1), jv_arms(cs, (i + 2)), 1)));  }
const char* jv_match_stmt(Expr e, long ind) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", jv_pad(ind, "")), "switch (", 1), jv_scrutinee(e), 1), ") { ", 1), jv_stmt_arms(e.children, 1), 1), "}\n", 1);  }
const char* jv_stmt_arms(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? "default -> {} " : ((strcmp((*(Expr*)cs.data[i]).text, "_") == 0) ? maca_cat_own(maca_cat("default -> ", jv_action((*(Expr*)cs.data[(i + 1)]))), " ", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("case ", jv_label((*(Expr*)cs.data[i]))), " -> ", 1), jv_action((*(Expr*)cs.data[(i + 1)])), 1), " ", 1), jv_stmt_arms(cs, (i + 2)), 1)));  }
const char* jv_action(Expr e) { return (jv_pure(e) ? "{}" : maca_cat_own(maca_cat("", jv_expr(e)), ";", 1));  }
const char* jv_label(Expr p) { return (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("", jv_label((*(Expr*)p.children.data[0]))), ", ", 1), jv_label((*(Expr*)p.children.data[1])), 1) : ((p.kind == EStr) ? jv_string(p.text) : p.text));  }
const char* jv_type(const char* ty) { return (maca_ends_with(ty, "[]") ? maca_cat_own(maca_cat("java.util.List<", jv_boxed(maca_str_slice(ty, 0, (((int)strlen(ty)) - 2)))), ">", 1) : ((strcmp(map_type_key(ty), "") != 0) ? maca_cat_own(maca_cat_own(maca_cat("java.util.Map<", jv_boxed(map_type_key(ty))), ",", 1), maca_cat_own(maca_cat(" ", jv_boxed(map_type_val(ty))), ">", 1), 3) : (((strcmp(ty, "") == 0) || (strcmp(ty, "int") == 0)) ? "long" : ((strcmp(ty, "float") == 0) ? "double" : ((strcmp(ty, "bool") == 0) ? "boolean" : ((strcmp(ty, "str") == 0) ? "String" : ((maca_str_index_of(ty, ") -> ") >= 0) ? "Object" : ty)))))));  }
const char* jv_boxed(const char* ty) { const char* named = jv_type(ty); return ((strcmp(named, "long") == 0) ? "Long" : ((strcmp(named, "double") == 0) ? "Double" : ((strcmp(named, "boolean") == 0) ? "Boolean" : named)));  }
MacaList jvmemit_errors(Module m) { return jv_item_errors(m.items, jv_fn_names(m.items, 0, maca_listv(0)), 0, maca_listv(0));  }
MacaList jv_fn_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : (((*(Stmt*)items.data[i]).kind == SFn) ? jv_fn_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : jv_fn_names(items, (i + 1), acc)));  }
MacaList jv_item_errors(MacaList items, MacaList fns, long i, MacaList acc) { return ((i >= (items.len)) ? acc : jv_item_errors(items, fns, (i + 1), jv_one_item((*(Stmt*)items.data[i]), fns, acc)));  }
MacaList jv_one_item(Stmt s, MacaList fns, MacaList acc) { return (((s.kind == SSum) && jv_carries(s.params, 0)) ? maca_list_cat(acc, maca_listv(1, (long)(jv_payload_refusal(s.name)))) : (((s.kind == SSum) || (s.kind == SRecord)) ? acc : ((s.kind == SFn) ? jv_body_errors(s.body, fns, 0, acc) : (jv_is_impl(s, fns) ? acc : jv_value_errors(s.value, fns, acc)))));  }
long jv_carries(MacaList vs, long i) { return ((i >= (vs.len)) ? 0 : ((((*(Expr*)vs.data[i]).children.len) > 0) ? 1 : jv_carries(vs, (i + 1))));  }
const char* jv_payload_refusal(const char* name) { return maca_cat_own(maca_cat_own(maca_cat("`", name), "` is a sum whose variants carry a payload; a Java enum carries", 1), " none, so the payload has nowhere to live", 1);  }
MacaList jv_body_errors(MacaList body, MacaList fns, long i, MacaList acc) { return ((i >= (body.len)) ? acc : jv_body_errors(body, fns, (i + 1), jv_value_errors((*(Stmt*)body.data[i]).value, fns, acc)));  }
MacaList jv_value_errors(Expr e, MacaList fns, MacaList acc) { const char* bad = jv_refusal(e, fns); return ((strcmp(bad, "") != 0) ? maca_list_cat(acc, maca_listv(1, (long)(bad))) : ((e.kind == EIf) ? jv_if_errors(e, fns, acc) : jv_body_errors(e.stmts, fns, 0, jv_kid_errors(e.children, fns, 0, acc))));  }
MacaList jv_if_errors(Expr e, MacaList fns, MacaList acc) { return jv_branch_errors((*(Expr*)e.children.data[2]), fns, jv_branch_errors((*(Expr*)e.children.data[1]), fns, jv_value_errors((*(Expr*)e.children.data[0]), fns, acc)));  }
MacaList jv_branch_errors(Expr e, MacaList fns, MacaList acc) { return (jv_no_else(e) ? acc : ((e.kind == EBlock) ? jv_value_errors((*(Expr*)e.children.data[0]), fns, jv_body_errors(e.stmts, fns, 0, acc)) : jv_value_errors(e, fns, acc)));  }
MacaList jv_kid_errors(MacaList xs, MacaList fns, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : jv_kid_errors(xs, fns, (i + 1), jv_value_errors((*(Expr*)xs.data[i]), fns, acc)));  }
const char* jv_refusal(Expr e, MacaList fns) { return ((e.kind == ELambda) ? "a lambda has no interface to be on the jvm target; name a function" : (((e.kind == EIdent) && (maca_list_index_of_str(fns, e.text) >= 0)) ? maca_cat_own(maca_cat_own(maca_cat("`", e.text), "` is a function used as a value, and Java has no such value", 1), " without an interface to put it on", 1) : ((e.kind == EWith) ? "a record update (`with`) is not lowered by the jvm target" : ((e.kind == EBlock) ? "a block in value position is not lowered by the jvm target" : ((e.kind == EUnary) ? jv_effect_refusal(e.text) : ((e.kind == EMatch) ? jv_match_refusal(e) : (((e.kind == ERecord) && (strcmp(e.text, "") == 0)) ? "this `{ ... }` names no record, and a Java `new` needs a type" : (((e.kind == EBinary) && (strcmp(e.text, "=") == 0)) ? jv_store_refusal((*(Expr*)e.children.data[0])) : ((e.kind == EMethod) ? jv_method_refusal(e) : (((e.kind == ECall) && (!jv_known_call(e.text, fns))) ? maca_cat_own(maca_cat("`", e.text), "` is a builtin the jvm target does not lower", 1) : ""))))))))));  }
const char* jv_effect_refusal(const char* op) { return (((strcmp(op, "-") == 0) || (strcmp(op, "!") == 0)) ? "" : (((strcmp(op, "spawn") == 0) || (strcmp(op, "await") == 0)) ? maca_cat_own(maca_cat("`", op), "` needs a scheduler, which the jvm target does not emit", 1) : "the error operators (`?`, `fail`) are not lowered by the jvm target"));  }
const char* jv_store_refusal(Expr target) { return ((target.kind == EMethod) ? "a write into a list; a list is immutable on the jvm target" : "");  }
const char* jv_method_refusal(Expr e) { const char* ty = (*(Expr*)e.children.data[0]).ty; return ((strcmp(map_type_key(ty), "") != 0) ? "`Map` is not lowered by the jvm target" : ((((strcmp(e.text, "map") == 0) || (strcmp(e.text, "filter") == 0)) || (strcmp(e.text, "reduce") == 0)) ? maca_cat_own(maca_cat("`", e.text), "` takes a function, which the jvm target does not pass", 1) : ""));  }
const char* jv_match_refusal(Expr e) { Expr on = (*(Expr*)e.children.data[0]); return (((strcmp(on.ty, "bool") == 0) || (strcmp(on.ty, "float") == 0)) ? maca_cat_own(maca_cat_own(maca_cat("a `match` on a ", on.ty), " is not a Java `switch`, which takes an int,", 1), " a string or an enum", 1) : jv_arm_refusal(e.children, 1));  }
const char* jv_arm_refusal(MacaList cs, long i) { return (((i + 1) >= (cs.len)) ? "" : (((*(Expr*)cs.data[i]).kind == EGuard) ? "an arm guarded by `if` is not a `case` label on the jvm target" : ((strcmp(jv_shape_pat((*(Expr*)cs.data[i])), "") != 0) ? maca_cat_own(maca_cat("", jv_shape_pat((*(Expr*)cs.data[i]))), " is not a `case` label on the jvm target", 1) : jv_arm_refusal(cs, (i + 2)))));  }
const char* jv_shape_pat(Expr p) { return (((strcmp(p.text, "[]") == 0) || (strcmp(p.text, "[..]") == 0)) ? "a list pattern" : ((strcmp(p.text, "{}") == 0) ? "a record pattern" : (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? "" : (((p.children.len) > 0) ? maca_cat_own(maca_cat("`", p.text), "(...)`, which binds a payload,", 1) : ""))));  }
long jv_known_call(const char* name, MacaList fns) { return (((maca_list_index_of_str(fns, name) >= 0) || jv_upper(name)) || jv_builtin(name));  }
long jv_builtin(const char* name) { return ((((((((((((((strcmp(name, "info") == 0) || (strcmp(name, "print") == 0)) || (strcmp(name, "err") == 0)) || (strcmp(name, "warn") == 0)) || (strcmp(name, "str") == 0)) || (strcmp(name, "int") == 0)) || (strcmp(name, "float") == 0)) || (strcmp(name, "len") == 0)) || (strcmp(name, "abs") == 0)) || (strcmp(name, "min") == 0)) || (strcmp(name, "max") == 0)) || (strcmp(name, "pow") == 0)) || (strcmp(name, "log") == 0)) || jv_math_call(name));  }
const char* print_module(Module m) { return print_items(m.items, maca_listv(0), 0, 0, "");  }
const char* print_marked(Module m, MacaList marks) { return print_items(m.items, marks, 0, 0, "");  }
const char* print_source(const char* src) { MacaList ts = lex(src); MacaList marks = lex_marked(src); return print_marked(parse_module(ts, 0, maca_listv(0)), in_order(marks, imports_of(src, ts, marks, 0, maca_listv(0)), 0, 0, maca_listv(0)));  }
MacaList imports_of(const char* src, MacaList ts, MacaList marks, long i, MacaList acc) { return (((*(Token*)ts.data[i]).kind == Eof) ? acc : (((*(Token*)ts.data[i]).kind != KwImport) ? imports_of(src, ts, marks, (i + 1), acc) : imports_of(src, ts, marks, import_end(ts, (i + 1)), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Token), (Token[]){ one_import(src, ts, marks, i) }))))));  }
Token one_import(const char* src, MacaList ts, MacaList marks, long i) { long from = (*(Token*)ts.data[i]).pos; long upto = import_text_end(src, ts, marks, import_end(ts, (i + 1)), from); return mk_token(KwImport, maca_trim(maca_str_slice(src, from, upto)), from);  }
long import_text_end(const char* src, MacaList ts, MacaList marks, long shut, long from) { long tail = (((*(Token*)ts.data[shut]).pos > from) ? (*(Token*)ts.data[shut]).pos : ((int)strlen(src))); long m = past(marks, 0, from); return (((m < (marks.len)) && ((*(Token*)marks.data[m]).pos < tail)) ? (*(Token*)marks.data[m]).pos : tail);  }
MacaList in_order(MacaList a, MacaList b, long i, long j, MacaList acc) { return (((i >= (a.len)) && (j >= (b.len))) ? acc : (((j >= (b.len)) || ((i < (a.len)) && ((*(Token*)a.data[i]).pos < (*(Token*)b.data[j]).pos))) ? in_order(a, b, (i + 1), j, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Token), (Token[]){ (*(Token*)a.data[i]) })))) : in_order(a, b, i, (j + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Token), (Token[]){ (*(Token*)b.data[j]) }))))));  }
const char* print_items(MacaList items, MacaList marks, long i, long seen, const char* acc) { return ((i >= (items.len)) ? maca_cat(acc, trailing_marks(marks, seen, "")) : ({ const char* here = maca_cat(acc, marks_before(marks, seen, (*(Stmt*)items.data[i]).pos, "")); const char* gap = ((i == ((items.len) - 1)) ? "\n" : "\n\n"); print_items(items, marks, (i + 1), past(marks, seen, (*(Stmt*)items.data[i]).pos), maca_cat_own(maca_cat(here, print_item((*(Stmt*)items.data[i]))), gap, 1)); }));  }
const char* marks_before(MacaList marks, long from, long upto, const char* acc) { return (((from >= (marks.len)) || ((*(Token*)marks.data[from]).pos >= upto)) ? acc : marks_before(marks, (from + 1), upto, maca_cat(acc, written(marks, from, 1))));  }
const char* trailing_marks(MacaList marks, long from, const char* acc) { return ((from >= (marks.len)) ? acc : trailing_marks(marks, (from + 1), maca_cat(acc, written(marks, from, 0))));  }
const char* written(MacaList marks, long from, long more) { long kept = ((*(Token*)marks.data[from]).kind == KwImport); long runs_on = ((!kept) || (((from + 1) < (marks.len)) && ((*(Token*)marks.data[(from + 1)]).kind == KwImport))); const char* lead = (((kept && (from > 0)) && ((*(Token*)marks.data[(from - 1)]).kind != KwImport)) ? "\n" : ""); return maca_cat_own(maca_cat(lead, (*(Token*)marks.data[from]).text), ((runs_on || (!more)) ? "\n" : "\n\n"), 1);  }
long past(MacaList marks, long from, long upto) { return (((from >= (marks.len)) || ((*(Token*)marks.data[from]).pos >= upto)) ? from : past(marks, (from + 1), upto));  }
const char* print_item(Stmt s) { return ((s.kind == SRecord) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), " = { ", 1), print_fields(s.params, 0, ""), 1), " }", 1) : ((s.kind == SSum) ? maca_cat_own(maca_cat_own(maca_cat("", s.name), " = ", 1), print_variants(s.params, 0, ""), 1) : ((s.kind == SBind) ? maca_cat_own(maca_cat_own(maca_cat("", s.name), " = ", 1), print_expr(s.value, 0), 1) : print_fn(s))));  }
const char* print_fields(MacaList fs, long i, const char* acc) { return ((i >= (fs.len)) ? acc : ((i == ((fs.len) - 1)) ? maca_cat_own(acc, maca_cat_own(maca_cat_own(maca_cat("", (*(Expr*)fs.data[i]).text), ": ", 1), (*(Expr*)fs.data[i]).ty, 1), 2) : print_fields(fs, (i + 1), maca_cat_own(acc, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", (*(Expr*)fs.data[i]).text), ": ", 1), (*(Expr*)fs.data[i]).ty, 1), ", ", 1), 2))));  }
const char* print_variants(MacaList vs, long i, const char* acc) { return ((i >= (vs.len)) ? acc : ((i == ((vs.len) - 1)) ? maca_cat(acc, print_variant((*(Expr*)vs.data[i]))) : print_variants(vs, (i + 1), maca_cat_own(maca_cat(acc, print_variant((*(Expr*)vs.data[i]))), " | ", 1))));  }
const char* print_variant(Expr v) { return (((v.children.len) == 0) ? v.text : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", v.text), "(", 1), print_payloads(v.children, 0, ""), 1), ")", 1));  }
const char* print_payloads(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? acc : ((i == ((ps.len) - 1)) ? maca_cat(acc, (*(Expr*)ps.data[i]).ty) : print_payloads(ps, (i + 1), maca_cat_own(maca_cat(acc, (*(Expr*)ps.data[i]).ty), ", ", 1))));  }
const char* print_fn(Stmt s) { const char* head = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), "(", 1), print_params(s.params, 0, ""), 1), ")", 1), print_ret(s.ret), 1); return (((s.body.len) == 0) ? head : ((((s.body.len) == 1) && ((*(Stmt*)s.body.data[0]).kind == SExpr)) ? maca_cat_own(maca_cat_own(maca_cat(head, " =>\n"), Indent, 1), print_expr((*(Stmt*)s.body.data[0]).value, 1), 1) : maca_cat_own(maca_cat_own(maca_cat(head, " {\n"), print_body(s.body, 0, "", 1), 1), "}", 1)));  }
const char* print_ret(const char* ret) { return ((strcmp(ret, "") == 0) ? "" : maca_cat(" -> ", ret));  }
const char* print_params(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? acc : ((i == ((ps.len) - 1)) ? maca_cat(acc, print_param((*(Expr*)ps.data[i]))) : print_params(ps, (i + 1), maca_cat_own(maca_cat(acc, print_param((*(Expr*)ps.data[i]))), ", ", 1))));  }
const char* print_param(Expr p) { return ((strcmp(p.ty, "") == 0) ? p.text : maca_cat_own(maca_cat_own(maca_cat("", p.text), ": ", 1), p.ty, 1));  }
const char* indent_of(long d) { return ((d <= 0) ? "" : maca_cat(Indent, indent_of((d - 1))));  }
const char* print_body(MacaList body, long i, const char* acc, long d) { return ((i >= (body.len)) ? acc : print_body(body, (i + 1), maca_cat_own(maca_cat_own(maca_cat(acc, indent_of(d)), print_stmt((*(Stmt*)body.data[i]), d), 1), "\n", 1), d));  }
const char* print_stmt(Stmt s, long d) { return ((((s.kind == SBind) && (s.value.kind == ELambda)) && (lambda_body(s.value).kind == EBlock)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), "(", 1), print_params(lambda_params(s.value), 0, ""), 1), ") ", 1), print_wrapped(lambda_body(s.value), d), 1) : (((s.kind == SBind) && (strcmp(s.ret, "") != 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", s.name), ": ", 1), s.ret, 1), " = ", 1), print_expr(s.value, d), 1) : (((s.kind == SBind) || (s.kind == SSet)) ? maca_cat_own(maca_cat_own(maca_cat("", s.name), " = ", 1), print_expr(s.value, d), 1) : print_expr(s.value, d))));  }
const char* print_expr(Expr e, long d) { return (e.kind == EInt ? maca_int_to_str(e.ival) : (e.kind == EFloat ? e.text : (e.kind == EStr ? print_str(e.text) : (e.kind == EBool ? e.text : (e.kind == EIdent ? e.text : (e.kind == ECall ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), "(", 1), print_args(e.children, 0, "", d), 1), ")", 1) : (e.kind == EAttr ? maca_cat_own(maca_cat_own(maca_cat("", e.text), "=", 1), print_expr((*(Expr*)e.children.data[0]), d), 1) : (e.kind == EBinary ? print_binary(e, d) : (e.kind == ETernary ? print_ternary(e, d) : (e.kind == EIf ? print_if(e, d) : (e.kind == EUnary ? print_unary(e, d) : (e.kind == EField ? maca_cat_own(held((*(Expr*)e.children.data[0]), d), maca_cat(".", e.text), 2) : (e.kind == EMethod ? print_method(e, d) : (e.kind == EList ? maca_cat_own(maca_cat("[", print_args(e.children, 0, "", d)), "]", 1) : (e.kind == ERecord ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", e.text), " { ", 1), print_args(e.children, 0, "", d), 1), " }", 1) : (e.kind == EWith ? print_with(e, d) : (e.kind == EMatch ? print_match(e, d) : (e.kind == ELambda ? print_lambda(e, d) : (e.kind == EBlock ? print_wrapped(e, d) : (e.kind == EWhile ? print_while(e, d) : (e.kind == EFor ? print_for(e, d) : (e.kind == EGuard ? print_pattern(e, d) : (e.kind == EJump ? print_jump(e, d) : e.text)))))))))))))))))))))));  }
const char* print_str(const char* s) { return maca_cat_own(maca_cat("\"", doubled_braces(maca_chars(s), 0, "")), "\"", 1);  }
const char* doubled_braces(MacaList cs, long i, const char* acc) { return ((i >= (cs.len)) ? acc : (((strcmp(((const char*)cs.data[i]), "{") == 0) || (strcmp(((const char*)cs.data[i]), "}") == 0)) ? doubled_braces(cs, (i + 1), maca_cat_own(maca_cat(acc, ((const char*)cs.data[i])), ((const char*)cs.data[i]), 1)) : doubled_braces(cs, (i + 1), maca_cat(acc, ((const char*)cs.data[i])))));  }
const char* print_jump(Expr e, long d) { return (((e.children.len) == 0) ? e.text : maca_cat_own(maca_cat(e.text, " "), print_expr((*(Expr*)e.children.data[0]), d), 1));  }
const char* print_method(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(held((*(Expr*)e.children.data[0]), d), maca_cat_own(maca_cat(".", e.text), "(", 1), 2), print_args(e.children, 1, "", d), 1), ")", 1);  }
long loose(Expr e) { return (((e.kind == ETernary) || (e.kind == EIf)) || (e.kind == ELambda));  }
const char* held(Expr e, long d) { const char* inner = print_expr(e, d); return (loose(e) ? maca_cat_own(maca_cat("(", inner), ")", 1) : inner);  }
const char* print_unary(Expr e, long d) { Expr c = (*(Expr*)e.children.data[0]); const char* inner = print_expr(c, d); return (((c.kind == EBinary) || loose(c)) ? maca_cat_own(maca_cat_own(maca_cat(e.text, "("), inner, 1), ")", 1) : maca_cat(e.text, inner));  }
const char* print_with(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat(print_expr((*(Expr*)e.children.data[0]), d), " with { "), print_args(e.children, 1, "", d), 1), " }", 1);  }
const char* print_lambda(Expr e, long d) { MacaList ps = lambda_params(e); const char* body = print_expr(lambda_body(e), d); return ((((ps.len) == 1) && (strcmp((*(Expr*)ps.data[0]).ty, "") == 0)) ? maca_cat_own(maca_cat((*(Expr*)ps.data[0]).text, " => "), body, 1) : maca_cat_own(maca_cat_own(maca_cat("(", print_params(ps, 0, "")), ") => ", 1), body, 1));  }
const char* print_inner(Expr e, long d) { return ((e.kind == EBlock) ? maca_cat_own(maca_cat_own(maca_cat(print_body(e.stmts, 0, "", d), indent_of(d)), print_expr((*(Expr*)e.children.data[0]), d), 1), "\n", 1) : maca_cat_own(maca_cat(indent_of(d), print_expr(e, d)), "\n", 1));  }
const char* print_wrapped(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat("{\n", print_inner(e, (d + 1))), indent_of(d), 1), "}", 1);  }
long needs_block(Expr e) { Expr els = (*(Expr*)e.children.data[2]); return ((((*(Expr*)e.children.data[1]).kind == EBlock) || (els.kind == EBlock)) ? 1 : (((els.kind == EIdent) && (strcmp(els.text, "?") == 0)) ? 1 : ((els.kind == EIf) ? needs_block(els) : 0)));  }
const char* print_if(Expr e, long d) { return (needs_block(e) ? print_guards(e, d) : print_ternary(e, d));  }
const char* print_guards(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("if ", held((*(Expr*)e.children.data[0]), d)), " ", 1), print_wrapped((*(Expr*)e.children.data[1]), d), 1), print_otherwise((*(Expr*)e.children.data[2]), d), 1);  }
const char* print_otherwise(Expr e, long d) { return (((e.kind == EIdent) && (strcmp(e.text, "?") == 0)) ? "" : ((e.kind == EIf) ? maca_cat(" else ", print_guards(e, d)) : maca_cat(" else ", print_wrapped(e, d))));  }
const char* print_while(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("while ", held((*(Expr*)e.children.data[0]), d)), " {\n", 1), print_body(e.stmts, 0, "", (d + 1)), 1), indent_of(d), 1), "}", 1);  }
const char* print_for(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("for ", e.text), " in ", 1), print_expr((*(Expr*)e.children.data[0]), d), 1), " {\n", 1), print_body(e.stmts, 0, "", (d + 1)), 1), indent_of(d), 1), "}", 1);  }
const char* print_match(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("match ", print_expr((*(Expr*)e.children.data[0]), d)), " {\n", 1), print_arms(e.children, 1, "", (d + 1)), 1), indent_of(d), 1), "}", 1);  }
const char* print_arms(MacaList xs, long i, const char* acc, long d) { return (((i + 1) >= (xs.len)) ? acc : print_arms(xs, (i + 2), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(acc, indent_of(d)), print_pattern((*(Expr*)xs.data[i]), d), 1), " => ", 1), print_arm((*(Expr*)xs.data[(i + 1)]), d), 1), "\n", 1), d));  }
const char* print_arm(Expr e, long d) { return ((e.kind == EBlock) ? print_wrapped(e, d) : print_expr(e, d));  }
const char* print_pattern(Expr p, long d) { return ((p.kind == EGuard) ? maca_cat_own(maca_cat(print_pattern((*(Expr*)p.children.data[0]), d), " if "), print_expr((*(Expr*)p.children.data[1]), d), 1) : (((p.kind == EBinary) && (strcmp(p.text, "|") == 0)) ? maca_cat_own(maca_cat(print_pattern((*(Expr*)p.children.data[0]), d), " | "), print_pattern((*(Expr*)p.children.data[1]), d), 1) : (((p.kind != EIdent) || ((p.children.len) == 0)) ? print_expr(p, d) : ((strcmp(p.text, "[..]") == 0) ? print_rest(p.children, d) : (((strcmp(p.text, "[]") == 0) && ((p.children.len) == 1)) ? maca_cat_own(maca_cat("[", print_pattern((*(Expr*)p.children.data[0]), d)), "]", 1) : ((strcmp(p.text, "[]") == 0) ? print_pieces(p.children, 0, "", d) : ((strcmp(p.text, "{}") == 0) ? maca_cat_own(maca_cat("{ ", print_pieces(p.children, 0, "", d)), " }", 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", p.text), "(", 1), print_pieces(p.children, 0, "", d), 1), ")", 1))))))));  }
const char* print_rest(MacaList xs, long d) { Expr last = (*(Expr*)xs.data[((xs.len) - 1)]); const char* head = print_pieces(maca_list_slice(xs, 0, ((xs.len) - 1)), 0, "", d); return (((xs.len) == 1) ? maca_cat("..", last.text) : maca_cat_own(head, maca_cat(", ..", last.text), 2));  }
const char* print_pieces(MacaList xs, long i, const char* acc, long d) { return ((i >= (xs.len)) ? acc : ((i == ((xs.len) - 1)) ? maca_cat(acc, print_pattern((*(Expr*)xs.data[i]), d)) : print_pieces(xs, (i + 1), maca_cat_own(maca_cat(acc, print_pattern((*(Expr*)xs.data[i]), d)), ", ", 1), d)));  }
long op_power(const char* op) { return ((((strcmp(op, "*") == 0) || (strcmp(op, "/") == 0)) || (strcmp(op, "%") == 0)) ? 7 : ((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "++") == 0)) ? 6 : (((strcmp(op, "<<") == 0) || (strcmp(op, ">>") == 0)) ? 5 : ((strcmp(op, "..") == 0) ? 4 : (((((((strcmp(op, "==") == 0) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">=") == 0)) ? 3 : ((strcmp(op, "&&") == 0) ? 2 : ((strcmp(op, "||") == 0) ? 1 : 0)))))));  }
const char* print_binary(Expr e, long d) { long mine = op_power(e.text); const char* l = print_operand((*(Expr*)e.children.data[0]), mine, 0, d); const char* r = print_operand((*(Expr*)e.children.data[1]), mine, 1, d); return ((strcmp(e.text, "=") == 0) ? maca_cat_own(maca_cat(l, " = "), r, 1) : maca_cat_own(maca_cat_own(l, maca_cat_own(maca_cat(" ", e.text), " ", 1), 2), r, 1));  }
const char* print_operand(Expr c, long mine, long side, long d) { const char* inner = print_expr(c, d); return (loose(c) ? maca_cat_own(maca_cat("(", inner), ")", 1) : (((c.kind != EBinary) || (strcmp(c.text, "=") == 0)) ? inner : (((op_power(c.text) < mine) || ((op_power(c.text) == mine) && (side == 1))) ? maca_cat_own(maca_cat("(", inner), ")", 1) : inner)));  }
const char* print_ternary(Expr e, long d) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(held((*(Expr*)e.children.data[0]), d), " ? "), print_expr((*(Expr*)e.children.data[1]), d), 1), " : ", 1), print_expr((*(Expr*)e.children.data[2]), d), 1);  }
const char* print_args(MacaList xs, long i, const char* acc, long d) { return ((i >= (xs.len)) ? acc : ((i == ((xs.len) - 1)) ? maca_cat(acc, print_expr((*(Expr*)xs.data[i]), d)) : print_args(xs, (i + 1), maca_cat_own(maca_cat(acc, print_expr((*(Expr*)xs.data[i]), d)), ", ", 1), d)));  }
MacaList lines(const char* s) { MacaList parts = maca_split(maca_replace(s, "\r\n", "\n"), "\n"); long n = (parts.len); return (((n > 0) && (strcmp(((const char*)parts.data[(n - 1)]), "") == 0)) ? maca_list_slice(parts, 0, (n - 1)) : parts);  }
MacaList words(const char* s) { return keep_nonempty(maca_split(maca_replace(maca_replace(s, "\t", " "), "\n", " "), " "), 0, maca_listv(0));  }
MacaList keep_nonempty(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((strcmp(((const char*)xs.data[i]), "") == 0) ? keep_nonempty(xs, (i + 1), acc) : keep_nonempty(xs, (i + 1), maca_list_pushed(acc, (long)(((const char*)xs.data[i]))))));  }
MacaList split_once(const char* s, const char* sep) { long at = maca_str_index_of(s, sep); return ((at < 0) ? maca_listv(2, (long)(s), (long)("")) : maca_listv(2, (long)(maca_str_slice(s, 0, at)), (long)(maca_str_slice(s, (at + ((int)strlen(sep))), ((int)strlen(s))))));  }
const char* strip_prefix(const char* s, const char* p) { return (maca_starts_with(s, p) ? maca_str_slice(s, ((int)strlen(p)), ((int)strlen(s))) : s);  }
const char* strip_suffix(const char* s, const char* p) { return (maca_ends_with(s, p) ? maca_str_slice(s, 0, (((int)strlen(s)) - ((int)strlen(p)))) : s);  }
long index_of_from(const char* s, const char* pat, long start) { long at = ((start < 0) ? 0 : start); return ((at >= ((int)strlen(s))) ? (-1) : ({ long hit = maca_str_index_of(maca_str_slice(s, at, ((int)strlen(s))), pat); ((hit < 0) ? (-1) : (at + hit)); }));  }
long last_index_of(const char* s, const char* pat) { return scan_last(s, pat, 0, (-1));  }
long scan_last(const char* s, const char* pat, long at, long best) { long hit = index_of_from(s, pat, at); return ((hit < 0) ? best : scan_last(s, pat, (hit + 1), hit));  }
const char* between(const char* s, const char* open_mc, const char* close_mc) { long at = maca_str_index_of(s, open_mc); return ((at < 0) ? "" : ({ const char* tail = maca_str_slice(s, (at + ((int)strlen(open_mc))), ((int)strlen(s))); long end = maca_str_index_of(tail, close_mc); ((end < 0) ? "" : maca_str_slice(tail, 0, end)); }));  }
const char* escape_html(const char* s) { return maca_replace(maca_replace(maca_replace(maca_replace(s, "&", "&amp;"), "<", "&lt;"), ">", "&gt;"), "\"", "&quot;");  }
long count(const char* s, const char* pat) { return ((((int)strlen(pat)) == 0) ? 0 : count_from(s, pat, 0, 0));  }
long count_from(const char* s, const char* pat, long at, long n) { long hit = maca_str_index_of(maca_str_slice(s, at, ((int)strlen(s))), pat); return ((hit < 0) ? n : count_from(s, pat, ((at + hit) + ((int)strlen(pat))), (n + 1)));  }
const char* title_case(const char* s) { return join_words(({ MacaList _m = words(s); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(capitalize(((const char*)_m.data[_i]))); _r; }), 0, "");  }
const char* capitalize(const char* w) { return ((((int)strlen(w)) == 0) ? w : maca_cat_own(maca_upper(maca_str_at(w, 0)), maca_lower(maca_str_slice(w, 1, ((int)strlen(w)))), 3));  }
const char* join_words(MacaList ws, long i, const char* acc) { return ((i >= (ws.len)) ? acc : ((strcmp(acc, "") == 0) ? join_words(ws, (i + 1), ((const char*)ws.data[i])) : join_words(ws, (i + 1), maca_cat_own(maca_cat(acc, " "), ((const char*)ws.data[i]), 1))));  }
const char* indent(const char* s, const char* pad) { return maca_list_join(({ MacaList _m = lines(s); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(({ const char* l = ((const char*)_m.data[_i]); pad_unless_empty(l, pad); })); _r; }), "\n");  }
const char* pad_unless_empty(const char* line, const char* pad) { return ((strcmp(line, "") == 0) ? line : maca_cat(pad, line));  }
const char* dedent(const char* s) { long cut = common_indent(lines(s), 0, (-1)); return ((cut <= 0) ? s : maca_list_join(({ MacaList _m = lines(s); MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(({ const char* l = ((const char*)_m.data[_i]); drop_indent(l, cut); })); _r; }), "\n"));  }
const char* drop_indent(const char* line, long cut) { return ((((int)strlen(line)) >= cut) ? maca_str_slice(line, cut, ((int)strlen(line))) : line);  }
long common_indent(MacaList ls, long i, long best) { return ((i < (ls.len)) ? common_indent(ls, (i + 1), narrower(best, ((const char*)ls.data[i]))) : ((best < 0) ? 0 : best));  }
long narrower(long best, const char* line) { long n = leading_spaces(maca_chars(line), 0); return ((strcmp(maca_trim(line), "") == 0) ? best : (((best < 0) || (n < best)) ? n : best));  }
long leading_spaces(MacaList cs, long i) { return (((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), " ") != 0)) ? i : leading_spaces(cs, (i + 1)));  }
const char* wrap(const char* s, long width) { return fill(words(s), 0, width, "", "");  }
const char* fill(MacaList ws, long i, long width, const char* cur, const char* out) { return ((i >= (ws.len)) ? flush(out, cur) : next_word(ws, i, width, cur, out));  }
const char* next_word(MacaList ws, long i, long width, const char* cur, const char* out) { const char* w = ((const char*)ws.data[i]); const char* joined = ((strcmp(cur, "") == 0) ? w : maca_cat_own(maca_cat(cur, " "), w, 1)); return (((((int)strlen(joined)) <= width) || (strcmp(cur, "") == 0)) ? fill(ws, (i + 1), width, joined, out) : fill(ws, (i + 1), width, w, flush(out, cur)));  }
const char* flush(const char* out, const char* cur) { return ((strcmp(cur, "") == 0) ? out : ((strcmp(out, "") == 0) ? cur : maca_cat_own(maca_cat(out, "\n"), cur, 1)));  }
const char* quote(const char* s) { return maca_cat_own(maca_cat_own("\"", maca_replace(maca_replace(maca_replace(maca_replace(maca_replace(s, "\\", "\\\\"), "\"", "\\\""), "\n", "\\n"), "\r", "\\r"), "\t", "\\t"), 2), "\"", 1);  }
const char* array_of_str(MacaList xs) { return maca_cat_own(maca_cat_own("[", maca_list_join(({ MacaList _m = xs; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(quote(((const char*)_m.data[_i]))); _r; }), ","), 2), "]", 1);  }
const char* array_of_int(MacaList xs) { return maca_cat_own(maca_cat_own("[", maca_list_join(({ MacaList _m = xs; MacaList _r; _r.len = _m.len; _r.data = maca_cells((_r.len ? _r.len : 1) * sizeof(long)); for (int _i = 0; _i < _m.len; _i++) _r.data[_i] = (long)(({ long v = ((long)_m.data[_i]); maca_int_to_str(v); })); _r; }), ","), 2), "]", 1);  }
const char* object_of(MacaList keys, MacaList values) { return maca_cat_own(maca_cat_own("{", maca_list_join(pairs(keys, values, 0, maca_listv(0)), ","), 2), "}", 1);  }
MacaList pairs(MacaList ks, MacaList vs, long i, MacaList acc) { return (((i >= (ks.len)) || (i >= (vs.len))) ? acc : ({ const char* pair = maca_cat_own(maca_cat(quote(((const char*)ks.data[i])), ":"), ((const char*)vs.data[i]), 1); pairs(ks, vs, (i + 1), maca_list_pushed(acc, (long)(pair))); }));  }
const char* get(const char* src, const char* key) { const char* marker = maca_cat(quote(key), ":"); long at = maca_str_index_of(src, marker); return ((at < 0) ? "" : value_at(src, (at + ((int)strlen(marker)))));  }
long get_int(const char* src, const char* key, long dflt) { const char* v = get(src, key); return ((strcmp(v, "") == 0) ? dflt : atol(v));  }
long get_bool(const char* src, const char* key) { return (strcmp(get(src, key), "true") == 0);  }
MacaList items(const char* src) { const char* body = maca_trim(src); return ((!maca_starts_with(body, "[")) ? maca_listv(0) : split_items(maca_chars(maca_str_slice(body, 1, (((int)strlen(body)) - 1))), 0, 0, "", maca_listv(0)));  }
MacaList split_items(MacaList cs, long i, long depth, const char* cur, MacaList acc) { return ((i < (cs.len)) ? next_item(cs, i, depth, cur, acc) : ((strcmp(maca_trim(cur), "") == 0) ? acc : maca_list_pushed(acc, (long)(unwrap(cur)))));  }
MacaList next_item(MacaList cs, long i, long depth, const char* cur, MacaList acc) { const char* c = ((const char*)cs.data[i]); return ((strcmp(c, "\"") == 0) ? copy_string(cs, (i + 1), depth, maca_cat(cur, c), acc) : (((strcmp(c, ",") == 0) && (depth == 0)) ? split_items(cs, (i + 1), depth, "", maca_list_pushed(acc, (long)(unwrap(cur)))) : split_items(cs, (i + 1), (depth + nesting(c)), maca_cat(cur, c), acc)));  }
long nesting(const char* c) { return (((strcmp(c, "[") == 0) || (strcmp(c, "{") == 0)) ? 1 : (((strcmp(c, "]") == 0) || (strcmp(c, "}") == 0)) ? (-1) : 0));  }
MacaList copy_string(MacaList cs, long i, long depth, const char* cur, MacaList acc) { return ((i >= (cs.len)) ? split_items(cs, i, depth, cur, acc) : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? split_items_after_escape(cs, i, depth, cur, acc) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? split_items(cs, (i + 1), depth, maca_cat(cur, "\""), acc) : copy_string(cs, (i + 1), depth, maca_cat(cur, ((const char*)cs.data[i])), acc))));  }
MacaList split_items_after_escape(MacaList cs, long i, long depth, const char* cur, MacaList acc) { const char* escaped = (((i + 1) < (cs.len)) ? ((const char*)cs.data[(i + 1)]) : ""); return copy_string(cs, (i + 2), depth, maca_cat_own(maca_cat(cur, ((const char*)cs.data[i])), escaped, 1), acc);  }
const char* value_at(const char* src, long at) { const char* rest = maca_trim(maca_str_slice(src, at, ((int)strlen(src)))); return (maca_starts_with(rest, "\"") ? unescape(maca_str_slice(rest, 1, quote_end(maca_chars(rest), 1))) : ((maca_starts_with(rest, "[") || maca_starts_with(rest, "{")) ? maca_str_slice(rest, 0, balanced_end(maca_chars(rest), 0, 0)) : maca_str_slice(rest, 0, bare_end(maca_chars(rest), 0))));  }
long balanced_end(MacaList cs, long i, long depth) { return ((i >= (cs.len)) ? i : balanced_step(cs, i, depth));  }
long balanced_step(MacaList cs, long i, long depth) { const char* c = ((const char*)cs.data[i]); return ((strcmp(c, "\"") == 0) ? balanced_end(cs, skip_string(cs, (i + 1)), depth) : (((strcmp(c, "[") == 0) || (strcmp(c, "{") == 0)) ? balanced_end(cs, (i + 1), (depth + 1)) : (((strcmp(c, "]") != 0) && (strcmp(c, "}") != 0)) ? balanced_end(cs, (i + 1), depth) : ((depth <= 1) ? (i + 1) : balanced_end(cs, (i + 1), (depth - 1))))));  }
long skip_string(MacaList cs, long i) { long end = quote_end(cs, i); return ((end >= (cs.len)) ? (cs.len) : (end + 1));  }
long quote_end(MacaList cs, long i) { return ((i >= (cs.len)) ? (cs.len) : ((strcmp(((const char*)cs.data[i]), "\\") == 0) ? quote_end(cs, (i + 2)) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? i : quote_end(cs, (i + 1)))));  }
long bare_end(MacaList cs, long i) { return ((i >= (cs.len)) ? i : ((((strcmp(((const char*)cs.data[i]), ",") == 0) || (strcmp(((const char*)cs.data[i]), "}") == 0)) || (strcmp(((const char*)cs.data[i]), "]") == 0)) ? i : bare_end(cs, (i + 1))));  }
const char* unwrap(const char* s) { const char* t = maca_trim(s); return ((((((int)strlen(t)) >= 2) && maca_starts_with(t, "\"")) && maca_ends_with(t, "\"")) ? unescape(maca_str_slice(t, 1, (((int)strlen(t)) - 1))) : t);  }
const char* unescape(const char* s) { return maca_replace(maca_replace(maca_replace(maca_replace(maca_replace(s, "\\n", "\n"), "\\r", "\r"), "\\t", "\t"), "\\\"", "\""), "\\\\", "\\");  }
Trace trace(const char* label, MacaList spans) { return (Trace){ .label = label, .spans = spans };  }
Span region(const char* name, long start, long end) { return (Span){ .name = name, .start = start, .end = end, .depth = 0, .parent = (-1), .closed = 1 };  }
long duration(Span s) { return (s.end - s.start);  }
long span_count(Trace t) { return (t.spans.len);  }
long wall(Trace t) { return (((t.spans.len) == 0) ? 0 : (last_end(t.spans, 0, (*(Span*)t.spans.data[0]).end) - origin(t)));  }
long origin(Trace t) { return (((t.spans.len) == 0) ? 0 : first_start(t.spans, 0, (*(Span*)t.spans.data[0]).start));  }
long first_start(MacaList xs, long i, long best) { return ((i >= (xs.len)) ? best : (((*(Span*)xs.data[i]).start < best) ? first_start(xs, (i + 1), (*(Span*)xs.data[i]).start) : first_start(xs, (i + 1), best)));  }
long last_end(MacaList xs, long i, long best) { return ((i >= (xs.len)) ? best : (((*(Span*)xs.data[i]).end > best) ? last_end(xs, (i + 1), (*(Span*)xs.data[i]).end) : last_end(xs, (i + 1), best)));  }
MacaList roots(Trace t) { return kids_of(t.spans, (-1), 0, maca_listv(0));  }
MacaList children(Trace t, long i) { return kids_of(t.spans, i, 0, maca_listv(0));  }
MacaList kids_of(MacaList xs, long parent, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((held_by(xs, i) == parent) ? kids_of(xs, parent, (i + 1), maca_list_pushed(acc, (long)(i))) : kids_of(xs, parent, (i + 1), acc)));  }
long held_by(MacaList xs, long i) { long p = (*(Span*)xs.data[i]).parent; return (((p >= 0) && (p < i)) ? p : (-1));  }
long levels(Trace t) { return deepest(t.spans, 0, 0);  }
long deepest(MacaList xs, long i, long best) { return ((i >= (xs.len)) ? best : ((((*(Span*)xs.data[i]).depth + 1) > best) ? deepest(xs, (i + 1), ((*(Span*)xs.data[i]).depth + 1)) : deepest(xs, (i + 1), best)));  }
MacaList level(Trace t, long d) { return at_depth(t.spans, d, 0, maca_listv(0));  }
MacaList at_depth(MacaList xs, long d, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : (((*(Span*)xs.data[i]).depth == d) ? at_depth(xs, d, (i + 1), maca_list_pushed(acc, (long)(i))) : at_depth(xs, d, (i + 1), acc)));  }
long child_time(Trace t, long i) { return sum_kids(t, children(t, i), 0, 0);  }
long sum_kids(Trace t, MacaList ids, long i, long acc) { return ((i >= (ids.len)) ? acc : sum_kids(t, ids, (i + 1), (acc + duration((*(Span*)t.spans.data[((long)ids.data[i])])))));  }
long self_time(Trace t, long i) { long own = (duration((*(Span*)t.spans.data[i])) - child_time(t, i)); return ((own > 0) ? own : 0);  }
MacaList leaked(Trace t) { return unclosed(t.spans, 0, maca_listv(0));  }
MacaList unclosed(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((*(Span*)xs.data[i]).closed ? unclosed(xs, (i + 1), acc) : unclosed(xs, (i + 1), maca_list_pushed(acc, (long)((*(Span*)xs.data[i]).name)))));  }
long find_span(Trace t, const char* name) { return scan_name(t.spans, name, 0);  }
long scan_name(MacaList xs, const char* name, long i) { return ((i >= (xs.len)) ? (-1) : ((strcmp((*(Span*)xs.data[i]).name, name) == 0) ? i : scan_name(xs, name, (i + 1))));  }
const char* to_json(Trace t) { MacaList parts = span_objects(t.spans, 0, maca_listv(0)); const char* body = maca_list_join(parts, ","); const char* head = maca_cat("{\"label\":", quote(t.label)); return maca_cat_own(maca_cat_own(maca_cat(head, ",\"spans\":["), body, 1), "]}", 1);  }
MacaList span_objects(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : span_objects(xs, (i + 1), maca_list_pushed(acc, (long)(span_json((*(Span*)xs.data[i]))))));  }
const char* span_json(Span s) { const char* flag = (s.closed ? "true" : "false"); const char* when = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("\"start\":", maca_int_to_str(s.start), 2), ",\"end\":", 1), maca_int_to_str(s.end), 3), ",\"depth\":", 1), maca_int_to_str(s.depth), 3); const char* where = maca_cat_own(maca_cat_own(maca_cat_own("\"parent\":", maca_int_to_str(s.parent), 2), ",\"closed\":", 1), flag, 1); const char* head = maca_cat("{\"name\":", quote(s.name)); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(head, ","), when, 1), ",", 1), where, 1), "}", 1);  }
Trace from_json(const char* src) { return trace(get(src, "label"), read_spans(items(get(src, "spans")), 0, maca_listv(0)));  }
MacaList read_spans(MacaList objs, long i, MacaList acc) { return ((i >= (objs.len)) ? acc : read_spans(objs, (i + 1), maca_list_pushed(acc, maca_box(sizeof(Span), (Span[]){ read_span(((const char*)objs.data[i])) }))));  }
Span read_span(const char* o) { return (Span){ .name = get(o, "name"), .start = get_int(o, "start", 0), .end = get_int(o, "end", 0), .depth = get_int(o, "depth", 0), .parent = get_int(o, "parent", (-1)), .closed = get_bool(o, "closed") };  }
long enabled() { return ((strcmp(maca_env("NO_COLOR"), "") != 0) ? 0 : ((strcmp(maca_env("FORCE_COLOR"), "") != 0) ? 1 : maca_is_tty()));  }
const char* paint(const char* code, const char* s) { return (((!enabled()) || (strcmp(s, "") == 0)) ? s : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_chr(27), "[", 1), code, 1), "m", 1), s, 1), maca_chr(27), 3), "[0m", 1));  }
long width(const char* s) { long n = ((int)strlen(s)); long out = 0; long i = 0; while ((i < n)) { long b = maca_ord(maca_str_at(s, i)); if ((b == 27)) { i = skip_escape(s, (i + 1)); i; } else if ((b < 128)) { out = (out + 1); i = (i + 1); i; } else { long len = utf8_len(b); out = (out + columns(codepoint(s, i, len))); i = (i + len); i; } } return out;  }
long utf8_len(long b) { return ((b >= 240) ? 4 : ((b >= 224) ? 3 : ((b >= 192) ? 2 : 1)));  }
long codepoint(const char* s, long i, long len) { long lead = maca_ord(maca_str_at(s, i)); long cp = (lead - lead_base(len)); long at = 1; while ((at < len)) { cp = ((cp * 64) + tail(s, (i + at))); at = (at + 1); } return cp;  }
long lead_base(long len) { return ((len == 4) ? 240 : ((len == 3) ? 224 : ((len == 2) ? 192 : 0)));  }
long tail(const char* s, long i) { long b = maca_ord(maca_str_at(s, i)); return ((b >= 128) ? (b - 128) : 0);  }
long columns(long cp) { return (((cp >= 768) && (cp <= 879)) ? 0 : ((cp == 8203) ? 0 : (((cp >= 4352) && (cp <= 4447)) ? 2 : (((cp >= 11904) && (cp <= 42191)) ? 2 : (((cp >= 44032) && (cp <= 55203)) ? 2 : (((cp >= 63744) && (cp <= 64255)) ? 2 : (((cp >= 65072) && (cp <= 65135)) ? 2 : (((cp >= 65280) && (cp <= 65376)) ? 2 : (((cp >= 127744) && (cp <= 129791)) ? 2 : 1)))))))));  }
long skip_escape(const char* s, long from) { long n = ((int)strlen(s)); long i = from; while (((i < n) && (!(isalpha((unsigned char)(maca_str_at(s, i))[0]) != 0)))) { i = (i + 1); } return (i + 1);  }
const char* pad(const char* s, long w) { long short_mc = (w - width(s)); return ((short_mc > 0) ? maca_cat_own(s, maca_repeat(" ", short_mc), 2) : s);  }
const char* pad_left(const char* s, long w) { long short_mc = (w - width(s)); return ((short_mc > 0) ? maca_cat_own(maca_repeat(" ", short_mc), s, 1) : s);  }
const char* plain(const char* s) { long n = ((int)strlen(s)); const char* out = ""; long i = 0; while ((i < n)) { if ((strcmp(maca_str_at(s, i), maca_chr(27)) == 0)) { i = skip_escape(s, (i + 1)); i; } else { out = maca_cat_own(out, maca_str_at(s, i), 2); i = (i + 1); i; } } return out;  }
const char* bold(const char* s) { return paint("1", s);  }
const char* dim(const char* s) { return paint("2", s);  }
const char* italic(const char* s) { return paint("3", s);  }
const char* underline(const char* s) { return paint("4", s);  }
const char* red(const char* s) { return paint("31", s);  }
const char* green(const char* s) { return paint("32", s);  }
const char* yellow(const char* s) { return paint("33", s);  }
const char* blue(const char* s) { return paint("34", s);  }
const char* magenta(const char* s) { return paint("35", s);  }
const char* cyan(const char* s) { return paint("36", s);  }
const char* grey(const char* s) { return paint("90", s);  }
const char* ok(const char* s) { return maca_cat_own(maca_cat(green("✓"), " "), s, 1);  }
const char* warn(const char* s) { return maca_cat_own(maca_cat(yellow("!"), " "), s, 1);  }
const char* bad(const char* s) { return maca_cat_own(maca_cat(red("✗"), " "), s, 1);  }
const char* note__20(const char* s) { return maca_cat_own(maca_cat(grey("-"), " "), grey(s), 1);  }
Scale scale_of(Trace t, long cols) { return (Scale){ .base = origin(t), .total = wall(t), .cols = cols };  }
long column(Scale sc, long at) { return ((sc.total <= 0) ? 0 : (((at - sc.base) * sc.cols) / sc.total));  }
const char* flame(Trace t, long cols) { return ((span_count(t) == 0) ? dim("(nothing recorded)") : ({ MacaList rows = chart_rows(t, scale_of(t, cols), 0, maca_listv(0)); maca_cat_own(maca_cat_own(maca_cat(chart_head(t), "\n"), maca_list_join(rows, "\n"), 3), leak_note(t), 1); }));  }
const char* chart_head(Trace t) { const char* seen = maca_cat_own(maca_cat(plural(span_count(t), "span"), ", "), plural(levels(t), "level"), 1); const char* counts = maca_cat_own(maca_cat_own(maca_cat_own(", ", maca_int_to_str(wall(t)), 2), "ms, ", 1), seen, 1); return maca_cat(bold(t.label), dim(counts));  }
const char* plural(long n, const char* noun) { const char* ending = ((n == 1) ? "" : "s"); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("", maca_int_to_str(n), 2), " ", 1), noun, 1), ending, 1);  }
const char* leak_note(Trace t) { MacaList open_mc = leaked(t); return (((open_mc.len) == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat("\n", yellow("!")), " never closed: ", 1), maca_list_join(open_mc, ", "), 3));  }
MacaList chart_rows(Trace t, Scale sc, long d, MacaList acc) { return ((d >= levels(t)) ? acc : ({ const char* row = lay_out(t, sc, level(t, d), 0, 0, ""); chart_rows(t, sc, (d + 1), maca_list_pushed(acc, (long)(row))); }));  }
const char* lay_out(Trace t, Scale sc, MacaList ids, long i, long cursor, const char* acc) { return ((i >= (ids.len)) ? acc : ({ Span s = (*(Span*)t.spans.data[((long)ids.data[i])]); long from = bar_start(sc, s, cursor); long w = bar_width(sc, s, from); const char* gap = maca_repeat(" ", (from - cursor)); const char* drawn = maca_cat_own(maca_cat(acc, gap), tint(((long)ids.data[i]), bar_text(s, w)), 1); lay_out(t, sc, ids, (i + 1), (from + w), drawn); }));  }
long bar_start(Scale sc, Span s, long cursor) { long left = column(sc, s.start); return ((left < cursor) ? cursor : left);  }
long bar_width(Scale sc, Span s, long from) { long w = (column(sc, s.end) - from); return ((w < 1) ? 1 : w);  }
const char* bar_text(Span s, long w) { return ((w < 3) ? maca_repeat("▏", w) : maca_cat_own(maca_cat("[", inside(s.name, duration(s), (w - 2))), "]", 1));  }
const char* inside(const char* name, long ms, long room) { const char* full = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", name), " ", 1), maca_int_to_str(ms), 3), "ms", 1); return ((width(full) <= room) ? pad(full, room) : ((width(name) <= room) ? pad(name, room) : pad(maca_cat(fit(name, (room - 1)), "…"), room)));  }
const char* fit(const char* s, long n) { return ((width(s) <= n) ? s : maca_str_slice(s, 0, widest_prefix(s, n, 0, 0)));  }
long widest_prefix(const char* s, long n, long i, long best) { return ((i > ((int)strlen(s))) ? best : ((!on_boundary(s, i)) ? widest_prefix(s, n, (i + 1), best) : ((width(maca_str_slice(s, 0, i)) > n) ? best : widest_prefix(s, n, (i + 1), i))));  }
long on_boundary(const char* s, long i) { return ((i >= ((int)strlen(s))) ? 1 : ({ long b = maca_ord(maca_str_at(s, i)); ((b < 128) || (b >= 192)); }));  }
const char* tint(long i, const char* s) { return paint(sgr(i), s);  }
const char* sgr(long i) { MacaList codes = maca_listv(6, (long)("36"), (long)("32"), (long)("33"), (long)("34"), (long)("35"), (long)("96")); return ((const char*)codes.data[(i % (codes.len))]);  }
const char* flame_svg(Trace t, long px) { return flame_svg_in(t, px, "ms");  }
const char* flame_svg_in(Trace t, long px, const char* unit) { long inset = 8; long rows = levels(t); long high = ((svg_head_h() + (rows * svg_row())) + inset); Scale sc = scale_of(t, (px - (inset * 2))); const char* body = svg_frames(t, sc, inset, 0, "", unit); return maca_cat_own(maca_cat_own(maca_cat(svg_open(px, high), svg_backdrop(t, px, high, inset, unit)), body, 1), "</svg>\n", 1);  }
long svg_head_h() { return 38;  }
long svg_row() { return 22;  }
const char* svg_open(long px, long high) { const char* ns = "xmlns=\"http://www.w3.org/2000/svg\""; const char* box = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("viewBox=\"0 0 ", maca_int_to_str(px), 2), " ", 1), maca_int_to_str(high), 3), "\"", 1); const char* face = "font-family=\"'JetBrainsMono Nerd Font', monospace\" font-size=\"11\""; return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("<svg ", ns), " width=\"", 1), maca_int_to_str(px), 3), "\" height=\"", 1), maca_int_to_str(high), 3), "\" ", 1), box, 1), " ", 1), face, 1), ">\n", 1);  }
const char* svg_backdrop(Trace t, long px, long high, long inset, const char* unit) { const char* back = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("<rect width=\"", maca_int_to_str(px), 2), "\" height=\"", 1), maca_int_to_str(high), 3), "\" fill=\"#12121a\"/>\n", 1); const char* name = escape_html(t.label); const char* seen = maca_cat_own(maca_cat(plural(span_count(t), "span"), ", "), plural(levels(t), "level"), 1); const char* counts = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("", maca_int_to_str(wall(t)), 2), unit, 1), ", ", 1), seen, 1); const char* top = maca_cat_own(maca_cat_own("<text x=\"", maca_int_to_str(inset), 2), "\" y=\"17\" fill=\"#f2f1f8\" font-weight=\"600\">", 1); const char* sub = maca_cat_own(maca_cat_own("<text x=\"", maca_int_to_str(inset), 2), "\" y=\"31\" fill=\"#8d8c9b\">", 1); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(back, top), name, 1), "</text>\n", 1), sub, 1), counts, 1), "</text>\n", 1);  }
const char* svg_frames(Trace t, Scale sc, long inset, long i, const char* acc, const char* unit) { return ((i >= span_count(t)) ? acc : svg_frames(t, sc, inset, (i + 1), maca_cat(acc, svg_frame(t, sc, inset, i, unit)), unit));  }
const char* svg_frame(Trace t, Scale sc, long inset, long i, const char* unit) { Span s = (*(Span*)t.spans.data[i]); long left = (inset + column(sc, s.start)); long w = frame_width(sc, s, inset, left); long y = (svg_head_h() + (s.depth * svg_row())); const char* tip = maca_cat_own(maca_cat("<title>", svg_tip(t, i, unit)), "</title>", 1); const char* box = svg_rect(left, y, w, i); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("<g>", tip), box, 1), svg_label(s, left, y, w), 1), "</g>\n", 1);  }
long frame_width(Scale sc, Span s, long inset, long left) { long w = ((inset + column(sc, s.end)) - left); return ((w < 2) ? 2 : w);  }
const char* svg_rect(long x, long y, long w, long i) { const char* size = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("x=\"", maca_int_to_str(x), 2), "\" y=\"", 1), maca_int_to_str(y), 3), "\" width=\"", 1), maca_int_to_str(w), 3), "\" height=\"", 1), maca_int_to_str((svg_row() - 3)), 3), "\"", 1); const char* look = maca_cat_own(maca_cat("rx=\"2\" fill=\"", swatch(i)), "\" stroke=\"#12121a\"", 1); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("<rect ", size), " ", 1), look, 1), "/>", 1);  }
const char* svg_label(Span s, long x, long y, long w) { long room = ((w - 8) / 7); return ((room < 3) ? "" : ({ const char* text = escape_html(fit(s.name, room)); const char* at = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("x=\"", maca_int_to_str((x + 4)), 2), "\" y=\"", 1), maca_int_to_str(((y + svg_row()) - 9)), 3), "\"", 1); maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("<text ", at), " fill=\"#12121a\" font-weight=\"600\">", 1), text, 1), "</text>", 1); }));  }
const char* svg_tip(Trace t, long i, const char* unit) { Span s = (*(Span*)t.spans.data[i]); double share = percent(duration(s), wall(t)); const char* own = maca_cat_own(maca_cat_own("self ", maca_int_to_str(self_time(t, i)), 2), unit, 1); return maca_cat_own(maca_cat_own(escape_html(s.name), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(": ", maca_int_to_str(duration(s)), 2), unit, 1), " (", 1), maca_fixed(share, 1), 3), "%), ", 1), 2), own, 1);  }
double percent(long part, long whole) { return ((whole <= 0) ? 0.0 : ((((double)(part)) * 100.0) / ((double)(whole))));  }
const char* swatch(long i) { MacaList fills = maca_listv(6, (long)("#7fd6d0"), (long)("#8fd694"), (long)("#e3cd7a"), (long)("#9db6ea"), (long)("#cba4dd"), (long)("#8ad4e8")); return ((const char*)fills.data[(i % (fills.len))]);  }
long maca_main(MacaList args) { return ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "build") == 0)) ? build_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "run") == 0)) ? run_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "test") == 0)) ? test_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "check") == 0)) ? check_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "fix") == 0)) ? fix_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "spec") == 0)) ? spec_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "fmt") == 0)) ? fmt_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "watch") == 0)) ? watch_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "profile") == 0)) ? profile_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "dev") == 0)) ? dev_cmd(args) : ((((args.len) >= 1) && module_asked(((const char*)args.data[0]))) ? module_cmd(args) : ((((args.len) >= 1) && tooled(((const char*)args.data[0]))) ? run_file(tool_path(((const char*)args.data[0])), maca_list_slice(args, 1, (args.len))) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "add") == 0)) ? add_cmd(args) : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "install") == 0)) ? install_cmd() : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "update") == 0)) ? update_cmd() : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "upgrade") == 0)) ? upgrade_cmd() : ((((args.len) >= 1) && (strcmp(((const char*)args.data[0]), "init") == 0)) ? init_project((((args.len) >= 2) ? ((const char*)args.data[1]) : ".")) : ((((args.len) >= 1) && version_asked(((const char*)args.data[0]))) ? ({ maca_say(stdout, maca_cat("maca ", Version), "\n", 1); 0; }) : ((((args.len) >= 1) && help_asked(((const char*)args.data[0]))) ? usage() : ((((args.len) >= 2) && maca_ends_with(((const char*)args.data[0]), ".maca")) ? compile_file(args) : (((args.len) >= 1) ? unknown_cmd(((const char*)args.data[0])) : demo())))))))))))))))))))));  }
int main(int argc, char** argv) { return maca_main(maca_args(argc, argv)); }
long version_asked(const char* a) { return (((strcmp(a, "--version") == 0) || (strcmp(a, "-V") == 0)) || (strcmp(a, "version") == 0));  }
long help_asked(const char* a) { return (((strcmp(a, "--help") == 0) || (strcmp(a, "-h") == 0)) || (strcmp(a, "help") == 0));  }
long usage() { maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca ", Version), "\n", 1), "\nusage: maca <command> [args]\n", 1), "\ncommands:\n", 1), "  init  [dir]                  scaffold a new project (maca.toml, main.maca)\n", 1), "  build [file.maca] [-o out]   compile (native | --target nix|js|rust|wasm)\n", 1), "  run   [file.maca] [args..]   compile and run\n", 1), "  -m <module>[.<fn>] [args..]  run one function out of a module\n", 1), "  watch <file.maca> [args..]   rebuild and rerun it on every change\n", 1), "  fmt   <file.maca>... [--check]  format in place\n", 1), "  lint  [path...]              Maca style rules, over a file or a tree\n", 1), "  doc   <dir> <file.maca>...   an API reference, rendered from /// lines\n", 1), "  test  [file.maca]            every `test_...` in the file, or every suite\n", 1), "  profile <file.maca> [-o s]   where the run spent its instructions\n", 1), "  dev   [dev.maca] [-o out]    the flake `nix develop` enters\n", 1), "  add   <spec>...              add a dependency, and fetch it\n", 1), "  install                      fetch what maca.toml names, at the locked version\n", 1), "  update                       re-resolve every dependency to its newest match\n", 1), "  upgrade                      replace this toolchain with the latest release\n", 1), "  bindgen <header.h> <out>     a C header, as Maca FFI declarations\n", 1), "  check [file.maca]            diagnostics, one line each\n", 1), "  fix   <file.maca>...         delete the keywords Maca has ", 1), "no word for\n", 1), "  spec  [--package <name>]     the language, as one document\n", 1), "  --version                    print the toolchain version\n", 1), "  --help                       this\n", 1), "\nwith no file, build/run/test/check are about the package the directory holds:\n", 1), "  its [[bin]], chosen with --bin <name> or [build] bin\n", 1), "\nbuild targets: native (default), --target nix | js | rust ", 1), "| jvm | tauri | wasm | embedded\n", 1), "\n[build] in maca.toml declares target, out, mcu, classpath, cflags and\n", 1), "  bin, so a project builds by\n", 1), "  saying `maca build`; a flag on the line still wins\n", 1), "\na [scripts] name in maca.toml is a command of its own, so `maca <name>`\n", 1), "  runs the line that table gives it", 1), "\n", 1); return 0;  }
long unknown_cmd(const char* name) { const char* script = chain_value(here_chain(), 0, "[scripts]", name); return ((strcmp(script, "") != 0) ? run_script(script) : ({ maca_say(stderr, maca_cat_own(maca_cat("maca: unknown command `", name), "`", 1), "\n", 1); usage(); 2; }));  }
long run_script(const char* cmd) { return ((strcmp(maca_env("OS"), "Windows_NT") == 0) ? maca_exec("cmd", maca_listv(2, (long)("/C"), (long)(cmd))) : maca_exec("sh", maca_listv(2, (long)("-c"), (long)(cmd))));  }
long check_only(const char* src, const char* target) { Unit unit = unit_of(src); Lexed scanned = end_run(lexed(unit.toks, unit.errs), 0, 0); Module parsed = parse_module(scanned.tokens, 0, maca_listv(0)); Module raw = desugared(parsed, src); return ((((report_all("import", unit.unknown, 0) + report_all("scan", scanned.errors, 0)) + report_all("parse", parsed.errors, 0)) + report_all("embed", raw.errors, 0)) + report_all("check", check_errors_on(raw, target), 0));  }
long init_project(const char* root) { const char* name = leaf_of(root); maca_make_dir(root); write_absent(maca_cat_own(maca_cat("", root), "/maca.toml", 1), maca_cat_own(maca_cat_own(maca_cat("[package]\nname = \"", name), "\"\n\n[[bin]]\n", 1), "path = \"main.maca\"\n", 1)); write_absent(maca_cat_own(maca_cat("", root), "/main.maca", 1), "main() -> int {\n    info(\"hello\")\n    0\n}\n"); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat("initialized Maca project `", name), "` in ", 1), root, 1), "\n", 1); return 0;  }
long write_absent(const char* path, const char* text) { return (maca_file_exists(path) ? 0 : maca_write_file(path, text));  }
const char* leaf_of(const char* root) { long cut = sep_after(maca_chars(root), (((int)strlen(root)) - 1)); const char* tail = maca_str_slice(root, cut, ((int)strlen(root))); return (((strcmp(tail, "") == 0) || (strcmp(tail, ".") == 0)) ? "app" : tail);  }
const char* indent_unit(MacaList chain) { const char* size = chain_value(chain, 0, "[format]", "indent_size"); long wide = ((strcmp(size, "") == 0) ? 4 : atol(size)); return ((strcmp(chain_value(chain, 0, "[format]", "indent_style"), "tab") == 0) ? "\t" : space_run(wide));  }
const char* space_run(long n) { return ((n <= 0) ? "" : maca_cat(" ", space_run((n - 1))));  }
const char* reindent(const char* text, const char* unit) { return ((strcmp(unit, "    ") == 0) ? text : maca_list_join(reindent_lines(maca_split(text, "\n"), unit, 0, maca_listv(0)), "\n"));  }
MacaList reindent_lines(MacaList lines, const char* unit, long i, MacaList acc) { return ((i >= (lines.len)) ? acc : reindent_lines(lines, unit, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(reindent_line(((const char*)lines.data[i]), unit))))));  }
const char* reindent_line(const char* line, const char* unit) { long n = indent_width(line, 0); return maca_cat_own(maca_cat(unit_times(unit, (n / 4)), space_run((n % 4))), maca_str_slice(line, n, ((int)strlen(line))), 3);  }
const char* unit_times(const char* unit, long n) { return ((n <= 0) ? "" : maca_cat(unit, unit_times(unit, (n - 1))));  }
long indent_width(const char* line, long i) { return (((i < ((int)strlen(line))) && (strcmp(maca_str_at(line, i), " ") == 0)) ? indent_width(line, (i + 1)) : i);  }
long format_file(const char* src, long only_check) { const char* before = maca_read_file(src); const char* unit = indent_unit(chain_of(src)); const char* after = reindent(print_source(before), unit); return ((strcmp(after, reindent(print_source(after), unit)) != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat("", src), ": left as it is, the formatter cannot write it back", 1), "\n", 1); 1; }) : ((strcmp(before, after) == 0) ? 0 : (only_check ? ({ maca_say(stdout, maca_cat_own(maca_cat("", src), ": not formatted", 1), "\n", 1); 1; }) : ({ maca_write_file(src, after); 0; }))));  }
long fmt_cmd(MacaList args) { MacaList files = fix_files(args, 1, maca_listv(0)); return (((files.len) == 0) ? ({ maca_say(stderr, "usage: maca fmt <file.maca>... [--check]", "\n", 0); 2; }) : fmt_each(files, 0, (maca_list_index_of_str(args, "--check") >= 0), 0));  }
long fmt_each(MacaList files, long i, long only_check, long acc) { return ((i >= (files.len)) ? acc : fmt_each(files, (i + 1), only_check, (acc + format_file(((const char*)files.data[i]), only_check))));  }
long fix_cmd(MacaList args) { MacaList files = fix_files(args, 1, maca_listv(0)); return (((files.len) == 0) ? ({ maca_say(stderr, "usage: maca fix <file.maca>... [--dry-run]", "\n", 0); 2; }) : fix_each(files, 0, (maca_list_index_of_str(args, "--dry-run") >= 0), 0));  }
MacaList fix_files(MacaList args, long i, MacaList acc) { return ((i >= (args.len)) ? acc : (maca_starts_with(((const char*)args.data[i]), "--") ? fix_files(args, (i + 1), acc) : fix_files(args, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)args.data[i])))))));  }
long fix_each(MacaList files, long i, long dry, long bad) { return ((i >= (files.len)) ? bad : fix_each(files, (i + 1), dry, (bad + fix_one(((const char*)files.data[i]), dry))));  }
long fix_one(const char* path, long dry) { return ((!maca_file_exists(path)) ? ({ maca_say(stderr, maca_cat("fix: cannot read ", path), "\n", 1); 1; }) : fix_lexed(path, maca_read_file(path), dry));  }
long fix_lexed(const char* path, const char* src, long dry) { MacaList ts = lex(src); long n = phantom_count(ts, 0, 0); if (((n > 0) && (!dry))) { maca_write_file(path, maca_list_join(cut_all(maca_chars(src), ts, ((ts.len) - 1)), "")); } maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", path), ": ", 1), fix_said(dry), 1), " ", 1), maca_int_to_str(n), 3), ", left the rest to be read", 1), "\n", 1); return 0;  }
const char* fix_said(long dry) { return (dry ? "would delete" : "deleted");  }
long phantom_at(MacaList ts, long i) { return (((((i + 1) < (ts.len)) && phantom_word((*(Token*)ts.data[i]))) && ((*(Token*)ts.data[(i + 1)]).kind == TIdent)) && (!(*(Token*)ts.data[(i + 1)]).fresh));  }
long phantom_word(Token t) { return (((t.kind == TIdent) || (t.kind == KwLet)) && phantom_spelling(t.text));  }
long phantom_spelling(const char* w) { return (((((((strcmp(w, "let") == 0) || (strcmp(w, "var") == 0)) || (strcmp(w, "fn") == 0)) || (strcmp(w, "func") == 0)) || (strcmp(w, "def") == 0)) || (strcmp(w, "type") == 0)) || (strcmp(w, "async") == 0));  }
long phantom_count(MacaList ts, long i, long n) { return ((i >= (ts.len)) ? n : (phantom_at(ts, i) ? phantom_count(ts, (i + 1), (n + 1)) : phantom_count(ts, (i + 1), n)));  }
MacaList cut_all(MacaList cs, MacaList ts, long i) { return ((i < 0) ? cs : (phantom_at(ts, i) ? cut_all(cut_one(cs, (*(Token*)ts.data[i])), ts, (i - 1)) : cut_all(cs, ts, (i - 1))));  }
MacaList cut_one(MacaList cs, Token t) { return maca_list_cat(maca_list_slice(cs, 0, t.pos), maca_list_slice(cs, space_end(cs, (t.pos + ((int)strlen(t.text)))), (cs.len)));  }
long space_end(MacaList cs, long i) { return (((i < (cs.len)) && (strcmp(((const char*)cs.data[i]), " ") == 0)) ? space_end(cs, (i + 1)) : i);  }
long watch_cmd(MacaList args) { const char* src = first_named(args); return ((strcmp(src, "") == 0) ? ({ maca_say(stderr, "watch: expected a .maca file", "\n", 0); 2; }) : ((!maca_file_exists(src)) ? ({ maca_say(stderr, maca_cat_own(maca_cat("watch: ", src), " is not a file", 1), "\n", 1); 2; }) : ({ maca_say(stdout, maca_cat_own(maca_cat("watching ", src), "; Ctrl-C to stop", 1), "\n", 1); watched(src, maca_list_slice(args, 2, (args.len))); })));  }
long watched(const char* src, MacaList rest) { MacaList files = unit_of(src).seen; long seen = 0; while (1) { long now = newest_ms(files, 0, 0); if ((now > seen)) { seen = now; maca_say(stdout, maca_cat_own(maca_cat("── ", src), " changed, rebuilding ──", 1), "\n", 1); maca_say(stdout, maca_cat_own(maca_cat_own("── exited ", maca_int_to_str(run_file(src, rest)), 2), " ──", 1), "\n", 1); } maca_sleep_ms(WatchPollMs); } return 0;  }
long newest_ms(MacaList files, long i, long best) { return ((i >= (files.len)) ? best : ({ long at = maca_modified_ms(((const char*)files.data[i])); newest_ms(files, (i + 1), ((at > best) ? at : best)); }));  }
long module_asked(const char* a) { return ((strcmp(a, "-m") == 0) || (strcmp(a, "--module") == 0));  }
long module_cmd(MacaList args) { return (((args.len) < 2) ? ({ maca_say(stderr, "-m: expected a module, e.g. `maca -m http.serve`", "\n", 0); 2; }) : module_spec(((const char*)args.data[1]), maca_list_slice(args, 2, (args.len))));  }
long module_spec(const char* spec, MacaList rest) { const char* why = spec_refusal(spec); return ((strcmp(why, "") != 0) ? ({ maca_say(stderr, maca_cat("-m: ", why), "\n", 1); 2; }) : module_found(module_entry(spec), rest));  }
const char* spec_refusal(const char* spec) { return (maca_starts_with(spec, "/") ? maca_cat_own(maca_cat("`", spec), "` is an absolute path, not a module", 1) : (empty_segment(maca_split(maca_replace(spec, ".", "/"), "/"), 0) ? maca_cat_own(maca_cat("`", spec), "` has an empty path segment", 1) : ""));  }
long empty_segment(MacaList ps, long i) { return ((i >= (ps.len)) ? 0 : ((strcmp(((const char*)ps.data[i]), "") == 0) ? 1 : empty_segment(ps, (i + 1))));  }
Entry module_entry(const char* spec) { long cut = last_of(maca_chars(spec), ".", (((int)strlen(spec)) - 1)); const char* head = ((cut <= 0) ? "" : maca_replace(maca_str_slice(spec, 0, cut), ".", "/")); const char* found = ((strcmp(head, "") == 0) ? "" : module_path(head)); return ((strcmp(found, "") != 0) ? (Entry){ .module = head, .named = maca_str_slice(spec, (cut + 1), ((int)strlen(spec))), .path = found } : whole_entry(maca_replace(spec, ".", "/")));  }
Entry whole_entry(const char* whole) { return (Entry){ .module = whole, .named = "", .path = module_path(whole) };  }
const char* module_path(const char* name) { return resolved("", name);  }
long module_found(Entry at, MacaList rest) { return ((strcmp(at.path, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("-m: no module `", at.module), "`; looked for ", 1), at.module, 1), ".maca under ", 1), "the package roots this directory reaches", 1), "\n", 1); 2; }) : module_entered(at, rest));  }
long module_entered(Entry at, MacaList rest) { MacaList items = parse_module(lex(maca_read_file(at.path)), 0, maca_listv(0)).items; const char* own = leaf_of(at.module); const char* want = ((strcmp(at.named, "") == 0) ? entry_fn(items, own) : at.named); long def = fn_at(items, want, 0); return ((strcmp(want, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("-m: `", at.module), "` defines no `main` and no `", 1), own, 1), "`; name the ", 1), maca_cat_own(maca_cat("function, as in `maca -m ", at.module), ".something`", 1), 3), "\n", 1); 2; }) : ((def < 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("-m: `", at.module), "` defines no function `", 1), want, 1), "`", 1), "\n", 1); 2; }) : module_calling(at, (*(Stmt*)items.data[def]), want, rest)));  }
const char* entry_fn(MacaList items, const char* own) { return ((fn_at(items, "main", 0) >= 0) ? "main" : ((fn_at(items, own, 0) >= 0) ? own : ""));  }
long fn_at(MacaList items, const char* want, long i) { return ((i >= (items.len)) ? (-1) : ((((*(Stmt*)items.data[i]).kind == SFn) && (strcmp((*(Stmt*)items.data[i]).name, want) == 0)) ? i : fn_at(items, want, (i + 1))));  }
long module_calling(Entry at, Stmt def, const char* want, MacaList rest) { const char* shape = call_shape(def); return ((strcmp(shape, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("-m: `", want), "` takes (", 1), param_list(def.params, 0, ""), 1), "); an entry ", 1), "point takes either nothing or one `str[]`", 1), "\n", 1); 2; }) : ((strcmp(want, "main") == 0) ? run_file(at.path, rest) : module_shimmed(at, maca_cat(want, shape), def.ret, rest)));  }
const char* call_shape(Stmt def) { return (((def.params.len) == 0) ? "()" : ((((def.params.len) == 1) && (strcmp((*(Expr*)def.params.data[0]).ty, "str[]") == 0)) ? "(args)" : ""));  }
const char* param_list(MacaList ps, long i, const char* acc) { return ((i >= (ps.len)) ? acc : ((strcmp(acc, "") == 0) ? param_list(ps, (i + 1), maca_cat_own(maca_cat_own(maca_cat("", (*(Expr*)ps.data[i]).text), ": ", 1), (*(Expr*)ps.data[i]).ty, 1)) : param_list(ps, (i + 1), maca_cat_own(acc, maca_cat_own(maca_cat_own(maca_cat(", ", (*(Expr*)ps.data[i]).text), ": ", 1), (*(Expr*)ps.data[i]).ty, 1), 2))));  }
long module_shimmed(Entry at, const char* call, const char* ret, MacaList rest) { const char* shim = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", EntryDir), "/maca1-entry-", 1), maca_int_to_str(maca_now_ms()), 3), ".maca", 1); maca_make_dir(EntryDir); maca_write_file(shim, entry_source(at.module, call, ret)); long code = run_file(shim, rest); maca_remove_file(shim); return code;  }
const char* entry_source(const char* module, const char* call, const char* ret) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("import ", module), "\n\nmain(args: str[]) -> int {\n", 1), entry_body(call, ret), 1), "}\n", 1);  }
const char* entry_body(const char* call, const char* ret) { return ((strcmp(ret, "int") == 0) ? maca_cat_own(maca_cat("    ", call), "\n", 1) : ((strcmp(ret, "bool") == 0) ? maca_cat_own(maca_cat("    ", call), " ? 0 : 1\n", 1) : maca_cat_own(maca_cat("    ", call), "\n    0\n", 1)));  }
const char* flag_after(MacaList args, const char* name) { long at = maca_list_index_of_str(args, name); return (((at < 0) || ((at + 1) >= (args.len))) ? "" : ((const char*)args.data[(at + 1)]));  }
long build_classes(const char* src, const char* out, const char* cp) { const char* name = jvm_class(src); const char* java = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", out), "/", 1), name, 1), ".java", 1); maca_make_dir(out); long errs = compile_file(maca_listv(4, (long)(src), (long)(java), (long)("jvm"), (long)(name))); return ((errs > 0) ? errs : ((!javac_here()) ? ({ maca_say(stderr, maca_cat_own(maca_cat("maca: no javac on PATH, so ", java), " was written but not compiled.", 1), "\n", 1); 2; }) : ({ maca_write_file(java, maca_cat_own(java_imports(src), maca_read_file(java), 2)); maca_exec("javac", javac_line(java, out, cp)); })));  }
const char* jvm_class(const char* src) { const char* stem = stem_of(src); return ((strcmp(stem, "") == 0) ? "Main" : maca_cat_own(maca_upper(maca_str_at(stem, 0)), maca_str_slice(stem, 1, ((int)strlen(stem))), 3));  }
const char* java_imports(const char* src) { return java_named(unit_of(src).seen, 0, "");  }
const char* java_named(MacaList files, long i, const char* acc) { return ((i >= (files.len)) ? ((strcmp(acc, "") == 0) ? "" : maca_cat(acc, "\n")) : ({ MacaList got = lex_all(maca_read_file(((const char*)files.data[i]))).tokens; java_named(files, (i + 1), maca_cat(acc, java_in(got, 0, ""))); }));  }
const char* java_in(MacaList ts, long i, const char* acc) { return (((*(Token*)ts.data[i]).kind == Eof) ? acc : (((((*(Token*)ts.data[i]).kind == KwImport) && (strcmp((*(Token*)ts.data[(i + 1)]).text, "java") == 0)) && ((*(Token*)ts.data[(i + 2)]).kind == TStr)) ? java_in(ts, (i + 3), maca_cat_own(acc, maca_cat_own(maca_cat("import ", (*(Token*)ts.data[(i + 2)]).text), ";\n", 1), 2)) : java_in(ts, (i + 1), acc)));  }
MacaList javac_line(const char* java, const char* out, const char* cp) { return ((strcmp(cp, "") == 0) ? maca_listv(3, (long)(java), (long)("-d"), (long)(out)) : maca_listv(5, (long)(java), (long)("-d"), (long)(out), (long)("-cp"), (long)(cp)));  }
long javac_here() { return on_path("javac");  }
long on_path(const char* cmd) { return (strcmp(maca_trim(maca_capture("sh", maca_listv(2, (long)("-c"), (long)(maca_cat("command -v ", cmd))))), "") != 0);  }
long build_rust(const char* src, const char* out) { const char* rs = maca_cat(out, ".rs"); long errs = compile_file(maca_listv(3, (long)(src), (long)(rs), (long)("rust"))); MacaList deps = manifest_keys(chain_of(src), "[rust-dependencies]", 0); return ((errs > 0) ? errs : (((deps.len) > 0) ? cargo_built(src, rs, out, deps) : ((!on_path("rustc")) ? ({ maca_say(stderr, maca_cat_own(maca_cat("maca: no rustc on PATH, so ", rs), " was written but not compiled.", 1), "\n", 1); 2; }) : maca_exec("rustc", maca_listv(6, (long)(rs), (long)("--edition"), (long)("2021"), (long)("-O"), (long)("-o"), (long)(out))))));  }
long cargo_built(const char* src, const char* rs, const char* out, MacaList deps) { MacaList chain = chain_of(src); const char* proj = maca_cat(out, "-cargo"); maca_make_dir(maca_cat(proj, "/src")); maca_write_file(maca_cat_own(maca_cat("", proj), "/Cargo.toml", 1), cargo_toml(chain, deps, manifest_keys(chain, "[rust-patch]", 0))); maca_write_file(maca_cat_own(maca_cat("", proj), "/src/main.rs", 1), maca_read_file(rs)); return ((!on_path("cargo")) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca: no cargo on PATH, so ", rs), " was written but its ", 1), "[rust-dependencies] could not be built.", 1), "\n", 1); 2; }) : cargo_built_at(proj, out));  }
long cargo_built_at(const char* proj, const char* out) { long code = maca_exec("cargo", maca_listv(5, (long)("build"), (long)("--release"), (long)("--quiet"), (long)("--manifest-path"), (long)(maca_cat_own(maca_cat("", proj), "/Cargo.toml", 1)))); return ((code != 0) ? code : maca_exec("cp", maca_listv(2, (long)(maca_cat_own(maca_cat_own(maca_cat("", proj), "/target/release/", 1), CargoName, 1)), (long)(out))));  }
const char* cargo_toml(MacaList chain, MacaList deps, MacaList patch) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("[package]\nname = \"", CargoName), "\"\nversion = \"0.0.0\"\n", 1), maca_cat_own(maca_cat("edition = \"2021\"\n\n[[bin]]\nname = \"", CargoName), "\"\n", 1), 3), "path = \"src/main.rs\"\n\n[dependencies]\n", 1), cargo_entries(chain, "[rust-dependencies]", deps, 0, ""), 1), cargo_patch(chain, patch), 1);  }
const char* cargo_patch(MacaList chain, MacaList patch) { return (((patch.len) == 0) ? "" : maca_cat("\n[patch.crates-io]\n", cargo_entries(chain, "[rust-patch]", patch, 0, "")));  }
const char* cargo_entries(MacaList chain, const char* table, MacaList ks, long i, const char* acc) { return ((i >= (ks.len)) ? acc : ({ const char* rhs = cargo_value(chain_value(chain, 0, table, ((const char*)ks.data[i]))); cargo_entries(chain, table, ks, (i + 1), maca_cat_own(acc, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ((const char*)ks.data[i])), " = ", 1), rhs, 1), "\n", 1), 2)); }));  }
const char* cargo_value(const char* v) { return (maca_starts_with(v, "{") ? v : maca_cat_own(maca_cat("\"", v), "\"", 1));  }
MacaList manifest_keys(MacaList chain, const char* table, long i) { return ((i >= (chain.len)) ? maca_listv(0) : ({ MacaList here = table_keys(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)), table); (((here.len) > 0) ? here : manifest_keys(chain, table, (i + 1))); }));  }
long tooled(const char* name) { return (((strcmp(name, "lint") == 0) || (strcmp(name, "doc") == 0)) || (strcmp(name, "bindgen") == 0));  }
const char* tool_path(const char* name) { return ((strcmp(name, "lint") == 0) ? "apps/lint/lint.maca" : ((strcmp(name, "doc") == 0) ? "apps/macadoc/macadoc.maca" : "apps/bindgen/bindgen.maca"));  }
long build_out(const char* src, const char* out, const char* target, const char* mcu, const char* cp) { return ((((strcmp(target, "") == 0) || (strcmp(target, "native") == 0)) || (strcmp(target, "c") == 0)) ? build_binary(src, out) : ((strcmp(target, "embedded") == 0) ? build_firmware(src, out, mcu) : ((strcmp(target, "wasm") == 0) ? build_wasm(src, out) : ((strcmp(target, "js") == 0) ? build_page(src, out) : ((strcmp(target, "tauri") == 0) ? build_tauri(src, out) : ((strcmp(target, "rust") == 0) ? build_rust(src, out) : ((strcmp(target, "nix") == 0) ? compile_file(maca_listv(3, (long)(src), (long)(out), (long)(target))) : ((strcmp(target, "jvm") == 0) ? build_classes(src, out, cp) : ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca: no target `", target), "`. Try native, rust, js, nix, ", 1), "tauri, wasm, embedded or jvm.", 1), "\n", 1); 2; })))))))));  }
const char* default_out(const char* src, const char* target) { return ((strcmp(target, "js") == 0) ? maca_cat(stem_of(src), "-web") : ((strcmp(target, "tauri") == 0) ? maca_cat(stem_of(src), "-tauri") : stem_of(src)));  }
MacaList page_keys() { return maca_listv(3, (long)("title"), (long)("lang"), (long)("description"));  }
const char* stray_page_key(MacaList chain, long i) { return ((i >= (chain.len)) ? "" : ({ const char* here = stray_of(table_keys(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)), "[page]"), page_keys(), 0); ((strcmp(here, "") != 0) ? here : stray_page_key(chain, (i + 1))); }));  }
const char* page_setting(MacaList chain, const char* key, const char* dflt) { const char* got = chain_value(chain, 0, "[page]", key); return ((strcmp(got, "") == 0) ? dflt : got);  }
long build_page(const char* src, const char* dir) { MacaList chain = chain_of(src); const char* stray = stray_page_key(chain, 0); return ((strcmp(stray, "") != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca.toml [page]: unknown key `", stray), "` (known: ", 1), maca_list_join(page_keys(), ", "), 3), ")", 1), "\n", 1); 2; }) : ({ maca_make_dir(dir); long errs = compile_file(maca_listv(3, (long)(src), (long)(maca_cat_own(maca_cat("", dir), "/app.js", 1)), (long)("js"))); ((errs > 0) ? errs : page_written(src, dir, chain)); }));  }
long page_written(const char* src, const char* dir, MacaList chain) { MacaList assets = page_assets(unit_of(src).seen, 0, maca_listv(0)); MacaList found = resolved_assets(src, assets, 0, maca_listv(0)); MacaList bad = asset_errors(found, 0, maca_listv(0)); return (((bad.len) > 0) ? report_all("asset", bad, 0) : ({ const char* js = maca_read_file(maca_cat_own(maca_cat("", dir), "/app.js", 1)); maca_write_file(maca_cat_own(maca_cat("", dir), "/app.css", 1), ""); maca_write_file(maca_cat_own(maca_cat("", dir), "/index.html", 1), page_html(src, chain, assets, found, js)); 0; }));  }
MacaList page_assets(MacaList files, long i, MacaList acc) { return ((i >= (files.len)) ? acc : ({ const char* text = maca_read_file(((const char*)files.data[i])); page_assets(files, (i + 1), maca_list_cat(acc, assets_in(text, lex_all(text).tokens, 0, maca_listv(0)))); }));  }
MacaList assets_in(const char* src, MacaList ts, long i, MacaList acc) { return (((*(Token*)ts.data[i]).kind == Eof) ? acc : (((*(Token*)ts.data[i]).kind == KwImport) ? assets_in(src, ts, import_end(ts, (i + 1)), maca_list_cat(acc, asset_at(src, ts, (i + 1)))) : assets_in(src, ts, (i + 1), acc)));  }
MacaList asset_at(const char* src, MacaList ts, long i) { long at = (((*(Token*)ts.data[i]).kind == LBrace) ? (selection_end(ts, (i + 1)) + 2) : i); return (((*(Token*)ts.data[i]).kind == TStr) ? maca_listv(1, maca_box(sizeof(PageAsset), (PageAsset[]){ (PageAsset){ .kind = asset_kind((*(Token*)ts.data[i]).text), .spec = (*(Token*)ts.data[i]).text, .names = "", .text = "" } })) : ((((*(Token*)ts.data[i]).kind == LBrace) && ((*(Token*)ts.data[at]).kind == TStr)) ? maca_listv(1, maca_box(sizeof(PageAsset), (PageAsset[]){ (PageAsset){ .kind = "script", .spec = (*(Token*)ts.data[at]).text, .names = maca_list_join(asked_names(ts, (i + 1), maca_listv(0)), " "), .text = "" } })) : (tagged_block(src, ts, i) ? maca_listv(1, maca_box(sizeof(PageAsset), (PageAsset[]){ (PageAsset){ .kind = ((strcmp((*(Token*)ts.data[i]).text, "css") == 0) ? "stylesheet" : "script"), .spec = "", .names = "", .text = raw_block(src, (*(Token*)ts.data[(i + 1)]).pos) } })) : maca_listv(0))));  }
long tagged_block(const char* src, MacaList ts, long i) { return ((((strcmp((*(Token*)ts.data[i]).text, "css") == 0) || (strcmp((*(Token*)ts.data[i]).text, "js") == 0)) && ((*(Token*)ts.data[(i + 1)]).kind == TStr)) && (strcmp(maca_str_slice(src, (*(Token*)ts.data[(i + 1)]).pos, ((*(Token*)ts.data[(i + 1)]).pos + 3)), "\"\"\"") == 0));  }
const char* raw_block(const char* src, long at) { const char* rest = maca_str_slice(src, (at + 3), ((int)strlen(src))); long ends = maca_str_index_of(rest, "\"\"\""); return ((ends < 0) ? rest : maca_str_slice(rest, 0, ends));  }
MacaList asked_names(MacaList ts, long i, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == RBrace) || ((*(Token*)ts.data[i]).kind == Eof)) ? acc : (((*(Token*)ts.data[i]).kind == TIdent) ? asked_names(ts, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Token*)ts.data[i]).text)))) : asked_names(ts, (i + 1), acc)));  }
const char* asset_kind(const char* spec) { const char* path = ((const char*)maca_split(((const char*)maca_split(spec, "?").data[0]), "#").data[0]); return (maca_ends_with(path, ".css") ? "stylesheet" : (((maca_ends_with(path, ".js") || maca_ends_with(path, ".mjs")) || maca_ends_with(path, ".cjs")) ? "script" : (maca_ends_with(path, ".wasm") ? "wasm" : (maca_starts_with(path, NpmPrefix) ? "package" : ""))));  }
MacaList resolved_assets(const char* src, MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : resolved_assets(src, xs, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Found), (Found[]){ resolved_asset(src, (*(PageAsset*)xs.data[i])) })))));  }
MacaList asset_errors(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((strcmp((*(Found*)xs.data[i]).why, "") == 0) ? asset_errors(xs, (i + 1), acc) : asset_errors(xs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Found*)xs.data[i]).why))))));  }
Found resolved_asset(const char* src, PageAsset a) { const char* file = maca_cat(dir_of(src), a.spec); return ((strcmp(a.text, "") != 0) ? (Found){ .kind = a.kind, .file = "", .why = "" } : (maca_starts_with(a.spec, NpmPrefix) ? package_found(src, maca_str_slice(a.spec, ((int)strlen(NpmPrefix)), ((int)strlen(a.spec))), a.kind) : ((strcmp(a.kind, "") == 0) ? (Found){ .kind = "", .file = "", .why = "" } : (maca_file_exists(file) ? (Found){ .kind = a.kind, .file = file, .why = "" } : (Found){ .kind = a.kind, .file = "", .why = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("import \"", a.spec), "\": ", 1), file, 1), " is not there", 1) }))));  }
Found package_found(const char* src, const char* spec, const char* kind) { const char* name = package_name(spec); const char* sub = package_sub(spec); const char* dir = installed_at(dir_of(src), name); const char* said = maca_cat_own(maca_cat_own(maca_cat("import \"", NpmPrefix), spec, 1), "\": ", 1); return ((strcmp(name, "") == 0) ? (Found){ .kind = kind, .file = "", .why = maca_cat(said, "no package named") } : ((strcmp(dir, "") == 0) ? (Found){ .kind = kind, .file = "", .why = maca_cat_own(maca_cat_own(said, maca_cat_own(maca_cat("`", name), "` is not installed; run ", 1), 2), maca_cat_own(maca_cat_own(maca_cat("`maca add ", NpmPrefix), name, 1), "`", 1), 3) } : ((strcmp(sub, "") == 0) ? package_entry(dir, name, said) : (maca_file_exists(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), sub, 1)) ? (Found){ .kind = kind, .file = maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), sub, 1), .why = "" } : (Found){ .kind = kind, .file = "", .why = maca_cat_own(said, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", name), "` has no `", 1), sub, 1), "`", 1), 2) }))));  }
Found package_entry(const char* dir, const char* name, const char* said) { const char* entry = first_entry(maca_read_file(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), PackageJson, 1)), entry_keys(), 0); const char* file = maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), (maca_starts_with(entry, "./") ? maca_str_slice(entry, 2, ((int)strlen(entry))) : entry), 1); return ((strcmp(entry, "") == 0) ? (Found){ .kind = "", .file = "", .why = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(said, maca_cat_own(maca_cat("`", name), "` states no entry point (", 1), 2), maca_list_join(entry_keys(), "/"), 3), " names no file of a kind a ", 1), "page can carry); name the file, as in `import ", 1), maca_cat_own(maca_cat_own(maca_cat("\"", NpmPrefix), name, 1), "/dist/…\"`", 1), 3) } : ((!maca_file_exists(file)) ? (Found){ .kind = "", .file = "", .why = maca_cat_own(maca_cat_own(said, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", name), "` states \"", 1), entry, 1), "\", which is not ", 1), 2), "there", 1) } : (Found){ .kind = asset_kind(entry), .file = file, .why = "" }));  }
MacaList entry_keys() { return maca_listv(4, (long)("style"), (long)("browser"), (long)("module"), (long)("main"));  }
const char* first_entry(const char* text, MacaList keys, long i) { return ((i >= (keys.len)) ? "" : ({ const char* got = pkg_entry(text, ((const char*)keys.data[i])); ((strcmp(asset_kind(got), "") != 0) ? got : first_entry(text, keys, (i + 1))); }));  }
const char* pkg_entry(const char* text, const char* key) { MacaList cs = maca_chars(text); long at = maca_str_index_of(text, maca_cat_own(maca_cat("\"", key), "\"", 1)); long from = ((at < 0) ? (-1) : next_quote(cs, ((at + ((int)strlen(key))) + 2))); return ((from < 0) ? "" : quoted_upto(cs, (from + 1), maca_listv(0)));  }
long next_quote(MacaList cs, long i) { return ((((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), ",") == 0)) || (strcmp(((const char*)cs.data[i]), "}") == 0)) ? (-1) : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? i : next_quote(cs, (i + 1))));  }
const char* quoted_upto(MacaList cs, long i, MacaList acc) { return (((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), "\"") == 0)) ? maca_list_join(acc, "") : quoted_upto(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)cs.data[i]))))));  }
const char* installed_at(const char* dir, const char* name) { const char* at = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", dir), DepsDir, 1), "/", 1), name, 1); return (maca_is_dir(at) ? at : ((strcmp(dir, "") == 0) ? "" : installed_at(parent_of(dir), name)));  }
const char* package_name(const char* spec) { return ((const char*)maca_split(unscoped(spec), "/").data[0]);  }
const char* package_sub(const char* spec) { const char* rest = unscoped(spec); long at = maca_str_index_of(rest, "/"); return ((at < 0) ? "" : maca_str_slice(rest, (at + 1), ((int)strlen(rest))));  }
const char* unscoped(const char* spec) { long at = maca_str_index_of(spec, "/"); return ((!maca_starts_with(spec, "@")) ? spec : ((at < 0) ? "" : maca_str_slice(spec, (at + 1), ((int)strlen(spec)))));  }
const char* page_html(const char* src, MacaList chain, MacaList assets, MacaList found, const char* js) { const char* title = html_text(page_setting(chain, "title", stem_of(src))); const char* lang = page_setting(chain, "lang", ""); const char* desc = page_setting(chain, "description", ""); const char* opened = ((strcmp(lang, "") == 0) ? "<html>" : maca_cat_own(maca_cat("<html lang=\"", html_text(lang)), "\">", 1)); const char* meta = ((strcmp(desc, "") == 0) ? "" : maca_cat_own(maca_cat("<meta name=\"description\" content=\"", html_text(desc)), "\">\n", 1)); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("<!doctype html>\n", opened), "\n<head>\n<meta charset=\"utf-8\">\n", 1), "<meta name=\"viewport\" content=\"width=device-width,", 1), maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("initial-scale=1\">\n", meta), "<title>", 1), title, 1), "</title>\n", 1), 3), inlined(assets, found, 0, maca_listv(0), 1), 1), "<style>\n\n</style>\n</head>\n<body>\n<div id=\"app\"></div>\n", 1), inlined(assets, found, 0, maca_listv(0), 0), 1), maca_cat_own(maca_cat("<script>\n", js), "\n</script>\n</body>\n</html>\n", 1), 3);  }
const char* inlined(MacaList assets, MacaList found, long i, MacaList acc, long head) { return ((i >= (assets.len)) ? maca_list_join(acc, "") : ((head == (strcmp((*(Found*)found.data[i]).kind, "stylesheet") == 0)) ? inlined(assets, found, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(one_inlined((*(PageAsset*)assets.data[i]), (*(Found*)found.data[i]))))), head) : inlined(assets, found, (i + 1), acc, head)));  }
const char* one_inlined(PageAsset a, Found f) { const char* body = close_safe(((strcmp(a.text, "") != 0) ? a.text : maca_read_file(f.file))); return ((strcmp(f.kind, "stylesheet") == 0) ? maca_cat_own(maca_cat("<style>\n", body), "\n</style>\n", 1) : ((strcmp(f.kind, "wasm") == 0) ? maca_cat_own(maca_cat("<script id=\"wasm-b64\" type=\"application/octet-stream\">", base64_of(f.file)), "</script>\n", 1) : (((strcmp(f.kind, "script") == 0) && (strcmp(a.names, "") != 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(script_open(a, f), body), "\n</script>\n<script>\n", 1), named_bindings(maca_split(a.names, " "), 0, maca_listv(0)), 1), "</script>\n", 1) : ((strcmp(f.kind, "script") == 0) ? maca_cat_own(maca_cat(script_open(a, f), body), "\n</script>\n", 1) : ""))));  }
const char* script_open(PageAsset a, Found f) { return (((strcmp(a.text, "") == 0) && es_module(f.file)) ? "<script type=\"module\">\n" : "<script>\n");  }
long es_module(const char* path) { return (maca_ends_with(path, ".mjs") ? 1 : (maca_ends_with(path, ".cjs") ? 0 : module_package(dir_of(path))));  }
long module_package(const char* dir) { const char* text = maca_read_file(maca_cat_own(maca_cat("", dir), PackageJson, 1)); return ((strcmp(text, "") != 0) ? (strcmp(pkg_entry(text, "type"), "module") == 0) : ((strcmp(dir, "") == 0) ? 0 : module_package(parent_of(dir))));  }
const char* close_safe(const char* text) { MacaList parts = maca_split(text, "</"); return rejoined(parts, 1, maca_listv(1, (long)(((const char*)parts.data[0]))));  }
const char* rejoined(MacaList parts, long i, MacaList acc) { return ((i >= (parts.len)) ? maca_list_join(acc, "") : ({ const char* lead = maca_lower(maca_str_slice(((const char*)parts.data[i]), 0, 6)); const char* mark = ((maca_starts_with(lead, "style") || maca_starts_with(lead, "script")) ? "<\\/" : "</"); rejoined(parts, (i + 1), maca_list_cat(acc, maca_listv(2, (long)(mark), (long)(((const char*)parts.data[i]))))); }));  }
const char* html_text(const char* s) { return maca_replace(maca_replace(maca_replace(maca_replace(maca_replace(s, "&", "&amp;"), "<", "&lt;"), ">", "&gt;"), "\"", "&quot;"), "'", "&#39;");  }
const char* named_bindings(MacaList names, long i, MacaList acc) { return ((i >= (names.len)) ? maca_list_join(acc, "") : ({ const char* bound = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("const ", ((const char*)names.data[i])), " = ", 1), first_there(spellings(((const char*)names.data[i])), 0, maca_listv(0)), 1), ";\n", 1); named_bindings(names, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(bound)))); }));  }
const char* first_there(MacaList ss, long i, MacaList acc) { return ((i >= (ss.len)) ? maca_list_join(acc, " ?? ") : first_there(ss, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(maca_cat_own(maca_cat("globalThis[\"", ((const char*)ss.data[i])), "\"]", 1))))));  }
MacaList spellings(const char* name) { const char* camel = camel_of(maca_chars(name), 0, maca_listv(0), 0); const char* kebab = maca_replace(name, "_", "-"); MacaList held = ((strcmp(camel, name) == 0) ? maca_listv(1, (long)(name)) : maca_listv(2, (long)(name), (long)(camel))); return ((strcmp(kebab, name) == 0) ? held : maca_list_cat(held, maca_listv(1, (long)(kebab))));  }
const char* camel_of(MacaList cs, long i, MacaList acc, long up) { return ((i >= (cs.len)) ? maca_list_join(acc, "") : ((strcmp(((const char*)cs.data[i]), "_") == 0) ? camel_of(cs, (i + 1), acc, 1) : (up ? camel_of(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(maca_upper(((const char*)cs.data[i]))))), 0) : camel_of(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)cs.data[i])))), 0))));  }
const char* base64_of(const char* file) { return maca_trim(maca_capture("sh", maca_listv(3, (long)("-c"), (long)("base64 \"$0\" | tr -d '\\n'"), (long)(file))));  }
long build_wasm(const char* src, const char* out) { const char* wasm = (maca_ends_with(out, ".wasm") ? out : maca_cat(out, ".wasm")); const char* cfile = maca_cat(wasm, ".c"); long errs = compile_file(maca_listv(2, (long)(src), (long)(cfile))); return ((errs > 0) ? errs : ((maca_str_index_of(maca_read_file(cfile), "\n#include <setjmp.h>\n") >= 0) ? ({ maca_say(stderr, maca_cat_own("maca: the wasm target cannot carry `try` yet, because ", maca_cat_own(maca_cat("wasi-libc has no setjmp; ", cfile), " is written", 1), 2), "\n", 1); 1; }) : ({ long built = maca_exec("zig", maca_listv(7, (long)("cc"), (long)("-target"), (long)("wasm32-wasi"), (long)("-Os"), (long)(cfile), (long)("-o"), (long)(wasm))); ((built != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat("maca: zig did not link the wasm; ", cfile), " is written", 1), "\n", 1); built; }) : ({ maca_say(stdout, maca_cat("built ", wasm), "\n", 1); 0; })); })));  }
long build_firmware(const char* src, const char* dir, const char* mcu) { Mcu m = emb_mcu(mcu); return ((strcmp(m.name, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca: no --mcu `", mcu), "`. Try cortex-m0, cortex-m3, cortex-m4 ", 1), "or riscv32.", 1), "\n", 1); 2; }) : ({ maca_make_dir(dir); long errs = compile_file(maca_listv(3, (long)(src), (long)(maca_cat_own(maca_cat("", dir), "/firmware.c", 1)), (long)("embedded"))); ((errs > 0) ? errs : ({ maca_write_file(maca_cat_own(maca_cat("", dir), "/link.ld", 1), emb_linker_script(m)); cross_compile(m, dir); })); }));  }
long cross_compile(Mcu m, const char* dir) { long built = maca_exec("clang", maca_listv(13, (long)(maca_cat("--target=", m.triple)), (long)(maca_cat("-mcpu=", m.cpu)), (long)("-ffreestanding"), (long)("-nostdlib"), (long)("-Os"), (long)("-ffunction-sections"), (long)("-fdata-sections"), (long)("-fuse-ld=lld"), (long)(maca_cat_own(maca_cat("-Wl,-T,", dir), "/link.ld", 1)), (long)("-Wl,--gc-sections"), (long)("-o"), (long)(maca_cat_own(maca_cat("", dir), "/firmware.elf", 1)), (long)(maca_cat_own(maca_cat("", dir), "/firmware.c", 1)))); return ((built != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca: clang did not link the firmware; ", dir), "/firmware.c and ", 1), maca_cat_own(maca_cat("", dir), "/link.ld are written", 1), 3), "\n", 1); built; }) : ({ maca_exec("llvm-objcopy", maca_listv(4, (long)("-O"), (long)("binary"), (long)(maca_cat_own(maca_cat("", dir), "/firmware.elf", 1)), (long)(maca_cat_own(maca_cat("", dir), "/firmware.bin", 1)))); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("built firmware for ", m.name), " → ", 1), dir, 1), "/firmware.elf", 1), "\n", 1); 0; }));  }
long build_tauri(const char* src, const char* dir) { long built = build_page(src, maca_cat_own(maca_cat("", dir), "/dist", 1)); return ((built > 0) ? built : tauri_shell(src, dir));  }
long tauri_shell(const char* src, const char* dir) { const char* st = maca_cat_own(maca_cat("", dir), "/src-tauri", 1); const char* index_mc = maca_cat_own(maca_cat("", dir), "/dist/index.html", 1); const char* name = crate_ident(stem_of(src)); maca_write_file(maca_cat_own(maca_cat("", dir), "/dist/bridge.js", 1), tauri_bridge()); maca_write_file(index_mc, maca_replace(maca_read_file(index_mc), "</body>", "<script src=\"bridge.js\"></script>\n</body>")); maca_make_dir(maca_cat_own(maca_cat("", st), "/src", 1)); maca_write_file(maca_cat_own(maca_cat("", st), "/Cargo.toml", 1), tauri_cargo(name)); maca_write_file(maca_cat_own(maca_cat("", st), "/build.rs", 1), "fn main() {\n    tauri_build::build()\n}\n"); maca_write_file(maca_cat_own(maca_cat("", st), "/tauri.conf.json", 1), tauri_conf(name, page_setting(chain_of(src), "title", stem_of(src)))); maca_write_file(maca_cat_own(maca_cat("", st), "/src/main.rs", 1), tauri_main_rs()); long built = tauri_backend(src, st); return ((built > 0) ? built : tauri_scaffolded(src, dir, st));  }
long tauri_backend(const char* src, const char* st) { const char* backend = maca_cat(dir_of(src), "backend.maca"); return (maca_file_exists(backend) ? ({ maca_make_dir(maca_cat_own(maca_cat("", st), "/bin", 1)); build_binary(backend, maca_cat_own(maca_cat("", st), "/bin/backend", 1)); }) : 0);  }
long tauri_scaffolded(const char* src, const char* dir, const char* st) { const char* note = (maca_file_exists(maca_cat(dir_of(src), "backend.maca")) ? "" : " (no backend.maca beside it, so `maca_run` has nothing to run)"); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("scaffolded ", dir), note, 1), "\n  cd ", 1), st, 1), " && cargo tauri build   ", 1), "# needs the Tauri CLI and a system webview", 1), "\n", 1); return 0;  }
const char* tauri_bridge() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("globalThis.macaInvoke = async (arg) => {\n", "  const t = globalThis.__TAURI__;\n"), "  if (t && t.core && t.core.invoke) {\n", 1), "    return t.core.invoke('maca_run', { arg: String(arg) });\n", 1), "  }\n", 1), "  if (t && t.invoke) {\n", 1), "    return t.invoke('maca_run', { arg: String(arg) });\n", 1), "  }\n", 1), "  return '(no tauri runtime)';\n", 1), "};\n", 1);  }
const char* tauri_cargo(const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("[package]\nname = \"", name), "\"\nversion = \"0.1.0\"\n", 1), "edition = \"2021\"\n\n[build-dependencies]\n", 1), "tauri-build = { version = \"2\", features = [] }\n\n", 1), "[dependencies]\ntauri = { version = \"2\", features = [] }\n\n", 1), maca_cat_own(maca_cat("[[bin]]\nname = \"", name), "\"\npath = \"src/main.rs\"\n", 1), 3);  }
const char* tauri_conf(const char* name, const char* title) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{\n  \"productName\": \"", title), "\",\n  \"version\": \"0.1.0\",\n", 1), maca_cat_own(maca_cat("  \"identifier\": \"dev.maca.", name), "\",\n", 1), 3), "  \"build\": { \"frontendDist\": \"../dist\" },\n", 1), maca_cat_own(maca_cat("  \"app\": {\n    \"windows\": [{ \"title\": \"", title), "\", ", 1), 3), "\"width\": 900, \"height\": 640, \"resizable\": true }],\n", 1), "    \"security\": { \"csp\": null }\n  },\n", 1), "  \"bundle\": { \"active\": true, \"targets\": \"all\" }\n}\n", 1);  }
const char* tauri_main_rs() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]\n", "use std::process::Command;\n\n#[tauri::command]\n"), "fn maca_run(arg: String) -> String {\n", 1), "    let exe = std::env::current_exe()\n", 1), "        .ok()\n", 1), "        .and_then(|p| p.parent()", 1), ".map(|d| d.join(\"bin\").join(\"backend\")))\n", 1), "        .unwrap_or_else(|| ", 1), "std::path::PathBuf::from(\"backend\"));\n", 1), "    match Command::new(&exe).arg(&arg).output() {\n", 1), "        Ok(o) => ", 1), "String::from_utf8_lossy(&o.stdout).trim().to_string(),\n", 1), "        Err(e) => format!(\"error: {e}\"),\n", 1), "    }\n}\n\nfn main() {\n", 1), "    tauri::Builder::default()\n", 1), "        .invoke_handler(tauri::generate_handler![maca_run])\n", 1), "        .run(tauri::generate_context!())\n", 1), "        .expect(\"error while running tauri application\");\n}\n", 1);  }
const char* crate_ident(const char* stem) { const char* id = ident_chars(maca_chars(stem), 0, maca_listv(0)); return ((strcmp(id, "") == 0) ? "app" : ((isdigit((unsigned char)(maca_str_at(id, 0))[0]) != 0) ? maca_cat("_", id) : id));  }
const char* ident_chars(MacaList cs, long i, MacaList acc) { return ((i >= (cs.len)) ? maca_list_join(acc, "") : (((isalpha((unsigned char)(((const char*)cs.data[i]))[0]) != 0) || (isdigit((unsigned char)(((const char*)cs.data[i]))[0]) != 0)) ? ident_chars(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(maca_lower(((const char*)cs.data[i])))))) : ident_chars(cs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)("_"))))));  }
long build_binary(const char* src, const char* out) { const char* key = cache_key(src); const char* hit = ((strcmp(key, "") == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat("", cache_dir()), "/", 1), key, 1)); return ((((strcmp(hit, "") != 0) && maca_file_exists(hit)) && cache_take(hit, out)) ? 0 : ({ long built = build_fresh(src, out); if (((built == 0) && (strcmp(key, "") != 0))) { cache_store(key, out); } built; }));  }
const char* cache_dir() { const char* given = maca_env("MACA_CACHE"); const char* xdg = maca_env("XDG_CACHE_HOME"); const char* home = maca_env("HOME"); return ((strcmp(given, "") != 0) ? maca_cat_own(maca_cat("", given), "/build/bin", 1) : ((strcmp(xdg, "") != 0) ? maca_cat_own(maca_cat("", xdg), "/maca/build/bin", 1) : ((strcmp(home, "") != 0) ? maca_cat_own(maca_cat("", home), "/.cache/maca/build/bin", 1) : "/tmp/maca/build/bin")));  }
const char* cache_key(const char* src) { const char* off = maca_env("MACA_NO_CACHE"); return (((strcmp(off, "1") == 0) || (strcmp(off, "true") == 0)) ? "" : maca_trim(maca_capture("sh", maca_list_cat(maca_listv(4, (long)("-c"), (long)(cache_probe()), (long)("maca"), (long)(maca_cat_own("native ", maca_list_join(cc_flags(), " "), 2))), unit_of(src).seen))));  }
const char* cache_probe() { return maca_cat_own(maca_cat_own(maca_cat("command -v sha512sum >/dev/null || exit 0; ", "exe=$(stat -Lc %Y /proc/$PPID/exe 2>/dev/null); "), "[ -n \"$exe\" ] || exit 0; ( printf %s \"$exe $1\"; shift; ", 1), "sha512sum \"$@\" | cut -d\" \" -f1 ) | sha512sum | cut -d\" \" -f1", 1);  }
long cache_take(const char* from, const char* out) { return (maca_copy_bytes(from, out) && (maca_exec("chmod", maca_listv(2, (long)("755"), (long)(out))) == 0));  }
long cache_store(const char* key, const char* out) { const char* dir = cache_dir(); const char* tmp = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), key, 1), ".", 1), maca_int_to_str(maca_now_ms()), 3), ".tmp", 1); maca_make_dir(dir); return (maca_copy_bytes(out, tmp) && (maca_exec("mv", maca_listv(2, (long)(tmp), (long)(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), key, 1)))) == 0));  }
MacaList link_flags(const char* code) { long sq = (maca_str_index_of(code, "#include <sqlite3.h>") >= 0); long py = (maca_str_index_of(code, "#include <Python.h>") >= 0); return ((sq && py) ? maca_list_cat(maca_list_cat(cc_flags(), sqlite_flags()), python_flags()) : (sq ? maca_list_cat(cc_flags(), sqlite_flags()) : (py ? maca_list_cat(cc_flags(), python_flags()) : cc_flags())));  }
MacaList sqlite_flags() { const char* pkg = maca_trim(maca_capture("pkg-config", maca_listv(3, (long)("--cflags"), (long)("--libs"), (long)("sqlite3")))); const char* dev = ((strcmp(pkg, "") != 0) ? "" : nix_path("nixpkgs#sqlite.dev")); const char* lib = ((strcmp(dev, "") == 0) ? "" : nix_path("nixpkgs#sqlite.out")); return ((strcmp(pkg, "") != 0) ? maca_list_cat(nonempty(maca_split(pkg, " "), 0, maca_listv(0)), maca_listv(1, (long)("-lpthread"))) : ((strcmp(lib, "") != 0) ? maca_listv(5, (long)(maca_cat_own(maca_cat("-I", dev), "/include", 1)), (long)(maca_cat_own(maca_cat("-L", lib), "/lib", 1)), (long)("-lsqlite3"), (long)(maca_cat_own(maca_cat("-Wl,-rpath,", lib), "/lib", 1)), (long)("-lpthread")) : maca_listv(2, (long)("-lsqlite3"), (long)("-lpthread"))));  }
const char* nix_path(const char* attr) { return maca_trim(maca_capture("nix", maca_listv(4, (long)("build"), (long)("--no-link"), (long)("--print-out-paths"), (long)(attr))));  }
MacaList python_flags() { const char* inc = maca_trim(maca_capture("python3-config", maca_listv(1, (long)("--includes")))); const char* embed = maca_trim(maca_capture("python3-config", maca_listv(2, (long)("--ldflags"), (long)("--embed")))); const char* ld = ((strcmp(embed, "") != 0) ? embed : maca_trim(maca_capture("python3-config", maca_listv(1, (long)("--ldflags"))))); return nonempty(maca_list_cat(maca_split(inc, " "), maca_split(ld, " ")), 0, maca_listv(0));  }
long build_fresh(const char* src, const char* out) { const char* cfile = maca_cat(out, ".c"); long errs = compile_file(maca_listv(2, (long)(src), (long)(cfile))); return ((errs > 0) ? errs : maca_exec("cc", maca_list_cat(maca_list_cat(maca_listv(1, (long)(cfile)), link_flags(maca_read_file(cfile))), maca_listv(2, (long)("-o"), (long)(out)))));  }
MacaList cc_flags() { return nonempty(maca_split(chain_value(here_chain(), 0, "[build]", "cflags"), " "), 0, maca_listv(0));  }
long build_cmd(MacaList args) { const char* named = build_src(args, 1, ""); const char* src = ((strcmp(named, "") != 0) ? named : declared_bin("build", flag_after(args, "--bin"))); MacaList chain = ((strcmp(src, "") == 0) ? maca_listv(0) : chain_of(src)); const char* stray = stray_build_key(chain, 0); return ((strcmp(src, "") == 0) ? 2 : ((strcmp(stray, "") != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca.toml [build]: unknown key `", stray), "` (known: ", 1), maca_list_join(build_keys(), ", "), 3), ")", 1), "\n", 1); 2; }) : (((strcmp(named, "") != 0) && (!workspace_ok(chain))) ? 1 : ({ const char* out = build_setting(args, chain, "-o", "out"); const char* target = sniffed(src, build_setting(args, chain, "--target", "target")); build_out(src, ((strcmp(out, "") == 0) ? default_out(src, target) : out), target, build_setting(args, chain, "--mcu", "mcu"), declared_cp(args, chain)); }))));  }
const char* declared_cp(MacaList args, MacaList chain) { const char* short_mc = flag_after(args, "--cp"); return ((strcmp(short_mc, "") != 0) ? short_mc : build_setting(args, chain, "--classpath", "classpath"));  }
const char* build_setting(MacaList args, MacaList chain, const char* flag, const char* key) { const char* given = flag_after(args, flag); return ((strcmp(given, "") != 0) ? given : ((strcmp(key, "out") == 0) ? declared_out(chain, 0) : chain_value(chain, 0, "[build]", key)));  }
const char* declared_out(MacaList chain, long i) { return ((i >= (chain.len)) ? "" : ({ const char* got = toml_value(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)), "[build]", "out"); ((strcmp(got, "") != 0) ? maca_cat(((const char*)chain.data[i]), got) : declared_out(chain, (i + 1))); }));  }
MacaList build_keys() { return maca_listv(6, (long)("target"), (long)("out"), (long)("mcu"), (long)("classpath"), (long)("cflags"), (long)("bin"));  }
const char* stray_build_key(MacaList chain, long i) { return ((i >= (chain.len)) ? "" : ({ const char* here = stray_of(table_keys(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)), "[build]"), build_keys(), 0); ((strcmp(here, "") != 0) ? here : stray_build_key(chain, (i + 1))); }));  }
const char* stray_of(MacaList keys, MacaList known, long i) { return ((i >= (keys.len)) ? "" : ((maca_list_index_of_str(known, ((const char*)keys.data[i])) >= 0) ? stray_of(keys, known, (i + 1)) : ((const char*)keys.data[i])));  }
const char* build_src(MacaList args, long i, const char* held) { return ((i >= (args.len)) ? held : (takes_value(((const char*)args.data[i])) ? build_src(args, (i + 2), held) : build_src(args, (i + 1), ((const char*)args.data[i]))));  }
long takes_value(const char* a) { return ((((((strcmp(a, "-o") == 0) || (strcmp(a, "--target") == 0)) || (strcmp(a, "--bin") == 0)) || (strcmp(a, "--mcu") == 0)) || (strcmp(a, "--cp") == 0)) || (strcmp(a, "--classpath") == 0));  }
const char* sniffed(const char* src, const char* asked) { const char* found = ((strcmp(asked, "") != 0) ? "" : detected_target(src)); if ((strcmp(found, "") != 0)) { maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("note: ", why_target(found)), "; building --target ", 1), found, 1), " ", 1), "(pass --target to override)", 1), "\n", 1); } return ((strcmp(found, "") != 0) ? found : asked);  }
const char* detected_target(const char* src) { MacaList ts = lex(maca_read_file(src)); return (imports_nixpkgs(imports_in(ts, 0, maca_listv(0)), 0) ? "nix" : (answers_element(ts, 0) ? "js" : ""));  }
const char* why_target(const char* found) { return ((strcmp(found, "nix") == 0) ? "source imports nixpkgs (config mode)" : "a view returns Element (reactive-UI mode)");  }
long imports_nixpkgs(MacaList paths, long i) { return ((i >= (paths.len)) ? 0 : (((strcmp(((const char*)paths.data[i]), "nixpkgs") == 0) || maca_ends_with(((const char*)paths.data[i]), "/nixpkgs")) ? 1 : imports_nixpkgs(paths, (i + 1))));  }
long answers_element(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == Eof) ? 0 : ((((*(Token*)ts.data[i]).kind == Arrow) && (strcmp((*(Token*)ts.data[(i + 1)]).text, "Element") == 0)) ? 1 : answers_element(ts, (i + 1))));  }
long run_cmd(MacaList args) { const char* named = first_named(args); return (((strcmp(named, "") != 0) && (!workspace_ok(chain_of(named)))) ? 1 : ((strcmp(named, "") != 0) ? run_file(named, maca_list_slice(args, 2, (args.len))) : ({ const char* src = declared_bin("run", flag_after(args, "--bin")); ((strcmp(src, "") == 0) ? 2 : run_file(src, maca_listv(0))); })));  }
long test_cmd(MacaList args) { const char* named = first_named(args); return ((strcmp(named, "") != 0) ? test_file(named) : test_package(here_chain()));  }
long test_package(MacaList chain) { return (((chain.len) == 0) ? ({ maca_say(stderr, maca_cat("test: no maca.toml here, so there is no package to test; ", "name a .maca file"), "\n", 1); 2; }) : ({ const char* dir = maca_cat(((const char*)chain.data[0]), tests_dir(chain)); MacaList suites = suite_files(dir, maca_list_dir(dir), 0, maca_listv(0)); (((suites.len) == 0) ? no_suites(dir) : ({ maca_say(stdout, maca_cat_own(package_heading(chain), maca_cat_own(maca_cat_own(": ", maca_int_to_str((suites.len)), 2), " suites", 1), 2), "\n", 1); ran_suites(suites, 0, 0); })); }));  }
const char* package_heading(MacaList chain) { const char* got = chain_value(chain, 0, "[package]", "version"); return ((strcmp(got, "") == 0) ? package_of(chain) : maca_cat_own(maca_cat(package_of(chain), " "), got, 1));  }
const char* tests_dir(MacaList chain) { const char* named = chain_value(chain, 0, "[package]", "tests"); return ((strcmp(named, "") == 0) ? "tests" : named);  }
MacaList suite_files(const char* dir, MacaList names, long i, MacaList acc) { return ((i >= (names.len)) ? acc : (maca_ends_with(((const char*)names.data[i]), ".maca") ? suite_files(dir, names, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(maca_cat_own(maca_cat(dir, "/"), ((const char*)names.data[i]), 1))))) : suite_files(dir, names, (i + 1), acc)));  }
long no_suites(const char* dir) { maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("test: ", dir), " holds no .maca suite; name a file, or set ", 1), "[package] tests", 1), "\n", 1); return 2;  }
long ran_suites(MacaList suites, long i, long failed) { long n = (suites.len); return ((i >= n) ? ({ long ok = (n - failed); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("", maca_int_to_str(ok), 2), " of ", 1), maca_int_to_str(n), 3), " suites passed", 1), "\n", 1); failed; }) : ({ const char* one = ((const char*)suites.data[i]); maca_say(stdout, maca_cat("== ", one), "\n", 1); ran_suites(suites, (i + 1), (failed + ((test_file(one) == 0) ? 0 : 1))); }));  }
long check_cmd(MacaList args) { const char* src = entry_of("check", args); const char* asked = flag_after(args, "--target"); const char* held = ((strcmp(asked, "") == 0) ? "native" : asked); return ((strcmp(src, "") == 0) ? 2 : ((!is_known_target(held)) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca check: no target `", held), "`; built targets are", 1), " native, c, js, jvm, rust, tauri, embedded, nix, or", 1), " `all` for what every program target shares", 1), "\n", 1); 2; }) : ((maca_list_index_of_str(args, "--json") >= 0) ? check_json(src, held) : check_only(src, held))));  }
long check_json(const char* path, const char* target) { Unit unit = unit_of(path); Lexed scanned = end_run(lexed(unit.toks, unit.errs), 0, 0); Module parsed = parse_module(scanned.tokens, 0, maca_listv(0)); Module raw = desugared(parsed, path); MacaList broke = maca_list_cat(maca_list_cat(unit.unknown, scanned.errors), parsed.errors); MacaList said = (((broke.len) > 0) ? unparsed(broke, 0, maca_listv(0)) : maca_list_cat(unread(raw.errors, 0, maca_listv(0)), check_diagnostics_on(raw, target))); MacaList body = diag_list(path, maca_read_file(path), said, 0, maca_listv(0)); maca_say(stdout, maca_cat_own(maca_cat_own("{\"format\":1,\"diagnostics\":[", maca_list_join(body, ","), 2), "]}", 1), "\n", 1); return (((said.len) == 0) ? 0 : 1);  }
MacaList unread(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ({ Diagnostic said = (Diagnostic){ .code = "M0008", .message = ((const char*)xs.data[i]), .pos = 0, .note = maca_cat("the file is read from beside the program", " while it is being built") }; unread(xs, (i + 1), maca_list_pushed(acc, maca_box(sizeof(Diagnostic), (Diagnostic[]){ said }))); }));  }
MacaList unparsed(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ({ Diagnostic said = (Diagnostic){ .code = "M0001", .message = ((const char*)xs.data[i]), .pos = 0, .note = maca_cat("the file did not parse, so nothing", " after this was checked") }; unparsed(xs, (i + 1), maca_list_pushed(acc, maca_box(sizeof(Diagnostic), (Diagnostic[]){ said }))); }));  }
MacaList diag_list(const char* path, const char* text, MacaList ds, long i, MacaList acc) { return ((i >= (ds.len)) ? acc : ({ const char* one = diag_json(path, text, (*(Diagnostic*)ds.data[i])); diag_list(path, text, ds, (i + 1), maca_list_pushed(acc, (long)(one))); }));  }
const char* diag_json(const char* path, const char* text, Diagnostic d) { const char* name = quoted_name(d.message); long at = anchor_at(text, name, d.pos); long lo = ((at < 0) ? at_or_zero(text, d.pos) : at); long hi = ((at < 0) ? lo : (lo + ((int)strlen(name)))); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{\"code\":", json_text(d.code)), ",\"severity\":\"error\",\"message\":", 1), json_text(d.message), 1), ",\"explain\":", 1), json_text(diag_explain(d.code)), 1), ",\"note\":", 1), note_json(d.note), 1), ",\"file\":", 1), json_text(path), 1), ",\"span\":", 1), spot_json(text, lo, hi), 1), ",\"suggestions\":", 1), fix_suggestions(text, d, lo, hi), 1), "}", 1);  }
const char* note_json(const char* note) { return ((strcmp(note, "") == 0) ? "null" : json_text(note));  }
const char* fix_suggestions(const char* text, Diagnostic d, long lo, long hi) { const char* name = quoted_name(d.message); return (((!maca_starts_with(d.message, "no expression starts at ")) || (!phantom_spelling(name))) ? "[]" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("[{\"message\":", json_text(maca_cat_own(maca_cat("delete `", name), "`", 1))), ",\"span\":", 1), spot_json(text, lo, hi), 1), ",\"replacement\":\"\",\"applicability\":", 1), "\"machine-applicable\"}]", 1));  }
const char* spot_json(const char* text, long lo, long hi) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("{\"start\":", maca_int_to_str(lo), 2), ",\"end\":", 1), maca_int_to_str(hi), 3), ",\"start_line\":", 1), maca_int_to_str(line_at(text, lo)), 3), ",\"start_column\":", 1), maca_int_to_str(col_at(text, lo)), 3), ",\"end_line\":", 1), maca_int_to_str(line_at(text, hi)), 3), ",\"end_column\":", 1), maca_int_to_str(col_at(text, hi)), 3), "}", 1);  }
const char* json_text(const char* s) { return maca_cat_own(maca_cat_own("\"", maca_replace(maca_replace(maca_replace(maca_replace(maca_replace(s, "\\", "\\\\"), "\"", "\\\""), "\n", "\\n"), "\r", "\\r"), "\t", "\\t"), 2), "\"", 1);  }
long line_at(const char* text, long at) { return (maca_split(maca_str_slice(text, 0, at), "\n").len);  }
long col_at(const char* text, long at) { MacaList before = maca_split(maca_str_slice(text, 0, at), "\n"); return (((int)strlen(((const char*)before.data[((before.len) - 1)]))) + 1);  }
const char* quoted_name(const char* msg) { long open_mc = maca_str_index_of(msg, "`"); const char* rest = ((open_mc < 0) ? "" : maca_str_slice(msg, (open_mc + 1), ((int)strlen(msg)))); long shut = maca_str_index_of(rest, "`"); return ((shut < 0) ? "" : maca_str_slice(rest, 0, shut));  }
long anchor_at(const char* text, const char* name, long from) { long ahead = ((strcmp(name, "") == 0) ? (-1) : word_from(text, name, at_or_zero(text, from))); return ((ahead >= 0) ? ahead : ((strcmp(name, "") == 0) ? (-1) : word_from(text, name, 0)));  }
long at_or_zero(const char* text, long at) { return (((at > 0) && (at < ((int)strlen(text)))) ? at : 0);  }
long word_from(const char* text, const char* name, long from) { long n = ((int)strlen(name)); long last = (((int)strlen(text)) - n); long i = ((from < 0) ? 0 : from); long found = (-1); while ((i <= last)) { if (word_here(text, name, i, n)) { found = i; break; } i = (i + 1); } return found;  }
long word_here(const char* text, const char* name, long at, long n) { return ((strcmp(maca_str_at(text, at), maca_str_at(name, 0)) != 0) ? 0 : ((word_char(maca_str_at(text, (at - 1))) || word_char(maca_str_at(text, (at + n)))) ? 0 : (strcmp(maca_str_slice(text, at, (at + n)), name) == 0)));  }
long word_char(const char* c) { return ((strcmp(c, "") != 0) && (((isalpha((unsigned char)(c)[0]) != 0) || (isdigit((unsigned char)(c)[0]) != 0)) || (strcmp(c, "_") == 0)));  }
const char* entry_of(const char* cmd, MacaList args) { const char* named = first_named(args); return ((strcmp(named, "") != 0) ? named : declared_bin(cmd, flag_after(args, "--bin")));  }
const char* first_named(MacaList args) { return ((((args.len) < 2) || maca_starts_with(((const char*)args.data[1]), "--")) ? "" : ((const char*)args.data[1]));  }
const char* stem_of(const char* src) { const char* leaf = maca_str_slice(src, sep_after(maca_chars(src), (((int)strlen(src)) - 1)), ((int)strlen(src))); return (maca_ends_with(leaf, ".maca") ? maca_str_slice(leaf, 0, (((int)strlen(leaf)) - 5)) : leaf);  }
MacaList manifest_chain(const char* from) { MacaList dirs = manifest_dirs(rooted(from), maca_listv(0)); return upto(dirs, workspace_at(dirs, ((dirs.len) - 1)), 0, maca_listv(0));  }
const char* rooted(const char* from) { const char* at = maca_real_path(((strcmp(from, "") == 0) ? "." : from)); return ((strcmp(at, "") == 0) ? "" : maca_cat(at, "/"));  }
MacaList manifest_dirs(const char* dir, MacaList acc) { return ((strcmp(dir, "") == 0) ? acc : (maca_file_exists(maca_cat(dir, Manifest)) ? manifest_dirs(parent_of(dir), maca_list_cat(acc, maca_listv(1, (long)(dir)))) : manifest_dirs(parent_of(dir), acc)));  }
long workspace_at(MacaList dirs, long i) { return ((i < 0) ? (((dirs.len) == 0) ? (-1) : 0) : (declares_workspace(maca_read_file(maca_cat(((const char*)dirs.data[i]), Manifest))) ? i : workspace_at(dirs, (i - 1))));  }
long declares_workspace(const char* toml) { return heads(maca_split(toml, "\n"), 0, "[workspace]");  }
long heads(MacaList lines, long i, const char* want) { return ((i >= (lines.len)) ? 0 : ((strcmp(maca_trim(((const char*)lines.data[i])), want) == 0) ? 1 : heads(lines, (i + 1), want)));  }
MacaList upto(MacaList dirs, long stop, long i, MacaList acc) { return (((stop < 0) || (i > stop)) ? acc : upto(dirs, stop, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)dirs.data[i]))))));  }
MacaList here_chain() { return manifest_chain(".");  }
MacaList chain_of(const char* src) { return manifest_chain(dir_of(src));  }
const char* chain_value(MacaList chain, long i, const char* table, const char* key) { return ((i >= (chain.len)) ? "" : ({ const char* got = toml_value(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)), table, key); ((strcmp(got, "") != 0) ? got : chain_value(chain, (i + 1), table, key)); }));  }
long workspace_ok(MacaList chain) { const char* why = workspace_problem(chain); return ((strcmp(why, "") == 0) ? 1 : ({ maca_say(stderr, maca_cat("maca: ", why), "\n", 1); 0; }));  }
const char* workspace_problem(MacaList chain) { return (((chain.len) == 0) ? "" : ({ const char* root = ((const char*)chain.data[((chain.len) - 1)]); const char* nested = nested_workspace(chain, root, 0); ((strcmp(nested, "") != 0) ? nested : members_problem(root, members_of(maca_read_file(maca_cat(root, Manifest))))); }));  }
const char* nested_workspace(MacaList chain, const char* root, long i) { return ((i >= (chain.len)) ? "" : (((strcmp(((const char*)chain.data[i]), root) != 0) && declares_workspace(maca_read_file(maca_cat(((const char*)chain.data[i]), Manifest)))) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", ((const char*)chain.data[i])), Manifest, 1), ": [workspace] inside a workspace; only ", 1), maca_cat_own(maca_cat_own(maca_cat("", root), Manifest, 1), " declares one", 1), 3) : nested_workspace(chain, root, (i + 1))));  }
const char* members_problem(const char* root, MacaList ms) { const char* listed = member_refusal(root, ms, 0); return ((strcmp(listed, "") != 0) ? listed : stray_in(root, ms, parents_of(ms, 0, maca_listv(0)), 0));  }
const char* member_refusal(const char* root, MacaList ms, long i) { return ((i >= (ms.len)) ? "" : ((strcmp(maca_read_file(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), ((const char*)ms.data[i]), 1), "/", 1), Manifest, 1)), "") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), Manifest, 1), ": [workspace] member `", 1), ((const char*)ms.data[i]), 1), "` has no ", 1), Manifest, 1) : (named_package(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), ((const char*)ms.data[i]), 1), "/", 1), Manifest, 1)) ? member_refusal(root, ms, (i + 1)) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), ((const char*)ms.data[i]), 1), "/", 1), Manifest, 1), ": a workspace member states its own ", 1), "[package] name", 1))));  }
long named_package(const char* file) { return (strcmp(toml_value(maca_read_file(file), "[package]", "name"), "") != 0);  }
MacaList parents_of(MacaList ms, long i, MacaList acc) { return ((i >= (ms.len)) ? acc : ({ const char* up = dir_of(((const char*)ms.data[i])); parents_of(ms, (i + 1), ((maca_list_index_of_str(acc, up) >= 0) ? acc : maca_list_cat(acc, maca_listv(1, (long)(up))))); }));  }
const char* stray_in(const char* root, MacaList ms, MacaList parents, long i) { return ((i >= (parents.len)) ? "" : ({ const char* found = stray_at(root, ms, ((const char*)parents.data[i]), maca_list_dir(maca_cat(root, ((const char*)parents.data[i]))), 0); ((strcmp(found, "") != 0) ? found : stray_in(root, ms, parents, (i + 1))); }));  }
const char* stray_at(const char* root, MacaList ms, const char* parent, MacaList names, long i) { return ((i >= (names.len)) ? "" : ((maca_file_exists(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), parent, 1), ((const char*)names.data[i]), 1), "/", 1), Manifest, 1)) && (maca_list_index_of_str(ms, maca_cat(parent, ((const char*)names.data[i]))) < 0)) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), Manifest, 1), ": `", 1), parent, 1), ((const char*)names.data[i]), 1), "` holds a ", 1), Manifest, 1), " but is ", 1), "not a [workspace] member; list it or delete it", 1) : stray_at(root, ms, parent, names, (i + 1))));  }
MacaList members_of(const char* toml) { MacaList lines = maca_split(toml, "\n"); long at = key_line(lines, 0, "", "[workspace]", "members"); return ((at < 0) ? maca_listv(0) : cleaned(maca_split(bracketed(list_body(lines, at, "")), ","), 0, maca_listv(0)));  }
long key_line(MacaList lines, long i, const char* at, const char* table, const char* key) { return ((i >= (lines.len)) ? (-1) : ((strcmp(toml_head(((const char*)lines.data[i])), "") != 0) ? key_line(lines, (i + 1), toml_head(((const char*)lines.data[i])), table, key) : (((strcmp(at, table) == 0) && (strcmp(toml_key(((const char*)lines.data[i])), key) == 0)) ? i : key_line(lines, (i + 1), at, table, key))));  }
const char* list_body(MacaList lines, long i, const char* acc) { return (((i >= (lines.len)) || (maca_str_index_of(acc, "]") >= 0)) ? acc : list_body(lines, (i + 1), maca_cat_own(acc, maca_trim(((const char*)lines.data[i])), 2)));  }
const char* bracketed(const char* s) { long a = maca_str_index_of(s, "["); long b = maca_str_index_of(s, "]"); return (((a < 0) || (b <= a)) ? "" : maca_str_slice(s, (a + 1), b));  }
MacaList cleaned(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ({ const char* one = unquoted(maca_trim(((const char*)xs.data[i]))); cleaned(xs, (i + 1), ((strcmp(one, "") == 0) ? acc : maca_list_cat(acc, maca_listv(1, (long)(one))))); }));  }
const char* declared_bin(const char* cmd, const char* want) { MacaList chain = here_chain(); return (((chain.len) == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", cmd), ": expected a .maca file, and there is no ", 1), Manifest, 1), " here to name one", 1), "\n", 1); ""; }) : ((!workspace_ok(chain)) ? "" : chosen_bin(cmd, chain, want)));  }
const char* chosen_bin(const char* cmd, MacaList chain, const char* want) { const char* own = ((const char*)chain.data[0]); const char* pkg = package_of(chain); MacaList bins = bins_of(maca_split(maca_read_file(maca_cat(own, Manifest)), "\n"), 0, maca_listv(0)); const char* asked = ((strcmp(want, "") == 0) ? chain_value(chain, 0, "[build]", "bin") : want); long at = bin_pick(bins, asked); long n = (bins.len); const char* names = bin_names(bins, 0, ""); return ((n == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", cmd), ": package `", 1), pkg, 1), "` declares no [[bin]] in ", 1), Manifest, 1), "; name a .maca file", 1), "\n", 1); ""; }) : ((at < 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", cmd), ": package `", 1), pkg, 1), "` declares ", 1), maca_int_to_str(n), 3), " binaries; pass --bin <name>, ", 1), maca_cat_own(maca_cat("or declare [build] bin (one of ", names), ")", 1), 3), "\n", 1); ""; }) : bin_file(cmd, own, (*(Bin*)bins.data[at]))));  }
const char* bin_file(const char* cmd, const char* dir, Bin b) { const char* at = maca_cat(dir, b.path); return (maca_file_exists(at) ? at : ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", cmd), ": [[bin]] `", 1), b.name, 1), "` names ", 1), at, 1), ", which is not a file", 1), "\n", 1); ""; }));  }
long bin_pick(MacaList bins, const char* want) { return ((strcmp(want, "") != 0) ? bin_at(bins, want, 0) : (((bins.len) == 1) ? 0 : (-1)));  }
long bin_at(MacaList bins, const char* want, long i) { return ((i >= (bins.len)) ? (-1) : ((strcmp((*(Bin*)bins.data[i]).name, want) == 0) ? i : bin_at(bins, want, (i + 1))));  }
const char* bin_names(MacaList bins, long i, const char* acc) { return ((i >= (bins.len)) ? acc : ((strcmp(acc, "") == 0) ? bin_names(bins, (i + 1), (*(Bin*)bins.data[i]).name) : bin_names(bins, (i + 1), maca_cat_own(maca_cat(acc, ", "), (*(Bin*)bins.data[i]).name, 1))));  }
const char* package_of(MacaList chain) { const char* got = chain_value(chain, 0, "[package]", "name"); return ((strcmp(got, "") == 0) ? "this package" : got);  }
MacaList bins_of(MacaList lines, long i, MacaList acc) { return ((i >= (lines.len)) ? acc : ((strcmp(toml_head(((const char*)lines.data[i])), "[[bin]]") == 0) ? bins_of(lines, (i + 1), with_bin(acc, one_bin(lines, (i + 1)))) : bins_of(lines, (i + 1), acc)));  }
MacaList with_bin(MacaList acc, Bin b) { return ((strcmp(b.path, "") == 0) ? acc : maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Bin), (Bin[]){ b }))));  }
Bin one_bin(MacaList lines, long i) { const char* path = block_value(lines, i, "path"); const char* name = block_value(lines, i, "name"); return (Bin){ .name = ((strcmp(name, "") == 0) ? path : name), .path = path };  }
const char* block_value(MacaList lines, long i, const char* key) { return (((i >= (lines.len)) || (strcmp(toml_head(((const char*)lines.data[i])), "") != 0)) ? "" : ((strcmp(toml_key(((const char*)lines.data[i])), key) == 0) ? toml_val(((const char*)lines.data[i])) : block_value(lines, (i + 1), key)));  }
MacaList table_keys(const char* toml, const char* table) { return keys_in(maca_split(toml, "\n"), 0, "", table, maca_listv(0));  }
MacaList keys_in(MacaList lines, long i, const char* at, const char* table, MacaList acc) { return ((i >= (lines.len)) ? acc : ((strcmp(toml_head(((const char*)lines.data[i])), "") != 0) ? keys_in(lines, (i + 1), toml_head(((const char*)lines.data[i])), table, acc) : (((strcmp(at, table) == 0) && (strcmp(toml_key(((const char*)lines.data[i])), "") != 0)) ? keys_in(lines, (i + 1), at, table, maca_list_cat(acc, maca_listv(1, (long)(toml_key(((const char*)lines.data[i])))))) : keys_in(lines, (i + 1), at, table, acc))));  }
const char* toml_value(const char* toml, const char* table, const char* key) { return table_value(maca_split(toml, "\n"), 0, "", table, key);  }
const char* table_value(MacaList lines, long i, const char* at, const char* table, const char* key) { return ((i >= (lines.len)) ? "" : ((strcmp(toml_head(((const char*)lines.data[i])), "") != 0) ? table_value(lines, (i + 1), toml_head(((const char*)lines.data[i])), table, key) : (((strcmp(at, table) == 0) && (strcmp(toml_key(((const char*)lines.data[i])), key) == 0)) ? toml_val(((const char*)lines.data[i])) : table_value(lines, (i + 1), at, table, key))));  }
const char* toml_head(const char* line) { const char* t = maca_trim(line); return (maca_starts_with(t, "[") ? t : "");  }
const char* toml_key(const char* line) { const char* t = maca_trim(line); return ((maca_starts_with(t, "#") || (maca_str_index_of(t, "=") < 0)) ? "" : maca_trim(maca_str_slice(t, 0, maca_str_index_of(t, "="))));  }
const char* toml_val(const char* line) { const char* t = maca_trim(line); return unquoted(maca_trim(maca_str_slice(t, (maca_str_index_of(t, "=") + 1), ((int)strlen(t)))));  }
const char* unquoted(const char* v) { return ((((((int)strlen(v)) >= 2) && maca_starts_with(v, "\"")) && maca_ends_with(v, "\"")) ? maca_str_slice(v, 1, (((int)strlen(v)) - 1)) : v);  }
MacaList nonempty(MacaList xs, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((strcmp(((const char*)xs.data[i]), "") == 0) ? nonempty(xs, (i + 1), acc) : nonempty(xs, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)xs.data[i])))))));  }
const char* shell_pid() { return maca_trim(maca_capture("sh", maca_listv(2, (long)("-c"), (long)("echo $$"))));  }
const char* scratch_path(const char* kind) { const char* root = maca_env("TMPDIR"); const char* dir = ((strcmp(root, "") == 0) ? "/tmp" : root); return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", dir), "/maca1-", 1), kind, 1), "-", 1), maca_int_to_str(maca_now_ms()), 3), "-", 1), shell_pid(), 1);  }
long run_file(const char* src, MacaList rest) { const char* out = scratch_path("run"); long built = build_binary(src, out); return ((built != 0) ? built : ({ long code = maca_exec(out, rest); maca_remove_file(out); maca_remove_file(maca_cat_own(maca_cat("", out), ".c", 1)); code; }));  }
long test_file(const char* src) { Unit unit = unit_of(src); Lexed scanned = end_run(lexed(unit.toks, unit.errs), 0, 0); Module raw = parse_module(scanned.tokens, 0, maca_listv(0)); MacaList names = test_names(raw.items, 0, maca_listv(0)); return (((names.len) == 0) ? ({ maca_say(stdout, maca_cat("no test_ functions in ", src), "\n", 1); 0; }) : run_tests(src, names));  }
MacaList test_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : ((((*(Stmt*)items.data[i]).kind == SFn) && maca_starts_with((*(Stmt*)items.data[i]).name, "test_")) ? test_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : test_names(items, (i + 1), acc)));  }
long run_tests(const char* src, MacaList names) { const char* driver = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", dir_of(src)), "maca1-test-", 1), maca_int_to_str(maca_now_ms()), 3), ".maca", 1); maca_write_file(driver, maca_cat(without_main(maca_read_file(src)), test_main(names, 0, ""))); long code = run_file(driver, maca_listv(0)); maca_remove_file(driver); return code;  }
const char* without_main(const char* src) { MacaList items = parse_module(lex(src), 0, maca_listv(0)).items; long at = main_item(items, 0); return ((at < 0) ? src : cut_out(src, (*(Stmt*)items.data[at]).pos, item_start(items, (at + 1))));  }
long main_item(MacaList items, long i) { return ((i >= (items.len)) ? (0 - 1) : ((((*(Stmt*)items.data[i]).kind == SFn) && (strcmp((*(Stmt*)items.data[i]).name, "main") == 0)) ? i : main_item(items, (i + 1))));  }
long item_start(MacaList items, long i) { return ((i >= (items.len)) ? (0 - 1) : (*(Stmt*)items.data[i]).pos);  }
const char* cut_out(const char* src, long from, long upto) { MacaList cs = maca_chars(src); MacaList tail = ((upto < 0) ? maca_listv(0) : maca_list_slice(cs, upto, (cs.len))); return maca_cat_own(maca_list_join(maca_list_slice(cs, 0, from), ""), maca_list_join(tail, ""), 3);  }
const char* test_main(MacaList names, long i, const char* acc) { return ((i >= (names.len)) ? maca_cat_own(maca_cat("\nmain() -> int {\n", acc), "    failures()\n}\n", 1) : ({ const char* call = maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("    info(\"  ", ((const char*)names.data[i])), "\")\n    ", 1), ((const char*)names.data[i]), 1), "()\n", 1); test_main(names, (i + 1), maca_cat(acc, call)); }));  }
long compile_file(MacaList args) { Unit unit = unit_of(((const char*)args.data[0])); Lexed scanned = end_run(lexed(unit.toks, unit.errs), 0, 0); Module parsed = parse_module(scanned.tokens, 0, maca_listv(0)); Module raw = desugared(parsed, ((const char*)args.data[0])); long bad = (((report_all("import", unit.unknown, 0) + report_all("scan", scanned.errors, 0)) + report_all("parse", parsed.errors, 0)) + report_all("embed", raw.errors, 0)); long errs = (bad + report_all("check", check_errors(raw), 0)); Module m = annotated(monomorphic(lifted(annotated(raw)))); const char* asked = (((args.len) >= 3) ? ((const char*)args.data[2]) : ""); const char* emitted = ((strcmp(asked, "rust") == 0) ? remit_module(m) : ((strcmp(asked, "js") == 0) ? jemit_module(m) : ((strcmp(asked, "nix") == 0) ? nemit_module(m) : ((strcmp(asked, "embedded") == 0) ? eemit_module(m) : ((strcmp(asked, "jvm") == 0) ? jvmemit_module(m, (((args.len) >= 4) ? ((const char*)args.data[3]) : "Main")) : emit_module(m)))))); maca_write_file(((const char*)args.data[1]), emitted); return ((errs + refused_here(asked, m)) + report_all("import", foreign_errors(asked, ((const char*)args.data[0]), unit), 0));  }
long refused_here(const char* asked, Module m) { return ((strcmp(asked, "embedded") == 0) ? report_all("embedded", eemit_errors(m), 0) : ((strcmp(asked, "jvm") == 0) ? (report_all("jvm", jvmemit_errors(m), 0) + report_all("jvm", ported_errors(m.items, 0, "jvm", maca_listv(0)), 0)) : ((strcmp(asked, "rust") == 0) ? (report_all("rust", ported_errors(m.items, 0, "rust", maca_listv(0)), 0) + report_all("rust", kept_borrow_notes(m), 0)) : 0)));  }
MacaList ported_errors(MacaList items, long i, const char* target, MacaList acc) { return ((i >= (items.len)) ? acc : ported_errors(items, (i + 1), target, maca_list_cat(acc, ported_error((*(Stmt*)items.data[i]), target))));  }
MacaList ported_error(Stmt s, const char* target) { return (((s.kind == SFn) && ((s.body.len) == 0)) ? maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", s.name), "` is declared with no body, which is an FFI declaration, ", 1), maca_cat_own(maca_cat("and --target ", target), " has no C ABI bridge to reach it; supply ", 1), 3), "a body, or write the function in a raw block", 1))) : ((s.kind == SRecord) ? fn_fields(s.name, s.params, 0, target, maca_listv(0)) : maca_listv(0)));  }
MacaList fn_fields(const char* rec, MacaList fs, long i, const char* target, MacaList acc) { return ((i >= (fs.len)) ? acc : (is_fn_type((*(Expr*)fs.data[i]).ty) ? fn_fields(rec, fs, (i + 1), target, maca_list_cat(acc, maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", rec), ".", 1), (*(Expr*)fs.data[i]).text, 1), "` holds a function, which ", 1), maca_cat_own(maca_cat("--target ", target), " cannot carry; the native ", 1), 3), "and js targets can", 1))))) : fn_fields(rec, fs, (i + 1), target, acc)));  }
MacaList foreign_errors(const char* asked, const char* src, Unit u) { MacaList js = browser_errors(asked, u); return ((strcmp(asked, "rust") != 0) ? js : maca_list_cat(js, rust_import_errors(u.toks, 0, manifest_keys(chain_of(src), "[rust-dependencies]", 0), maca_listv(0))));  }
MacaList browser_errors(const char* asked, Unit u) { return (((strcmp(asked, "js") == 0) || (!js_import_in(u.toks, 0))) ? maca_listv(0) : maca_listv(1, (long)(browser_error(asked, browser_file(u.seen, 0)))));  }
const char* browser_error(const char* asked, const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`", name), "` runs in a browser: what implements it is an `import js` block, ", 1), maca_cat_own(maca_cat("and the ", target_named(asked)), " target has no JavaScript to run it ", 1), 3), "in; build the page with `maca build --target js`", 1);  }
const char* target_named(const char* asked) { return ((strcmp(asked, "") == 0) ? "native" : asked);  }
long js_import_in(MacaList ts, long i) { return (((i + 2) >= (ts.len)) ? 0 : (foreign_at(ts, i, "js") ? 1 : js_import_in(ts, (i + 1))));  }
long foreign_at(MacaList ts, long i, const char* lang) { return ((((*(Token*)ts.data[i]).kind == KwImport) && (strcmp((*(Token*)ts.data[(i + 1)]).text, lang) == 0)) && ((*(Token*)ts.data[(i + 2)]).kind == TStr));  }
const char* browser_file(MacaList files, long i) { return ((i >= (files.len)) ? "" : (js_import_in(lex_all(maca_read_file(((const char*)files.data[i]))).tokens, 0) ? module_named(((const char*)files.data[i])) : browser_file(files, (i + 1))));  }
const char* module_named(const char* path) { const char* stem = (maca_ends_with(path, ".maca") ? maca_str_slice(path, 0, (((int)strlen(path)) - 5)) : path); MacaList parts = maca_split(stem, "/"); return maca_list_join(maca_list_slice(parts, last_root(parts, 0, ((parts.len) - 1)), (parts.len)), "/");  }
long last_root(MacaList ps, long i, long acc) { return ((i >= (ps.len)) ? acc : ((((strcmp(((const char*)ps.data[i]), "modules") == 0) || (strcmp(((const char*)ps.data[i]), "src") == 0)) || (strcmp(((const char*)ps.data[i]), DepsDir) == 0)) ? last_root(ps, (i + 1), (i + 1)) : last_root(ps, (i + 1), acc)));  }
MacaList rust_import_errors(MacaList ts, long i, MacaList deps, MacaList acc) { return (((i + 2) >= (ts.len)) ? acc : ((((*(Token*)ts.data[i]).kind == KwImport) && ((*(Token*)ts.data[(i + 2)]).kind == TStr)) ? rust_import_errors(ts, (i + 3), deps, maca_list_cat(acc, rust_import_error((*(Token*)ts.data[(i + 1)]).text, (*(Token*)ts.data[(i + 2)]).text, deps))) : rust_import_errors(ts, (i + 1), deps, acc)));  }
MacaList rust_import_error(const char* lang, const char* spec, MacaList deps) { const char* krate = crate_of(spec); return ((strcmp(lang, "rust") != 0) ? maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("`import ", lang), " \"", 1), spec, 1), "\"` is not supported with --target rust ", 1), "(there is no C ABI bridge on the Rust path); call a Rust ", 1), "crate via `import rust` and [rust-dependencies] instead", 1))) : ((((strcmp(krate, "") == 0) || rust_builtin(krate)) || (maca_list_index_of_str(deps, krate) >= 0)) ? maca_listv(0) : maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("import rust \"", spec), "\" refers to crate `", 1), krate, 1), "`, which isn't ", 1), maca_cat_own(maca_cat("declared; add `", krate), " = \"…\"` under [rust-dependencies] ", 1), 3), "in maca.toml", 1)))));  }
long rust_builtin(const char* name) { return ((((((strcmp(name, "std") == 0) || (strcmp(name, "core") == 0)) || (strcmp(name, "alloc") == 0)) || (strcmp(name, "crate") == 0)) || (strcmp(name, "self") == 0)) || (strcmp(name, "super") == 0));  }
const char* crate_of(const char* spec) { const char* s = maca_trim(spec); long cut = maca_str_index_of(s, "::"); const char* head = ((cut < 0) ? s : maca_str_slice(s, 0, cut)); return (path_like(maca_chars(s), 0) ? unprefixed(head) : "");  }
const char* unprefixed(const char* name) { return (maca_starts_with(name, "r#") ? maca_str_slice(name, 2, ((int)strlen(name))) : name);  }
long path_like(MacaList cs, long i) { return ((i >= (cs.len)) ? ((cs.len) > 0) : (path_char(((const char*)cs.data[i])) ? path_like(cs, (i + 1)) : 0));  }
long path_char(const char* c) { return (((((isalpha((unsigned char)(c)[0]) != 0) || (isdigit((unsigned char)(c)[0]) != 0)) || (strcmp(c, "_") == 0)) || (strcmp(c, ":") == 0)) || (strcmp(c, "#") == 0));  }
Unit unit_of(const char* entry) { return load_unit(entry, (Unit){ .seen = maca_listv(0), .names = maca_listv(0), .owners = maca_listv(0), .asks = asks_of(entry), .toks = maca_listv(0), .errs = maca_listv(0), .unknown = maca_listv(0) });  }
MacaList asks_of(const char* entry) { return walk_asks(entry, (Asked){ .walked = maca_listv(0), .pairs = maca_listv(0) }).pairs;  }
Asked walk_asks(const char* path, Asked a) { return ((maca_list_index_of_str(a.walked, path) >= 0) ? a : ({ Lexed got = lex_all(maca_read_file(path)); asks_in(path, got.tokens, 0, ({ __typeof__(a) _w = a; _w.walked = maca_list_cat(a.walked, maca_listv(1, (long)(path))); _w; })); }));  }
Asked asks_in(const char* by, MacaList ts, long i, Asked a) { return (((*(Token*)ts.data[i]).kind == Eof) ? a : (((*(Token*)ts.data[i]).kind == KwImport) ? asks_in(by, ts, import_end(ts, (i + 1)), asks_at(by, ts, (i + 1), a)) : asks_in(by, ts, (i + 1), a)));  }
Asked asks_at(const char* by, MacaList ts, long i, Asked a) { MacaList paths = import_names(ts, i); const char* at = (((paths.len) == 0) ? "" : resolved(by, ((const char*)paths.data[0]))); return ((strcmp(at, "") == 0) ? a : (((*(Token*)ts.data[i]).kind == LBrace) ? walk_asks(at, ({ __typeof__(a) _w = a; _w.pairs = maca_list_cat(a.pairs, selected(ts, (i + 1), at, maca_listv(0))); _w; })) : walk_asks(at, a)));  }
MacaList selected(MacaList ts, long i, const char* at, MacaList acc) { return ((((*(Token*)ts.data[i]).kind == RBrace) || ((*(Token*)ts.data[i]).kind == Eof)) ? acc : (((*(Token*)ts.data[i]).kind == TIdent) ? selected(ts, (i + 1), at, maca_list_cat(acc, maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat("", at), " ", 1), (*(Token*)ts.data[i]).text, 1))))) : selected(ts, (i + 1), at, acc)));  }
const char* nix_valued(const char* path, const char* src) { return ((maca_str_index_of(src, "import nix") < 0) ? src : maca_cat(nix_bound(dir_of(path), lex_all(src).tokens, 0, ""), src));  }
const char* nix_bound(const char* dir, MacaList ts, long i, const char* acc) { return (((*(Token*)ts.data[i]).kind == Eof) ? acc : (((((*(Token*)ts.data[i]).kind == KwImport) && (strcmp((*(Token*)ts.data[(i + 1)]).text, "nix") == 0)) && ((*(Token*)ts.data[(i + 2)]).kind == TStr)) ? nix_bound(dir, ts, (i + 3), maca_cat(acc, nix_binding(dir, (*(Token*)ts.data[(i + 2)]).text))) : nix_bound(dir, ts, (i + 1), acc)));  }
const char* nix_binding(const char* dir, const char* spec) { const char* got = maca_trim(maca_capture("nix-instantiate", maca_listv(2, (long)("--eval"), (long)(maca_cat(dir, spec))))); return ((strcmp(got, "") == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", nix_name(spec)), " = ", 1), got, 1), "\n", 1));  }
const char* nix_name(const char* spec) { const char* leaf = maca_str_slice(spec, sep_after(maca_chars(spec), (((int)strlen(spec)) - 1)), ((int)strlen(spec))); return (maca_ends_with(leaf, ".nix") ? maca_str_slice(leaf, 0, (((int)strlen(leaf)) - 4)) : leaf);  }
Unit load_unit(const char* path, Unit u) { return ((maca_list_index_of_str(u.seen, path) >= 0) ? u : ({ Lexed got = lex_all(nix_valued(path, maca_read_file(path))); Unit marked = ({ __typeof__(u) _w = u; _w.seen = maca_list_cat(u.seen, maca_listv(1, (long)(path))); _w; }); Unit deps = load_deps(path, imports_in(got.tokens, 0, maca_listv(0)), 0, marked); spliced(deps, path, got.tokens, got.errors); }));  }
Unit spliced(Unit deps, const char* path, MacaList whole, MacaList errs) { MacaList mine = fn_names(parse_module(whole, 0, maca_listv(0)).items, 0, maca_listv(0)); MacaList clash = clashing(mine, deps.names, 0, maca_listv(0)); MacaList held = holding(deps, path, clash, 0, maca_listv(0)); MacaList taken = without_names(clash, held, 0, maca_listv(0)); const char* tag = maca_int_to_str((deps.seen.len)); MacaList toks = without_eof(whole); MacaList kept = (((taken.len) == 0) ? toks : renamed(toks, taken, tag, 0)); MacaList base = (((held.len) == 0) ? deps.toks : renamed(deps.toks, held, tag, 0)); return ({ __typeof__(deps) _w = deps; _w.toks = maca_list_cat(base, kept); _w.names = maca_list_cat(deps.names, mine); _w.owners = maca_list_cat(deps.owners, owned_by(mine, path, 0, maca_listv(0))); _w.errs = maca_list_cat(deps.errs, errs); _w; });  }
MacaList holding(Unit u, const char* path, MacaList clash, long i, MacaList acc) { return ((i >= (clash.len)) ? acc : (keeps(u, path, ((const char*)clash.data[i])) ? holding(u, path, clash, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)clash.data[i]))))) : holding(u, path, clash, (i + 1), acc)));  }
long keeps(Unit u, const char* path, const char* name) { return ((strcmp(path, ((const char*)u.seen.data[0])) == 0) || (wanted(u.asks, path, name) && (!wanted(u.asks, owner_of(u, name, 0, ""), name))));  }
long wanted(MacaList asks, const char* path, const char* name) { return (maca_list_index_of_str(asks, maca_cat_own(maca_cat_own(maca_cat("", path), " ", 1), name, 1)) >= 0);  }
const char* owner_of(Unit u, const char* name, long i, const char* held) { return ((i >= (u.names.len)) ? held : ((strcmp(((const char*)u.names.data[i]), name) == 0) ? owner_of(u, name, (i + 1), ((const char*)u.owners.data[i])) : owner_of(u, name, (i + 1), held)));  }
MacaList without_names(MacaList xs, MacaList drop, long i, MacaList acc) { return ((i >= (xs.len)) ? acc : ((maca_list_index_of_str(drop, ((const char*)xs.data[i])) >= 0) ? without_names(xs, drop, (i + 1), acc) : without_names(xs, drop, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)xs.data[i])))))));  }
MacaList owned_by(MacaList mine, const char* path, long i, MacaList acc) { return ((i >= (mine.len)) ? acc : owned_by(mine, path, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(path)))));  }
MacaList fn_names(MacaList items, long i, MacaList acc) { return ((i >= (items.len)) ? acc : ((strcmp((*(Stmt*)items.data[i]).name, "") != 0) ? fn_names(items, (i + 1), maca_list_cat(acc, maca_listv(1, (long)((*(Stmt*)items.data[i]).name)))) : fn_names(items, (i + 1), acc)));  }
MacaList clashing(MacaList mine, MacaList taken, long i, MacaList acc) { return ((i >= (mine.len)) ? acc : ((maca_list_index_of_str(taken, ((const char*)mine.data[i])) >= 0) ? clashing(mine, taken, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)mine.data[i]))))) : clashing(mine, taken, (i + 1), acc)));  }
MacaList renamed(MacaList ts, MacaList taken, const char* tag, long i) { MacaList out = maca_listv(0); long at = i; while ((at < (ts.len))) { out = maca_list_pushed(out, maca_box(sizeof(Token), (Token[]){ (((at > 0) && ((*(Token*)ts.data[(at - 1)]).kind == Dot)) ? (*(Token*)ts.data[at]) : one_renamed((*(Token*)ts.data[at]), taken, tag)) })); at = (at + 1); } return out;  }
Token one_renamed(Token t, MacaList taken, const char* tag) { return (((t.kind == TIdent) && (maca_list_index_of_str(taken, t.text) >= 0)) ? mk_token(TIdent, maca_cat_own(maca_cat_own(maca_cat("", t.text), "__", 1), tag, 1), t.pos) : t);  }
MacaList without_eof(MacaList ts) { return maca_list_slice(ts, 0, live_end(ts, (ts.len)));  }
long live_end(MacaList ts, long n) { return ((n <= 0) ? 0 : (((*(Token*)ts.data[(n - 1)]).kind == Eof) ? live_end(ts, (n - 1)) : n));  }
Unit load_deps(const char* by, MacaList wants, long i, Unit u) { return ((i >= (wants.len)) ? u : load_deps(by, wants, (i + 1), one_dep(by, ((const char*)wants.data[i]), u)));  }
Unit one_dep(const char* by, const char* want, Unit u) { const char* at = resolved(by, want); return ((strcmp(at, "") != 0) ? load_unit(at, u) : ((maca_str_index_of(want, "/") < 0) ? u : ({ __typeof__(u) _w = u; _w.unknown = maca_list_cat(u.unknown, maca_listv(1, (long)(maca_cat_own(maca_cat_own(maca_cat("", want), ", imported by ", 1), by, 1)))); _w; })));  }
MacaList imports_in(MacaList ts, long i, MacaList acc) { return (((*(Token*)ts.data[i]).kind == Eof) ? acc : (((*(Token*)ts.data[i]).kind == KwImport) ? imports_in(ts, import_end(ts, (i + 1)), maca_list_cat(acc, import_names(ts, (i + 1)))) : imports_in(ts, (i + 1), acc)));  }
MacaList import_names(MacaList ts, long i) { return (((*(Token*)ts.data[i]).kind == LBrace) ? import_names(ts, (selection_end(ts, (i + 1)) + 2)) : ((((*(Token*)ts.data[i]).kind == TStr) || ((*(Token*)ts.data[(i + 1)]).kind == TStr)) ? maca_listv(0) : maca_listv(1, (long)(import_path(ts, i, "")))));  }
const char* import_path(MacaList ts, long i, const char* acc) { return (((*(Token*)ts.data[(i + 1)]).kind == Slash) ? import_path(ts, (i + 2), maca_cat_own(maca_cat(acc, (*(Token*)ts.data[i]).text), "/", 1)) : maca_cat(acc, (*(Token*)ts.data[i]).text));  }
const char* resolved(const char* by, const char* want) { return search_up(dir_of(by), maca_cat(want, ".maca"));  }
const char* search_up(const char* dir, const char* want) { const char* here = in_base(dir, want); return ((strcmp(here, "") != 0) ? here : ((strcmp(dir, "") == 0) ? "" : search_up(parent_of(dir), want)));  }
const char* in_base(const char* dir, const char* want) { const char* own = found(maca_cat(dir, want)); const char* mods = found(maca_cat_own(maca_cat(dir, "modules/"), want, 1)); const char* src = found(maca_cat_own(maca_cat(dir, "src/"), want, 1)); return ((strcmp(own, "") != 0) ? own : ((strcmp(mods, "") != 0) ? mods : ((strcmp(src, "") != 0) ? src : found(maca_cat_own(maca_cat_own(maca_cat(dir, DepsDir), "/", 1), want, 1)))));  }
const char* found(const char* cand) { return ((strcmp(maca_read_file(cand), "") == 0) ? "" : cand);  }
const char* dir_of(const char* path) { return maca_str_slice(path, 0, sep_after(maca_chars(path), (((int)strlen(path)) - 1)));  }
const char* parent_of(const char* dir) { return dir_of(maca_str_slice(dir, 0, (((int)strlen(dir)) - 1)));  }
long sep_after(MacaList cs, long i) { return ((i < 0) ? 0 : ((strcmp(((const char*)cs.data[i]), "/") == 0) ? (i + 1) : sep_after(cs, (i - 1))));  }
long report_all(const char* stage, MacaList msgs, long i) { return ((i >= (msgs.len)) ? (msgs.len) : ({ maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat("", stage), " error: ", 1), ((const char*)msgs.data[i]), 1), "\n", 1); report_all(stage, msgs, (i + 1)); }));  }
long spec_cmd(MacaList args) { return (((maca_list_index_of_str(args, "--help") >= 0) || (maca_list_index_of_str(args, "-h") >= 0)) ? ({ maca_say(stdout, maca_cat_own(maca_cat("usage: maca spec --llm [--package <name>]\n", "\n  --llm       the whole specification, as one document"), "\n  --package   that one package's index, in the same form", 1), "\n", 1); 0; }) : (((maca_list_index_of_str(args, "--package") >= 0) && (strcmp(flag_after(args, "--package"), "") == 0)) ? ({ maca_say(stderr, "spec: --package wants a name", "\n", 0); 2; }) : spec_asked(spec_root(), flag_after(args, "--package"))));  }
long spec_asked(const char* root, const char* pkg) { return ((strcmp(root, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat("spec: no ", SpecDoc), " above this directory", 1), "\n", 1); 1; }) : ((strcmp(pkg, "") != 0) ? package_index(root, pkg) : printed_spec(llm_spec(root))));  }
const char* spec_root() { return root_above(maca_cat_own(maca_cwd(), "/", 1));  }
const char* root_above(const char* dir) { return ((strcmp(found(maca_cat(dir, SpecDoc)), "") != 0) ? dir : ((strcmp(dir, "") == 0) ? "" : root_above(parent_of(dir))));  }
long printed_spec(const char* text) { long used = ((((int)strlen(text)) + 2) / 3); return ((used > SpecBudget) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("spec: ", maca_int_to_str(used), 2), " tokens, over the ", 1), maca_int_to_str(SpecBudget), 3), " budget", 1), "\n", 1); 1; }) : ({ maca_say(stdout, text, "\n", 0); 0; }));  }
long package_index(const char* root, const char* name) { const char* body = indexed(maca_cat_own(maca_cat(root, "modules/"), name, 1), name, 1); return ((strcmp(body, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("spec: no package `", name), "` under ", 1), root, 1), "modules", 1), "\n", 1); 1; }) : ({ maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat("# `", name), "`\n", 1), body, 1), "\n", 1); 0; }));  }
const char* llm_spec(const char* root) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("# Maca ", Version), "\n", 1), "\nOne typed language for programs and infrastructure config. ", 1), "Everything you write is `.maca` or `maca.toml`.\n", 1), maca_cat_own(maca_cat("\nThe language section is `", SpecDoc), "` verbatim and the indexes are ", 1), 3), "read out of the packages beside it, so neither can drift from the ", 1), "tree this was run in.\n", 1), maca_cat_own(maca_cat("\n## The language\n\n", cheatsheet(maca_read_file(maca_cat(root, SpecDoc)))), "\n", 1), 3), "\n## Whole programs\n\nEach compiles and runs as written.\n", 1), examples(root), 1), "\n## Mistakes to avoid\n", 1), mistakes(), 1), "\n## Targets\n", 1), targets_table(), 1), "\n## The standard library\n\nCarried beside the compiler, so ", 1), "`import std/json` resolves anywhere. Every item below is one ", 1), "`import` away.\n", 1), builtin_methods(), 1), indexed(maca_cat(root, "modules/std"), "std", 0), 1), other_packages(root), 1);  }
const char* cheatsheet(const char* spec) { long at = maca_str_index_of(spec, SpecHeading); const char* rest = ((at < 0) ? "" : maca_str_slice(spec, (at + ((int)strlen(SpecHeading))), ((int)strlen(spec)))); long end = maca_str_index_of(rest, "\n## "); long cut = ((end < 0) ? ((int)strlen(rest)) : end); return maca_trim(maca_str_slice(rest, 0, cut));  }
const char* examples(const char* root) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat(example(root, "a whole program", "tour"), example(root, "sum types and match", "payload_sum")), example(root, "errors, without exceptions", "catch"), 1), example(root, "concurrency, with no `async` keyword", "async"), 1), example(root, "a generic function", "generic"), 1), example(root, "config mode, which compiles to Nix", "ffi_nix"), 1);  }
const char* example(const char* root, const char* what, const char* name) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\n### ", what), "\n\n```maca\n", 1), maca_read_file(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", root), "apps/examples/", 1), name, 1), ".maca", 1)), 3), "```\n", 1);  }
const char* indexed(const char* dir, const char* pkg, long full) { return index_files(dir, pkg, maca_list_dir(dir), 0, full, "");  }
const char* index_files(const char* dir, const char* pkg, MacaList names, long i, long full, const char* acc) { return ((i >= (names.len)) ? acc : ((!maca_ends_with(((const char*)names.data[i]), ".maca")) ? index_files(dir, pkg, names, (i + 1), full, acc) : index_files(dir, pkg, names, (i + 1), full, maca_cat(acc, index_file(dir, pkg, ((const char*)names.data[i]), full)))));  }
const char* index_file(const char* dir, const char* pkg, const char* name, long full) { const char* body = documented(maca_split(maca_read_file(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), name, 1)), "\n"), 0, "", full, ""); const char* stem = maca_str_slice(name, 0, (((int)strlen(name)) - 5)); return ((strcmp(body, "") == 0) ? "" : (full ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\n### `", pkg), "/", 1), stem, 1), "`\n\n", 1), body, 1) : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\n`", pkg), "/", 1), stem, 1), "`\n\n", 1), body, 1)));  }
const char* documented(MacaList lines, long i, const char* doc, long full, const char* acc) { return ((i >= (lines.len)) ? acc : (maca_starts_with(maca_trim(((const char*)lines.data[i])), "///") ? documented(lines, (i + 1), summary_of(((const char*)lines.data[i])), full, acc) : ((strcmp(doc, "") == 0) ? documented(lines, (i + 1), "", full, acc) : documented(lines, (i + 1), "", full, maca_cat(acc, item_line(signature_of(((const char*)lines.data[i])), doc, full))))));  }
const char* summary_of(const char* line) { const char* t = maca_trim(line); return maca_trim(maca_str_slice(t, 3, ((int)strlen(t))));  }
const char* signature_of(const char* line) { const char* t = maca_trim(line); long at = maca_str_index_of(t, "{"); return ((at < 0) ? t : maca_trim(maca_str_slice(t, 0, at)));  }
const char* item_line(const char* sig, const char* doc, long full) { return ((strcmp(sig, "") == 0) ? "" : (full ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("- `", sig), "` ", 1), doc, 1), "\n", 1) : maca_cat_own(maca_cat("- `", sig), "`\n", 1)));  }
const char* other_packages(const char* root) { return maca_cat_own(maca_cat("\nBeside `std`, the packages under `modules` ride in the same tree. Run ", "`maca spec --package <name>` for one of their indexes.\n\n"), package_lines(root, maca_list_dir(maca_cat(root, "modules")), 0, ""), 1);  }
const char* package_lines(const char* root, MacaList names, long i, const char* acc) { return ((i >= (names.len)) ? acc : (((strcmp(((const char*)names.data[i]), "std") == 0) || (!maca_is_dir(maca_cat_own(maca_cat_own(maca_cat("", root), "modules/", 1), ((const char*)names.data[i]), 1)))) ? package_lines(root, names, (i + 1), acc) : package_lines(root, names, (i + 1), maca_cat_own(maca_cat_own(maca_cat_own(acc, maca_cat_own(maca_cat("- `", ((const char*)names.data[i])), "`: ", 1), 2), blurb_of(root, ((const char*)names.data[i])), 1), "\n", 1))));  }
const char* blurb_of(const char* root, const char* pkg) { const char* dir = maca_cat_own(maca_cat_own(maca_cat("", root), "modules/", 1), pkg, 1); const char* first = first_maca(maca_list_dir(dir), 0); return ((strcmp(first, "") == 0) ? Carried : blurb(maca_read_file(maca_cat_own(maca_cat_own(maca_cat("", dir), "/", 1), first, 1))));  }
const char* first_maca(MacaList names, long i) { return ((i >= (names.len)) ? "" : (maca_ends_with(((const char*)names.data[i]), ".maca") ? ((const char*)names.data[i]) : first_maca(names, (i + 1))));  }
const char* blurb(const char* src) { MacaList lines = maca_split(src, "\n"); const char* head = (((lines.len) == 0) ? "" : maca_trim(((const char*)lines.data[0]))); return ((maca_starts_with(head, "//") && (!maca_starts_with(head, "///"))) ? maca_trim(maca_str_slice(head, 2, ((int)strlen(head)))) : Carried);  }
const char* mistakes() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\nThese are the five the checker sees most, and it rejects each by ", "name.\n\n"), "- **No `let`.** write `x = e` for a variable, `const x = e` for a ", 1), "constant; no `let`/`var` keyword.\n", 1), "- **No `fn`.** write the signature straight out: ", 1), "`name(arg: T) -> R { … }` or `name(arg: T) -> R => e`; no `fn` ", 1), "keyword.\n", 1), "- **No `type`.** declare a type by binding it: ", 1), "`Name = { field: T }` for a record, `Name = A | B` for a sum; no ", 1), "`type` keyword.\n", 1), "- **No `async`.** async is an inferred effect, so any function can ", 1), "`spawn` and `await`; no `async` keyword to write.\n", 1), "- **No `null`.** a sum type with an empty variant says what the ", 1), "absence means, and `match` makes you handle it; Maca has no null.\n", 1), "\nTwo more that parse but mean something else:\n\n", 1), "- **`lo..hi` excludes `hi`.** `0..xs.length()` is every index of ", 1), "`xs`. Writing `0..xs.length() - 1` misses the last element.\n", 1), "- **A block after `=>` is a record literal.** `f() => { a = 1 }` ", 1), "builds a record. Use `f() { … }` when you meant a body.\n", 1);  }
const char* targets_table() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\n`maca check --target <t>` refuses a program that performs an effect ", "its target cannot carry. With no `--target` a program is held to "), "`native`, which is what `maca build` produces.\n\n", 1), "| target | flag | carries |\n|---|---|---|\n", 1), "| native | (default) | io, net, os, async, exn |\n", 1), "| js | `--target js` | io, net, async, exn |\n", 1), "| jvm | `--target jvm` | io, net, os, async, exn |\n", 1), "| rust | `--target rust` | io, net, os, async, exn |\n", 1), "| embedded | `--target embedded` | exn |\n", 1), "| nix | `--target nix` | nothing: config mode is data |\n", 1), "\nThere is no BEAM target, by design.\n\n", 1), "Two differences are not effects, and are the back end's own error: ", 1), "`int / int` truncates natively and does not on `js`, and a ", 1), "function-typed record field is refused on `rust` and `jvm`.\n", 1);  }
const char* builtin_methods() { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("\nThese are builtins on the receiver, not module exports: write ", "`s.trim()`, never `import { trim } from std/text`.\n\n"), "- **`str`**: `length`, `split`, `trim`, `upper`, `lower`, ", 1), "`contains`, `starts_with`, `ends_with`, `replace`, `substr`, ", 1), "`slice`, `index_of`, `repeat`, `pad_start`, `pad_end`, ", 1), "`pad_center`, `chars`, `at`, `is_whitespace`, `is_ascii_digit`, ", 1), "`is_alpha`\n", 1), "- **`T[]`**: `map`, `filter`, `reduce`, `fold`, `sort`, `sort_by`, ", 1), "`reverse`, `push`, `pop`, `set`, `insert`, `remove`, `slice`, ", 1), "`contains`, `index_of`, `index_of_by`, `enumerate`, `sum`, `min`, ", 1), "`max`, `first`, `last`, `get`, `length`, `parallel`, `join`\n", 1), "- **`Map k v`**: `set`, `get`, `has`, `remove`, `keys`, `length`\n", 1);  }
long profile_cmd(MacaList args) { const char* src = build_src(args, 1, ""); const char* out = flag_after(args, "-o"); return ((strcmp(src, "") == 0) ? ({ maca_say(stderr, "profile: expected a .maca file", "\n", 0); 2; }) : profile_built(src, ((strcmp(out, "") == 0) ? maca_cat_own(maca_cat("", stem_of(src)), ".svg", 1) : out)));  }
long profile_built(const char* src, const char* svg) { const char* bin = scratch_path("profile"); long built = build_binary(src, bin); return ((built != 0) ? built : profile_measured(src, bin, svg));  }
long profile_measured(const char* src, const char* bin, const char* svg) { const char* cg = maca_cat_own(maca_cat("", bin), ".callgrind", 1); maca_say(stdout, maca_cat_own(maca_cat("profiling ", src), " under callgrind, which is slower than a plain run", 1), "\n", 1); maca_exec("valgrind", maca_listv(3, (long)("--tool=callgrind"), (long)(maca_cat("--callgrind-out-file=", cg)), (long)(bin))); const char* dump = maca_read_file(cg); maca_remove_file(bin); maca_remove_file(maca_cat_own(maca_cat("", bin), ".c", 1)); maca_remove_file(cg); return ((strcmp(dump, "") == 0) ? ({ maca_say(stderr, "profile: callgrind wrote nothing; valgrind has to be on PATH", "\n", 0); 2; }) : profile_reported(src, costs_in(dump), svg));  }
long profile_reported(const char* src, Dump d, const char* svg) { maca_say(stdout, cost_table(d), "\n", 0); maca_write_file(svg, flame_svg_in(trace(src, frames_of(d)), ProfileWidth, "Ir")); maca_say(stdout, maca_cat("flame graph -> ", svg), "\n", 1); return 0;  }
Dump costs_in(const char* text) { MacaList lines = maca_split(text, "\n"); MacaList table = name_table(lines); Scan s = (Scan){ .d = (Dump){ .fns = maca_listv(0), .own = maca_listv(0), .edges = maca_listv(0), .costs = maca_listv(0), .total = 0 }, .here = "", .callee = "" }; long i = 0; while ((i < (lines.len))) { s = one_line(table, ((const char*)lines.data[i]), s); i = (i + 1); } return ({ __typeof__(s.d) _w = s.d; _w.total = ((s.d.total < 1) ? 1 : s.d.total); _w; });  }
Scan one_line(MacaList table, const char* line, Scan s) { return (maca_starts_with(line, "fn=") ? ({ __typeof__(s) _w = s; _w.here = spec_name(table, maca_str_slice(line, 3, ((int)strlen(line)))); _w.callee = ""; _w; }) : (maca_starts_with(line, "cfn=") ? ({ __typeof__(s) _w = s; _w.callee = spec_name(table, maca_str_slice(line, 4, ((int)strlen(line)))); _w; }) : (((strcmp(s.here, "") == 0) || (!is_cost_line(line))) ? s : ((strcmp(s.callee, "") != 0) ? ({ __typeof__(s) _w = s; _w.d = charged(s.d, maca_cat_own(maca_cat_own(maca_cat("", s.here), "\n", 1), s.callee, 1), cost_in(line)); _w.callee = ""; _w; }) : ({ __typeof__(s) _w = s; _w.d = owned(s.d, s.here, cost_in(line)); _w; })))));  }
Dump charged(Dump d, const char* key, long ir) { long at = maca_list_index_of_str(d.edges, key); return ((at < 0) ? ({ __typeof__(d) _w = d; _w.edges = maca_list_pushed(d.edges, (long)(key)); _w.costs = maca_list_pushed(d.costs, (long)(ir)); _w; }) : ({ __typeof__(d) _w = d; _w.costs = maca_list_set(d.costs, at, (long)((((long)d.costs.data[at]) + ir))); _w; }));  }
Dump owned(Dump d, const char* name, long ir) { long at = maca_list_index_of_str(d.fns, name); return ((at < 0) ? ({ __typeof__(d) _w = d; _w.fns = maca_list_pushed(d.fns, (long)(name)); _w.own = maca_list_pushed(d.own, (long)(ir)); _w.total = (d.total + ir); _w; }) : ({ __typeof__(d) _w = d; _w.own = maca_list_set(d.own, at, (long)((((long)d.own.data[at]) + ir))); _w.total = (d.total + ir); _w; }));  }
long is_cost_line(const char* line) { const char* c = maca_str_slice(line, 0, 1); return ((((isdigit((unsigned char)(c)[0]) != 0) || (strcmp(c, "*") == 0)) || (strcmp(c, "+") == 0)) || (strcmp(c, "-") == 0));  }
long cost_in(const char* line) { MacaList fs = nonempty(maca_split(line, " "), 0, maca_listv(0)); return (((fs.len) < 2) ? 0 : atol(((const char*)fs.data[1])));  }
MacaList name_table(MacaList lines) { MacaList acc = maca_listv(0); long i = 0; while ((i < (lines.len))) { acc = with_spec(acc, name_spec(((const char*)lines.data[i]))); i = (i + 1); } return acc;  }
MacaList with_spec(MacaList acc, const char* spec) { return ((strcmp(spec, "") == 0) ? acc : maca_list_pushed(acc, (long)(spec)));  }
const char* name_spec(const char* line) { const char* body = fn_spec(line); long close_mc = maca_str_index_of(body, ")"); const char* tail = ((close_mc < 0) ? "" : maca_trim(maca_str_slice(body, (close_mc + 1), ((int)strlen(body))))); return (((!maca_starts_with(body, "(")) || (strcmp(tail, "") == 0)) ? "" : maca_cat_own(maca_cat_own(maca_cat_own("", maca_str_slice(body, 1, close_mc), 2), "\n", 1), tail, 1));  }
const char* fn_spec(const char* line) { return (maca_starts_with(line, "fn=") ? maca_trim(maca_str_slice(line, 3, ((int)strlen(line)))) : (maca_starts_with(line, "cfn=") ? maca_trim(maca_str_slice(line, 4, ((int)strlen(line)))) : ""));  }
const char* spec_name(MacaList table, const char* spec) { const char* s = maca_trim(spec); long close_mc = maca_str_index_of(s, ")"); const char* tail = ((close_mc < 0) ? "" : maca_trim(maca_str_slice(s, (close_mc + 1), ((int)strlen(s))))); return (((!maca_starts_with(s, "(")) || (close_mc < 0)) ? s : ((strcmp(tail, "") != 0) ? tail : table_name(table, maca_str_slice(s, 1, close_mc), 0)));  }
const char* table_name(MacaList table, const char* id, long i) { return ((i >= (table.len)) ? "" : ((strcmp(((const char*)maca_split(((const char*)table.data[i]), "\n").data[0]), id) == 0) ? maca_str_slice(((const char*)table.data[i]), (((int)strlen(id)) + 1), ((int)strlen(((const char*)table.data[i])))) : table_name(table, id, (i + 1))));  }
const char* cost_table(Dump d) { return maca_list_join(cost_rows(d, own_order(d, 0, maca_listv(0)), 0, maca_listv(1, (long)(maca_cat_own(maca_cat_own("  self%  ", maca_pad_start("Ir", 9, " "), 2), "  function", 1)))), "\n");  }
MacaList cost_rows(Dump d, MacaList order, long i, MacaList acc) { return ((((i >= (order.len)) || (i >= ProfileRows)) || (((long)d.own.data[((long)order.data[i])]) == 0)) ? acc : cost_rows(d, order, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(cost_row(d, ((long)order.data[i])))))));  }
const char* cost_row(Dump d, long at) { double share = percent(((long)d.own.data[at]), d.total); return maca_cat_own(maca_cat_own(maca_cat_own(maca_pad_end(maca_cat_own("  ", maca_fixed(share, 1), 2), 9, " "), maca_pad_start(maca_int_to_str(((long)d.own.data[at])), 9, " "), 3), "  ", 1), ((const char*)d.fns.data[at]), 1);  }
MacaList own_order(Dump d, long i, MacaList acc) { return ((i >= (d.fns.len)) ? acc : own_order(d, (i + 1), ranked(d, acc, i)));  }
MacaList ranked(Dump d, MacaList acc, long at) { long cut = rank_in(d, acc, ((long)d.own.data[at]), 0); return maca_list_cat(maca_list_pushed(maca_list_slice(acc, 0, cut), (long)(at)), maca_list_slice(acc, cut, (acc.len)));  }
long rank_in(Dump d, MacaList acc, long own, long i) { return ((i >= (acc.len)) ? i : ((((long)d.own.data[((long)acc.data[i])]) < own) ? i : rank_in(d, acc, own, (i + 1))));  }
MacaList frames_of(Dump d) { const char* root = root_name(d); return ((strcmp(root, "") == 0) ? maca_listv(0) : frames_at(d, (Call){ .to = root, .ir = inclusive_of(d, root) }, (Frame){ .depth = 0, .x = 0, .parent = (-1) }, maca_listv(0), maca_listv(0)));  }
MacaList frames_at(Dump d, Call f, Frame at, MacaList path, MacaList acc) { long mine = (acc.len); MacaList kids = (((at.depth + 1) >= ProfileDepth) ? maca_listv(0) : calls_of(d, f.to)); Span box = (Span){ .name = f.to, .start = at.x, .end = (at.x + f.ir), .depth = at.depth, .parent = at.parent, .closed = 1 }; return laid_out(d, kids, 0, (at.x + f.ir), (Frame){ .depth = (at.depth + 1), .x = at.x, .parent = mine }, maca_list_cat(path, maca_listv(1, (long)(f.to))), maca_list_pushed(acc, maca_box(sizeof(Span), (Span[]){ box })));  }
MacaList laid_out(Dump d, MacaList kids, long i, long stop, Frame at, MacaList path, MacaList acc) { return (((i >= (kids.len)) || (at.x >= stop)) ? acc : ((((*(Call*)kids.data[i]).ir <= 0) || (maca_list_index_of_str(path, (*(Call*)kids.data[i]).to) >= 0)) ? laid_out(d, kids, (i + 1), stop, at, path, acc) : ({ long wide = (((*(Call*)kids.data[i]).ir < (stop - at.x)) ? (*(Call*)kids.data[i]).ir : (stop - at.x)); laid_out(d, kids, (i + 1), stop, ({ __typeof__(at) _w = at; _w.x = (at.x + wide); _w; }), path, frames_at(d, (Call){ .to = (*(Call*)kids.data[i]).to, .ir = wide }, at, path, acc)); })));  }
const char* root_name(Dump d) { return ((maca_list_index_of_str(d.fns, "main") >= 0) ? "main" : (((d.fns.len) == 0) ? "" : ((const char*)d.fns.data[heaviest(d, 0, 0)])));  }
long heaviest(Dump d, long i, long best) { return ((i >= (d.fns.len)) ? best : ((((long)d.own.data[i]) > ((long)d.own.data[best])) ? heaviest(d, (i + 1), i) : heaviest(d, (i + 1), best)));  }
long inclusive_of(Dump d, const char* name) { return (own_of(d, name) + edge_sum(d, maca_cat_own(maca_cat("", name), "\n", 1), 0, 0));  }
long own_of(Dump d, const char* name) { long at = maca_list_index_of_str(d.fns, name); return ((at < 0) ? 0 : ((long)d.own.data[at]));  }
long edge_sum(Dump d, const char* from, long i, long acc) { return ((i >= (d.edges.len)) ? acc : (maca_starts_with(((const char*)d.edges.data[i]), from) ? edge_sum(d, from, (i + 1), (acc + ((long)d.costs.data[i]))) : edge_sum(d, from, (i + 1), acc)));  }
MacaList calls_of(Dump d, const char* from) { return heavy_first(calls_in(d, maca_cat_own(maca_cat("", from), "\n", 1), 0, maca_listv(0)), 0, maca_listv(0));  }
MacaList calls_in(Dump d, const char* from, long i, MacaList acc) { return ((i >= (d.edges.len)) ? acc : (maca_starts_with(((const char*)d.edges.data[i]), from) ? calls_in(d, from, (i + 1), maca_list_pushed(acc, maca_box(sizeof(Call), (Call[]){ one_call(d, from, i) }))) : calls_in(d, from, (i + 1), acc)));  }
Call one_call(Dump d, const char* from, long i) { return (Call){ .to = maca_str_slice(((const char*)d.edges.data[i]), ((int)strlen(from)), ((int)strlen(((const char*)d.edges.data[i])))), .ir = ((long)d.costs.data[i]) };  }
MacaList heavy_first(MacaList cs, long i, MacaList acc) { return ((i >= (cs.len)) ? acc : heavy_first(cs, (i + 1), call_ranked(acc, (*(Call*)cs.data[i]))));  }
MacaList call_ranked(MacaList acc, Call c) { long cut = call_rank(acc, c.ir, 0); return maca_list_cat(maca_list_pushed(maca_list_slice(acc, 0, cut), maca_box(sizeof(Call), (Call[]){ c })), maca_list_slice(acc, cut, (acc.len)));  }
long call_rank(MacaList acc, long ir, long i) { return ((i >= (acc.len)) ? i : (((*(Call*)acc.data[i]).ir < ir) ? i : call_rank(acc, ir, (i + 1))));  }
long dev_cmd(MacaList args) { const char* named = build_src(args, 1, ""); const char* src = ((strcmp(named, "") == 0) ? DevFile : named); const char* out = flag_after(args, "-o"); return ((!maca_file_exists(src)) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("dev: no ", src), " here. It describes the dev shell, in Maca:\n\n", 1), StarterDev, 1), "\nThen `maca dev` writes it and `nix develop` enters it.", 1), "\n", 1); 2; }) : dev_written(src, ((strcmp(out, "") == 0) ? "flake.nix" : out)));  }
long dev_written(const char* src, const char* out) { Module raw = parse_module(lex(maca_read_file(src)), 0, maca_listv(0)); long bad = report_all("parse", raw.errors, 0); return ((bad > 0) ? bad : dev_flake_out(raw, out));  }
long dev_flake_out(Module m, const char* out) { maca_write_file(out, dev_flake(m)); maca_say(stdout, maca_cat_own(maca_cat("wrote ", out), "; run `nix develop` to enter the shell", 1), "\n", 1); return 0;  }
const char* dev_flake(Module m) { return flake_text(dev_name(m.items), maca_cat_own(maca_cat(dev_packages(m.items), dev_env(m.items)), dev_hook(m.items), 1));  }
long dev_at(MacaList items, const char* name, long i) { return ((i >= (items.len)) ? (-1) : ((((*(Stmt*)items.data[i]).kind == SBind) && (strcmp((*(Stmt*)items.data[i]).name, name) == 0)) ? i : dev_at(items, name, (i + 1))));  }
const char* dev_name(MacaList items) { long at = dev_at(items, "dev.name", 0); return (((at < 0) || ((*(Stmt*)items.data[at]).value.kind != EStr)) ? "dev" : (*(Stmt*)items.data[at]).value.text);  }
const char* dev_packages(MacaList items) { long at = dev_at(items, "dev.packages", 0); const char* ps = ((at < 0) ? "[ ]" : nix_pkg_list((*(Stmt*)items.data[at]).value)); return maca_cat_own(maca_cat("          packages = ", ps), ";\n", 1);  }
const char* dev_env(MacaList items) { long at = dev_at(items, "dev.env", 0); return (((at < 0) || ((*(Stmt*)items.data[at]).value.kind != ERecord)) ? "" : dev_lines(nix_fields((*(Stmt*)items.data[at]).value.children, 0, maca_listv(0)), 0, ""));  }
const char* dev_lines(MacaList ls, long i, const char* acc) { return ((i >= (ls.len)) ? acc : dev_lines(ls, (i + 1), maca_cat_own(maca_cat_own(maca_cat(acc, "          "), ((const char*)ls.data[i]), 1), "\n", 1)));  }
const char* dev_hook(MacaList items) { long at = dev_at(items, "dev.shellHook", 0); long fell = ((at < 0) ? dev_at(items, "dev.shell_hook", 0) : at); return (((fell < 0) || ((*(Stmt*)items.data[fell]).value.kind != EStr)) ? "" : maca_cat_own(maca_cat("          shellHook = ", nix_string((*(Stmt*)items.data[fell]).value.text)), ";\n", 1));  }
const char* flake_text(const char* name, const char* shell) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("{\n  description = \"", name), " dev environment, written by `maca dev`\";\n\n", 1), "  inputs.nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n\n", 1), "  outputs = { self, nixpkgs }:\n    let\n", 1), "      systems = [ \"x86_64-linux\" \"aarch64-linux\" ", 1), "\"x86_64-darwin\" \"aarch64-darwin\" ];\n", 1), "      forAllSystems = f: nixpkgs.lib.genAttrs systems ", 1), "(system: f nixpkgs.legacyPackages.${system});\n", 1), "    in {\n      devShells = forAllSystems (pkgs: {\n", 1), "        default = pkgs.mkShell {\n", 1), shell, 1), "        };\n      });\n    };\n}\n", 1);  }
long add_cmd(MacaList args) { MacaList specs = nonflags(args, 1, maca_listv(0)); return (((specs.len) == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat("usage: maca add <spec>...\n  e.g. maca add npm:axios\n", "       maca add git+https://github.com/u/lib#main\n"), "       maca add utils@^1.2.0", 1), "\n", 1); 2; }) : ({ write_absent(Manifest, "[package]\nname = \"app\"\n\n[dependencies]\n"); added(specs, 0, registry_url(), 0); }));  }
long added(MacaList specs, long i, const char* registry, long bad) { return ((i >= (specs.len)) ? ((bad > 0) ? 1 : 0) : added(specs, (i + 1), registry, (bad + add_one(((const char*)specs.data[i]), registry))));  }
long add_one(const char* spec, const char* registry) { Spec s = parse_spec(spec); Pin p = (bad_name(s.name) ? failed(maca_cat_own(maca_cat("`", spec), "` names no package", 1)) : brought(s, spec, registry, unpinned())); return ((strcmp(p.why, "") != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca add ", spec), ": ", 1), p.why, 1), "\n", 1); 1; }) : ({ manifest_put(s.name, spec); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("added ", s.name), "@", 1), p.version, 1), " -> ", 1), DepsDir, 1), "/", 1), s.name, 1), "\n", 1); 0; }));  }
long install_cmd() { MacaList deps = manifest_deps(); return (((deps.len) == 0) ? ({ maca_say(stdout, maca_cat("no dependencies in ", Manifest), "\n", 1); 0; }) : install_all(deps, 0, registry_url(), 0));  }
long install_all(MacaList deps, long i, const char* registry, long bad) { return ((i >= (deps.len)) ? ((bad > 0) ? 1 : 0) : install_all(deps, (i + 1), registry, (bad + install_one((*(Dep*)deps.data[i]), registry))));  }
long install_one(Dep d, const char* registry) { return (maca_is_dir(maca_cat_own(maca_cat_own(maca_cat("", DepsDir), "/", 1), d.name, 1)) ? ({ maca_say(stdout, maca_cat_own(maca_cat("", d.name), " is already there", 1), "\n", 1); 0; }) : installed(d, brought(parse_spec(d.spec), d.spec, registry, pinned(lock_read(), d.name, d.spec))));  }
long installed(Dep d, Pin p) { return ((strcmp(p.why, "") != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca install ", d.name), ": ", 1), p.why, 1), "\n", 1); 1; }) : ({ maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("installed ", d.name), "@", 1), p.version, 1), " -> ", 1), DepsDir, 1), "/", 1), d.name, 1), "\n", 1); 0; }));  }
long update_cmd() { MacaList deps = manifest_deps(); return (((deps.len) == 0) ? ({ maca_say(stdout, maca_cat("no dependencies in ", Manifest), "\n", 1); 0; }) : update_all(deps, 0, registry_url(), 0));  }
long update_all(MacaList deps, long i, const char* registry, long bad) { return ((i >= (deps.len)) ? ((bad > 0) ? 1 : 0) : update_all(deps, (i + 1), registry, (bad + update_one((*(Dep*)deps.data[i]), registry))));  }
long update_one(Dep d, const char* registry) { Pin p = brought(parse_spec(d.spec), d.spec, registry, unpinned()); return ((strcmp(p.why, "") != 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat("maca update ", d.name), ": ", 1), p.why, 1), "\n", 1); 1; }) : ({ maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat("", d.name), " -> ", 1), p.version, 1), "\n", 1); 0; }));  }
Pin brought(Spec s, const char* spec, const char* registry, Pin want) { Pin p = ((strcmp(want.version, "") == 0) ? resolve_dep(s, registry) : want); return ((strcmp(p.why, "") != 0) ? p : landed(s, spec, p));  }
Pin landed(Spec s, const char* spec, Pin p) { const char* why = fetch(s, p); return ((strcmp(why, "") != 0) ? failed(why) : ({ lock_put(s, spec, p); p; }));  }
long upgrade_cmd() { const char* doc = http_get(Releases); const char* tag = json_str(doc, "tag_name"); const char* want = (maca_starts_with(tag, "v") ? maca_str_slice(tag, 1, ((int)strlen(tag))) : tag); return ((strcmp(doc, "") == 0) ? ({ maca_say(stderr, maca_cat("maca upgrade: cannot reach GitHub releases\n  install by hand: ", ReleasePage), "\n", 1); 1; }) : ((strcmp(want, "") == 0) ? ({ maca_say(stderr, maca_cat("maca upgrade: no published release yet\n  build from a checkout: ", "maca build apps/maca1/main.maca -o maca"), "\n", 1); 1; }) : ((strcmp(want, Version) == 0) ? ({ maca_say(stdout, maca_cat_own(maca_cat("maca ", Version), " is already the latest release", 1), "\n", 1); 0; }) : replaced(want, asset_url(doc, maca_cat("maca-", host_triple()))))));  }
const char* asset_url(const char* doc, const char* want) { const char* key = "\"browser_download_url\""; long at = maca_str_index_of(doc, key); return ((at < 0) ? "" : asset_pick(maca_str_slice(doc, (at + ((int)strlen(key))), ((int)strlen(doc))), want));  }
const char* asset_pick(const char* rest, const char* want) { const char* url = quoted_at(maca_chars(maca_str_slice(rest, 0, JsonWindow)), 0); return (maca_starts_with(seg_after(url, "/"), want) ? url : asset_url(rest, want));  }
long replaced(const char* want, const char* url) { const char* exe = maca_trim(maca_capture("sh", maca_listv(2, (long)("-c"), (long)("command -v maca")))); return ((strcmp(url, "") == 0) ? ({ maca_say(stderr, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("maca upgrade: release v", want), " has no asset for ", 1), host_triple(), 1), "\n", 1), "  build from a checkout: maca build apps/maca1/main.maca -o maca", 1), "\n", 1); 1; }) : ((strcmp(exe, "") == 0) ? ({ maca_say(stderr, maca_cat("maca upgrade: no `maca` on PATH to replace; take it by hand:\n  ", url), "\n", 1); 1; }) : swapped(exe, url, want)));  }
long swapped(const char* exe, const char* url, const char* want) { const char* tmp = maca_cat_own(maca_cat("", exe), ".new", 1); return ((maca_exec("curl", maca_listv(4, (long)("-fsSL"), (long)("-o"), (long)(tmp), (long)(url))) != 0) ? ({ maca_say(stderr, maca_cat("maca upgrade: download failed: ", url), "\n", 1); 1; }) : (((maca_exec("chmod", maca_listv(2, (long)("755"), (long)(tmp))) != 0) || (maca_exec("mv", maca_listv(2, (long)(tmp), (long)(exe))) != 0)) ? ({ maca_say(stderr, maca_cat("maca upgrade: cannot replace ", exe), "\n", 1); 1; }) : ({ maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat("upgraded maca ", Version), " -> ", 1), want, 1), "\n", 1); 0; })));  }
const char* host_triple() { const char* named = maca_trim(maca_capture("uname", maca_listv(1, (long)("-sm")))); const char* machine = (((maca_str_index_of(named, "arm64") >= 0) || (maca_str_index_of(named, "aarch64") >= 0)) ? "aarch64" : "x86_64"); return (maca_starts_with(named, "Darwin") ? maca_cat_own(maca_cat("", machine), "-macos", 1) : (maca_starts_with(named, "Linux") ? maca_cat_own(maca_cat("", machine), "-linux", 1) : "x86_64-windows"));  }
const char* json_str(const char* src, const char* key) { long at = maca_str_index_of(src, maca_cat_own(maca_cat("\"", key), "\"", 1)); long from = ((at + ((int)strlen(key))) + 2); return ((at < 0) ? "" : quoted_at(maca_chars(maca_str_slice(src, from, (from + JsonWindow))), 0));  }
const char* quoted_at(MacaList cs, long i) { return (((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), ",") == 0)) ? "" : ((strcmp(((const char*)cs.data[i]), "\"") == 0) ? upto_quote(cs, (i + 1), "") : quoted_at(cs, (i + 1))));  }
const char* upto_quote(MacaList cs, long i, const char* acc) { return (((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), "\"") == 0)) ? acc : upto_quote(cs, (i + 1), maca_cat(acc, ((const char*)cs.data[i]))));  }
MacaList nonflags(MacaList args, long i, MacaList acc) { return ((i >= (args.len)) ? acc : (maca_starts_with(((const char*)args.data[i]), "-") ? nonflags(args, (i + 1), acc) : nonflags(args, (i + 1), maca_list_cat(acc, maca_listv(1, (long)(((const char*)args.data[i])))))));  }
Spec parse_spec(const char* spec) { const char* s = maca_trim(spec); return (maca_starts_with(s, "npm:") ? npm_spec(maca_str_slice(s, 4, ((int)strlen(s)))) : (maca_starts_with(s, "git+") ? git_spec(maca_str_slice(s, 4, ((int)strlen(s)))) : reg_spec(s)));  }
Spec npm_spec(const char* rest) { long cut = version_cut(rest); const char* pkg = ((cut < 0) ? rest : maca_str_slice(rest, 0, cut)); return (Spec){ .name = seg_after(pkg, "/"), .kind = "npm", .pkg = pkg, .req = ((cut < 0) ? "latest" : maca_str_slice(rest, (cut + 1), ((int)strlen(rest)))) };  }
Spec git_spec(const char* rest) { long at = maca_str_index_of(rest, "#"); const char* url = ((at < 0) ? rest : maca_str_slice(rest, 0, at)); return (Spec){ .name = git_name(url), .kind = "git", .pkg = url, .req = ((at < 0) ? "" : maca_str_slice(rest, (at + 1), ((int)strlen(rest)))) };  }
Spec reg_spec(const char* s) { long cut = version_cut(s); const char* name = ((cut < 0) ? s : maca_str_slice(s, 0, cut)); return (Spec){ .name = name, .kind = "registry", .pkg = name, .req = ((cut < 0) ? "latest" : maca_str_slice(s, (cut + 1), ((int)strlen(s)))) };  }
long version_cut(const char* s) { long at = last_of(maca_chars(s), "@", (((int)strlen(s)) - 1)); return ((at > 0) ? at : (-1));  }
long last_of(MacaList cs, const char* want, long i) { return ((i < 0) ? (-1) : ((strcmp(((const char*)cs.data[i]), want) == 0) ? i : last_of(cs, want, (i - 1))));  }
const char* seg_after(const char* s, const char* sep) { return maca_str_slice(s, (last_of(maca_chars(s), sep, (((int)strlen(s)) - 1)) + 1), ((int)strlen(s)));  }
const char* git_name(const char* url) { const char* whole = (maca_ends_with(url, "/") ? maca_str_slice(url, 0, (((int)strlen(url)) - 1)) : url); const char* tail = seg_after(seg_after(whole, "/"), ":"); return (maca_ends_with(tail, ".git") ? maca_str_slice(tail, 0, (((int)strlen(tail)) - 4)) : tail);  }
long bad_name(const char* name) { return ((((strcmp(name, "") == 0) || (strcmp(name, ".") == 0)) || (strcmp(name, "..") == 0)) || (maca_str_index_of(name, "/") >= 0));  }
Ver ver_of(const char* text) { const char* t = maca_trim(text); const char* s = (maca_starts_with(t, "v") ? maca_str_slice(t, 1, ((int)strlen(t))) : t); MacaList parts = maca_split(maca_str_slice(s, 0, core_end(maca_chars(s), 0)), "."); return (((parts.len) > 3) ? bad_ver() : ver_parts(parts));  }
Ver bad_ver() { return (Ver){ .major = 0, .minor = 0, .patch = 0, .ok = 0 };  }
long core_end(MacaList cs, long i) { return ((((i >= (cs.len)) || (strcmp(((const char*)cs.data[i]), "-") == 0)) || (strcmp(((const char*)cs.data[i]), "+") == 0)) ? i : core_end(cs, (i + 1)));  }
Ver ver_parts(MacaList parts) { const char* a = ((const char*)parts.data[0]); const char* b = (((parts.len) > 1) ? ((const char*)parts.data[1]) : "0"); const char* c = (((parts.len) > 2) ? ((const char*)parts.data[2]) : "0"); return ((((!digits(a)) || (!digits(b))) || (!digits(c))) ? bad_ver() : (Ver){ .major = atol(a), .minor = atol(b), .patch = atol(c), .ok = 1 });  }
long digits(const char* s) { return ((strcmp(s, "") != 0) && all_digits(maca_chars(s), 0));  }
long all_digits(MacaList cs, long i) { return ((i >= (cs.len)) ? 1 : ((!(isdigit((unsigned char)(((const char*)cs.data[i]))[0]) != 0)) ? 0 : all_digits(cs, (i + 1))));  }
long ver_cmp(Ver a, Ver b) { return ((a.major != b.major) ? ((a.major < b.major) ? (-1) : 1) : ((a.minor != b.minor) ? ((a.minor < b.minor) ? (-1) : 1) : ((a.patch != b.patch) ? ((a.patch < b.patch) ? (-1) : 1) : 0)));  }
long satisfies(const char* version, const char* range) { Ver v = ver_of(version); return (v.ok && any_clause(v, maca_split(range, "||"), 0));  }
long any_clause(Ver v, MacaList clauses, long i) { return ((i >= (clauses.len)) ? 0 : (clause_holds(v, maca_trim(((const char*)clauses.data[i]))) ? 1 : any_clause(v, clauses, (i + 1))));  }
long clause_holds(Ver v, const char* clause) { return (((strcmp(clause, "") == 0) || (strcmp(clause, "*") == 0)) || every_part(v, nonempty(maca_split(clause, " "), 0, maca_listv(0)), 0));  }
long every_part(Ver v, MacaList parts, long i) { return ((i >= (parts.len)) ? 1 : ((!comparator(v, ((const char*)parts.data[i]))) ? 0 : every_part(v, parts, (i + 1))));  }
long comparator(Ver v, const char* part) { const char* c = maca_trim(part); return (maca_starts_with(c, "^") ? caret(v, ver_of(maca_str_slice(c, 1, ((int)strlen(c))))) : (maca_starts_with(c, "~") ? tilde(v, ver_of(maca_str_slice(c, 1, ((int)strlen(c))))) : ((maca_starts_with(c, ">=") || maca_starts_with(c, "<=")) ? cmp_op(v, maca_str_slice(c, 2, ((int)strlen(c))), maca_str_slice(c, 0, 2)) : (((maca_starts_with(c, ">") || maca_starts_with(c, "<")) || maca_starts_with(c, "=")) ? cmp_op(v, maca_str_slice(c, 1, ((int)strlen(c))), maca_str_slice(c, 0, 1)) : (((strcmp(c, "*") == 0) || (strcmp(c, "x") == 0)) ? 1 : (((maca_str_index_of(c, "x") >= 0) || (maca_str_index_of(c, "*") >= 0)) ? wild(v, maca_split(c, ".")) : exact_ver(v, ver_of(c))))))));  }
long caret(Ver v, Ver lo) { return ((lo.ok && (ver_cmp(v, lo) >= 0)) && (ver_cmp(v, caret_hi(lo)) < 0));  }
Ver caret_hi(Ver lo) { return ((lo.major > 0) ? (Ver){ .major = (lo.major + 1), .minor = 0, .patch = 0, .ok = 1 } : ((lo.minor > 0) ? (Ver){ .major = 0, .minor = (lo.minor + 1), .patch = 0, .ok = 1 } : (Ver){ .major = 0, .minor = 0, .patch = (lo.patch + 1), .ok = 1 }));  }
long tilde(Ver v, Ver lo) { return ((lo.ok && (ver_cmp(v, lo) >= 0)) && (ver_cmp(v, (Ver){ .major = lo.major, .minor = (lo.minor + 1), .patch = 0, .ok = 1 }) < 0));  }
long cmp_op(Ver v, const char* text, const char* op) { Ver w = ver_of(text); return ((!w.ok) ? 0 : ((strcmp(op, ">=") == 0) ? (ver_cmp(v, w) >= 0) : ((strcmp(op, "<=") == 0) ? (ver_cmp(v, w) <= 0) : ((strcmp(op, ">") == 0) ? (ver_cmp(v, w) > 0) : ((strcmp(op, "<") == 0) ? (ver_cmp(v, w) < 0) : (ver_cmp(v, w) == 0))))));  }
long exact_ver(Ver v, Ver w) { return (w.ok && (ver_cmp(v, w) == 0));  }
long wild(Ver v, MacaList parts) { return (wild_part(((const char*)parts.data[0]), v.major) && (((parts.len) < 2) || wild_part(((const char*)parts.data[1]), v.minor)));  }
long wild_part(const char* part, long have) { return (((strcmp(part, "x") == 0) || (strcmp(part, "*") == 0)) || (digits(part) && (atol(part) == have)));  }
const char* registry_url() { const char* named = toml_value(maca_read_file(Manifest), "[registry]", "url"); return ((strcmp(named, "") == 0) ? MacaRegistry : named);  }
Pin resolve_dep(Spec s, const char* registry) { return ((strcmp(s.kind, "git") == 0) ? git_pin(s) : ((strcmp(s.kind, "npm") == 0) ? registry_pin(NpmRegistry, s.pkg, s.req) : registry_pin(registry, s.pkg, s.req)));  }
Pin git_pin(Spec s) { const char* want = ((strcmp(s.req, "") == 0) ? "HEAD" : s.req); const char* sha = first_sha(maca_capture("git", maca_listv(3, (long)("ls-remote"), (long)(s.pkg), (long)(want)))); return ((strcmp(sha, "") == 0) ? failed(maca_cat_own(maca_cat_own(maca_cat("no ref `", want), "` in ", 1), s.pkg, 1)) : (Pin){ .version = ((strcmp(s.req, "") == 0) ? maca_str_slice(sha, 0, 12) : s.req), .tarball = "", .integrity = "", .commit = sha, .why = "" });  }
const char* first_sha(const char* out) { const char* t = maca_trim(out); long tab = maca_str_index_of(t, "\t"); return ((tab < 0) ? t : maca_str_slice(t, 0, tab));  }
Pin registry_pin(const char* base, const char* pkg, const char* req) { const char* tag = ((strcmp(req, "") == 0) ? "latest" : req); long ranged = ((strcmp(tag, "latest") != 0) && (!ver_of(tag).ok)); const char* asked = (ranged ? "latest" : tag); Pin p = pin_of(http_get(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", base), "/", 1), pkg, 1), "/", 1), asked, 1))); return ((strcmp(p.version, "") == 0) ? failed(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("no `", tag), "` of `", 1), pkg, 1), "` at ", 1), base, 1)) : (((!ranged) || satisfies(p.version, req)) ? p : failed(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("the newest `", pkg), "` is ", 1), p.version, 1), ", which does not satisfy ", 1), maca_cat_own(maca_cat("`", req), "`; name a version this registry publishes", 1), 3))));  }
Pin pin_of(const char* doc) { return (Pin){ .version = json_str(doc, "version"), .tarball = json_str(doc, "tarball"), .integrity = json_str(doc, "integrity"), .commit = "", .why = "" };  }
const char* http_get(const char* url) { return maca_capture("curl", maca_listv(4, (long)("-fsSL"), (long)("-A"), (long)("maca-cli"), (long)(url)));  }
Pin failed(const char* why) { return (Pin){ .version = "", .tarball = "", .integrity = "", .commit = "", .why = why };  }
Pin unpinned() { return failed("");  }
const char* fetch(Spec s, Pin p) { const char* dir = maca_cat_own(maca_cat_own(maca_cat("", DepsDir), "/", 1), s.name, 1); maca_remove_dir(dir); maca_make_dir(dir); return ((strcmp(s.kind, "git") == 0) ? git_into(s.pkg, p.commit, dir) : ((strcmp(p.tarball, "") == 0) ? maca_cat_own(maca_cat("the registry named no tarball for `", s.name), "`", 1) : tgz_into(p.tarball, p.integrity, dir)));  }
const char* git_into(const char* url, const char* sha, const char* dir) { return ((maca_exec("git", maca_listv(4, (long)("clone"), (long)("--quiet"), (long)(url), (long)(dir))) != 0) ? maca_cat_own(maca_cat("git clone ", url), " failed", 1) : (((strcmp(sha, "") != 0) && (maca_exec("git", maca_listv(5, (long)("-C"), (long)(dir), (long)("checkout"), (long)("--quiet"), (long)(sha))) != 0)) ? maca_cat_own(maca_cat("git checkout ", sha), " failed", 1) : ({ maca_remove_dir(maca_cat_own(maca_cat("", dir), "/.git", 1)); ""; })));  }
const char* tgz_into(const char* url, const char* integrity, const char* dir) { const char* tgz = maca_cat_own(maca_cat("", dir), "/.pkg.tgz", 1); return ((maca_exec("curl", maca_listv(4, (long)("-fsSL"), (long)("-o"), (long)(tgz), (long)(url))) != 0) ? maca_cat("download failed: ", url) : unpacked(tgz, integrity, dir));  }
const char* unpacked(const char* tgz, const char* integrity, const char* dir) { const char* why = ((strcmp(integrity, "") == 0) ? "" : mismatch(tgz, integrity)); return ((strcmp(why, "") != 0) ? ({ maca_remove_file(tgz); why; }) : ((maca_exec("tar", maca_listv(5, (long)("-xzf"), (long)(tgz), (long)("-C"), (long)(dir), (long)("--strip-components=1"))) != 0) ? maca_cat("cannot unpack ", tgz) : ({ maca_remove_file(tgz); ""; })));  }
const char* mismatch(const char* file, const char* want) { return ((!maca_starts_with(want, "sha512-")) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("integrity `", want), "` is not a sha512 digest; delete ", 1), LockFile, 1), " to ", 1), "re-resolve", 1) : digested(file, maca_str_slice(want, 7, ((int)strlen(want)))));  }
const char* digested(const char* file, const char* want) { const char* got = digest_of(file); const char* hex = hex_of(want); return ((strcmp(got, "") == 0) ? maca_cat_own(maca_cat("cannot take a sha512 of ", file), "; is sha512sum, shasum or openssl there?", 1) : ((strcmp(hex, "") == 0) ? maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("the integrity `sha512-", want), "` in ", 1), LockFile, 1), " is not base64", 1) : ((strcmp(got, hex) == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat("the download does not match what ", LockFile), " pinned\n", 1), maca_cat_own(maca_cat_own(maca_cat("  expected sha512 ", hex), "\n  got      sha512 ", 1), got, 1), 3))));  }
const char* digest_of(const char* file) { const char* tools = maca_cat_own(maca_cat("if command -v sha512sum >/dev/null; then sha512sum -b \"$0\";", " elif command -v shasum >/dev/null; then shasum -a 512 -b \"$0\";"), " else openssl dgst -sha512 -r \"$0\"; fi", 1); return maca_trim(maca_capture("sh", maca_listv(3, (long)("-c"), (long)(maca_cat_own(maca_cat("", tools), " | cut -d\" \" -f1", 1)), (long)(file))));  }
const char* hex_of(const char* b64) { return maca_trim(maca_capture("sh", maca_listv(3, (long)("-c"), (long)(maca_cat("printf %s \"$0\" | base64 -d 2>/dev/null", " | od -An -tx1 | tr -d \"[:space:]\"")), (long)(b64))));  }
MacaList manifest_deps() { MacaList lines = maca_split(maca_read_file(Manifest), "\n"); long at = table_at(lines, 0, "[dependencies]"); return ((at < 0) ? maca_listv(0) : deps_in(lines, (at + 1), block_end(lines, (at + 1)), maca_listv(0)));  }
MacaList deps_in(MacaList lines, long i, long end, MacaList acc) { return ((i >= end) ? acc : (((strcmp(toml_key(((const char*)lines.data[i])), "") == 0) || (strcmp(toml_val(((const char*)lines.data[i])), "") == 0)) ? deps_in(lines, (i + 1), end, acc) : deps_in(lines, (i + 1), end, maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Dep), (Dep[]){ (Dep){ .name = toml_key(((const char*)lines.data[i])), .spec = toml_val(((const char*)lines.data[i])) } }))))));  }
long manifest_put(const char* name, const char* spec) { MacaList lines = maca_split(maca_read_file(Manifest), "\n"); return maca_write_file(Manifest, maca_list_join(put_line(lines, name, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", name), " = \"", 1), spec, 1), "\"", 1)), "\n"));  }
MacaList put_line(MacaList lines, const char* name, const char* line) { long at = table_at(lines, 0, "[dependencies]"); return ((at < 0) ? maca_list_cat(started(lines), maca_listv(3, (long)("[dependencies]"), (long)(line), (long)(""))) : replaced_line(lines, (at + 1), name, line));  }
MacaList started(MacaList lines) { long n = (lines.len); return (((n > 0) && (strcmp(maca_trim(((const char*)lines.data[(n - 1)])), "") == 0)) ? maca_list_slice(lines, 0, (n - 1)) : lines);  }
MacaList replaced_line(MacaList lines, long from, const char* name, const char* line) { long end = tight_end(lines, from, block_end(lines, from)); long hit = key_at(lines, from, end, name); long cut = ((hit < 0) ? end : hit); long past = ((hit < 0) ? end : (hit + 1)); return maca_list_cat(maca_list_cat(maca_list_slice(lines, 0, cut), maca_listv(1, (long)(line))), maca_list_slice(lines, past, (lines.len)));  }
long table_at(MacaList lines, long i, const char* table) { return ((i >= (lines.len)) ? (-1) : ((strcmp(toml_head(((const char*)lines.data[i])), table) == 0) ? i : table_at(lines, (i + 1), table)));  }
long block_end(MacaList lines, long i) { return ((i >= (lines.len)) ? (lines.len) : ((strcmp(toml_head(((const char*)lines.data[i])), "") != 0) ? i : block_end(lines, (i + 1))));  }
long tight_end(MacaList lines, long from, long end) { return (((end <= from) || (strcmp(maca_trim(((const char*)lines.data[(end - 1)])), "") != 0)) ? end : tight_end(lines, from, (end - 1)));  }
long key_at(MacaList lines, long i, long end, const char* name) { return ((i >= end) ? (-1) : ((strcmp(toml_key(((const char*)lines.data[i])), name) == 0) ? i : key_at(lines, (i + 1), end, name)));  }
MacaList lock_read() { return lock_lines(maca_split(maca_read_file(LockFile), "\n"), 0, "", "", maca_listv(0));  }
MacaList lock_lines(MacaList lines, long i, const char* name, const char* block, MacaList acc) { return ((i >= (lines.len)) ? flushed(acc, name, block) : ((strcmp(toml_head(((const char*)lines.data[i])), "[[package]]") == 0) ? lock_lines(lines, (i + 1), "", "", flushed(acc, name, block)) : (noise(((const char*)lines.data[i])) ? lock_lines(lines, (i + 1), name, block, acc) : lock_lines(lines, (i + 1), lock_name(((const char*)lines.data[i]), name), maca_cat_own(maca_cat_own(block, maca_trim(((const char*)lines.data[i])), 2), "\n", 1), acc))));  }
MacaList flushed(MacaList acc, const char* name, const char* block) { return ((strcmp(name, "") == 0) ? acc : maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Pkg), (Pkg[]){ (Pkg){ .name = name, .block = block } }))));  }
long noise(const char* line) { const char* t = maca_trim(line); return ((strcmp(t, "") == 0) || maca_starts_with(t, "#"));  }
const char* lock_name(const char* line, const char* held) { return ((strcmp(toml_key(line), "name") == 0) ? toml_val(line) : held);  }
Pin pinned(MacaList entries, const char* name, const char* spec) { long at = pkg_at(entries, name, 0); const char* block = ((at < 0) ? "" : (*(Pkg*)entries.data[at]).block); return ((strcmp(toml_value(block, "", "request"), spec) != 0) ? unpinned() : (Pin){ .version = toml_value(block, "", "version"), .tarball = toml_value(block, "", "resolved"), .integrity = toml_value(block, "", "integrity"), .commit = toml_value(block, "", "commit"), .why = "" });  }
long pkg_at(MacaList entries, const char* name, long i) { return ((i >= (entries.len)) ? (-1) : ((strcmp((*(Pkg*)entries.data[i]).name, name) == 0) ? i : pkg_at(entries, name, (i + 1))));  }
long lock_put(Spec s, const char* spec, Pin p) { Pkg entry = (Pkg){ .name = s.name, .block = lock_block(s, spec, p) }; return maca_write_file(LockFile, lock_text(inserted(lock_read(), entry, 0, maca_listv(0)), 0, maca_cat_own(maca_cat_own(maca_cat("# ", LockFile), ": generated by `maca add`/`maca update`;", 1), " do not edit.\n", 1)));  }
const char* lock_block(Spec s, const char* spec, Pin p) { return maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("name = \"", s.name), "\"\nversion = \"", 1), p.version, 1), "\"\nrequest = \"", 1), spec, 1), "\"\n", 1), maca_cat_own(maca_cat("source = \"", lock_source(s)), "\"\n", 1), 3), lock_line("resolved", p.tarball), 1), lock_line("integrity", p.integrity), 1), lock_line("commit", p.commit), 1);  }
const char* lock_source(Spec s) { return ((strcmp(s.kind, "npm") == 0) ? maca_cat("npm:", s.name) : ((strcmp(s.kind, "git") == 0) ? maca_cat("git+", s.pkg) : "registry"));  }
const char* lock_line(const char* key, const char* value) { return ((strcmp(value, "") == 0) ? "" : maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("", key), " = \"", 1), value, 1), "\"\n", 1));  }
const char* lock_text(MacaList entries, long i, const char* acc) { return ((i >= (entries.len)) ? acc : lock_text(entries, (i + 1), maca_cat_own(maca_cat(acc, "\n[[package]]\n"), (*(Pkg*)entries.data[i]).block, 1)));  }
MacaList inserted(MacaList xs, Pkg p, long i, MacaList acc) { return ((i >= (xs.len)) ? maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Pkg), (Pkg[]){ p }))) : ((strcmp((*(Pkg*)xs.data[i]).name, p.name) == 0) ? inserted(xs, p, (i + 1), acc) : ((strcmp((*(Pkg*)xs.data[i]).name, p.name) > 0) ? maca_list_cat(maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Pkg), (Pkg[]){ p }))), maca_list_slice(xs, i, (xs.len))) : inserted(xs, p, (i + 1), maca_list_cat(acc, maca_listv(1, maca_box(sizeof(Pkg), (Pkg[]){ (*(Pkg*)xs.data[i]) })))))));  }
long demo() { const char* sample = "add(1) + 2 - 3"; MacaList toks = lex(sample); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own("scanned ", maca_int_to_str((toks.len)), 2), " tokens from ", 1), maca_int_to_str(((int)strlen(sample))), 3), " chars", 1), "\n", 1); PExpr tree = parse_expr(toks, 0); maca_say(stdout, maca_cat("parsed: ", show(tree.node)), "\n", 1); PExpr prec = parse_expr(lex("2 + 3 * 4"), 0); maca_say(stdout, maca_cat("precedence: ", show(prec.node)), "\n", 1); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("checked: type ", type_of(tree.node)), ", ", 1), maca_int_to_str(count_errors(tree.node)), 3), " errors", 1), "\n", 1); maca_say(stdout, maca_cat("emitted: ", emit_unit(tree.node)), "\n", 1); Expr bad = e_binary("+", e_str("hi"), e_int(1)); maca_say(stdout, maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat_own(maca_cat("checked: ", show(bad)), " → type ", 1), type_of(bad), 1), ", ", 1), maca_int_to_str(count_errors(bad)), 3), " errors", 1), "\n", 1); const char* fnsrc = "add(x: int, y: int) -> int => x + y"; PStmt fst = parse_fn(lex(fnsrc), 0); maca_say(stdout, maca_cat("fn: ", emit_fn(fst.snode)), "\n", 1); const char* prog = "inc(n: int) -> int => n + 1 dbl(n: int) -> int => n * 2"; Module mod = parse_module(lex(prog), 0, maca_listv(0)); maca_say(stdout, maca_cat_own(maca_cat_own("module (", maca_int_to_str((mod.items.len)), 2), " fns):", 1), "\n", 1); maca_say(stdout, emit_module(mod), "\n", 0); const char* prog2 = "ok() -> int => 1 + 2 bad() -> int => 1 + \"x\""; Module mod2 = parse_module(lex(prog2), 0, maca_listv(0)); maca_say(stdout, maca_cat_own(maca_cat_own("module check: ", maca_int_to_str(check_module(mod2)), 2), " type errors", 1), "\n", 1); const char* blk = "sq(n: int) -> int { t = n * n t }"; PStmt bfn = parse_fn(lex(blk), 0); maca_say(stdout, maca_cat("block fn: ", emit_fn(bfn.snode)), "\n", 1); PExpr two = parse_expr(lex("add(1, 2)"), 0); maca_say(stdout, maca_cat("multi-arg: ", show(two.node)), "\n", 1); PExpr tern = parse_expr(lex("a > b ? a : b"), 0); maca_say(stdout, maca_cat("ternary: ", show(tern.node)), "\n", 1); PExpr neg = parse_expr(lex("0 - -n"), 0); maca_say(stdout, maca_cat("unary: ", show(neg.node)), "\n", 1); PExpr md = parse_expr(lex("a % b + 1"), 0); maca_say(stdout, maca_cat("modulo: ", show(md.node)), "\n", 1); PExpr logic = parse_expr(lex("a < b && c || d"), 0); maca_say(stdout, maca_cat("logic: ", show(logic.node)), "\n", 1); PStmt sig = parse_fn(lex("tag(s: str, n: int) -> int => n"), 0); maca_say(stdout, maca_cat("typed sig C:    ", emit_fn(sig.snode)), "\n", 1); maca_say(stdout, maca_cat("typed sig Rust: ", remit_fn(sig.snode)), "\n", 1); PStmt bo = parse_fn(lex("flag() -> bool => true"), 0); maca_say(stdout, maca_cat("bool fn C:    ", emit_fn(bo.snode)), "\n", 1); maca_say(stdout, maca_cat("bool fn Rust: ", remit_fn(bo.snode)), "\n", 1); PStmt ffn = parse_fn(lex("scale(x: float) -> float => x * 2.0"), 0); maca_say(stdout, maca_cat("float fn C:    ", emit_fn(ffn.snode)), "\n", 1); maca_say(stdout, maca_cat("float fn Rust: ", remit_fn(ffn.snode)), "\n", 1); Module rec = parse_module(lex("Point = { x: int, y: int } sum(p: Point) -> int => p.x + p.y"), 0, maca_listv(0)); maca_say(stdout, maca_cat("record C:    ", emit_module(rec)), "\n", 1); maca_say(stdout, maca_cat("record Rust: ", remit_module(rec)), "\n", 1); Module su = parse_module(lex("Color = Red | Green | Blue rank(c: Color) -> int => c == Green ? 42 : 0"), 0, maca_listv(0)); maca_say(stdout, maca_cat("sum C:    ", emit_module(su)), "\n", 1); maca_say(stdout, maca_cat("sum Rust: ", remit_module(su)), "\n", 1); PStmt mt = parse_fn(lex("code(c: Color) -> int => match c { Red => 1 Green => 42 Blue => 3 }"), 0); maca_say(stdout, maca_cat("match C:    ", emit_fn(mt.snode)), "\n", 1); maca_say(stdout, maca_cat("match Rust: ", remit_fn(mt.snode)), "\n", 1); PStmt se = parse_fn(lex("kw(w: str) -> int => w == \"let\" ? 1 : 0"), 0); maca_say(stdout, maca_cat("streq C:    ", emit_fn(se.snode)), "\n", 1); maca_say(stdout, maca_cat("streq Rust: ", remit_fn(se.snode)), "\n", 1); PStmt ip = parse_fn(lex("label(n: int) -> str => \"n = {n}!\""), 0); maca_say(stdout, maca_cat("interp C:    ", emit_fn(ip.snode)), "\n", 1); maca_say(stdout, maca_cat("interp Rust: ", remit_fn(ip.snode)), "\n", 1); PExpr ip2 = parse_expr(lex("\"sum {a + b} done\""), 0); maca_say(stdout, maca_cat("interp expr: ", show(ip2.node)), "\n", 1); maca_say(stdout, maca_cat_own("check arity: ", maca_int_to_str(check_module(parse_module(lex("add(a: int, b: int) -> int => a + b use() -> int => add(1)"), 0, maca_listv(0)))), 2), "\n", 1); maca_say(stdout, maca_cat_own("check return: ", maca_int_to_str(check_module(parse_module(lex("bad() -> str => 1 + 2"), 0, maca_listv(0)))), 2), "\n", 1); maca_say(stdout, maca_cat_own("check calls: ", maca_int_to_str(check_module(parse_module(lex("n() -> int => 1 s() -> str => \"a\" mix() -> int => s() + 1"), 0, maca_listv(0)))), 2), "\n", 1); maca_say(stdout, maca_cat_own("check clean: ", maca_int_to_str(check_module(parse_module(lex("add(a: int, b: int) -> int => a + b use() -> int => add(1, 2)"), 0, maca_listv(0)))), 2), "\n", 1); Module sm = parse_module(lex("Shape = Circle(int) | Rect(int, int)"), 0, maca_listv(0)); maca_say(stdout, maca_cat("payload C:    ", emit_module(sm)), "\n", 1); maca_say(stdout, maca_cat("payload Rust: ", remit_module(sm)), "\n", 1); PStmt ar = parse_fn(lex("area(s: Shape) -> int => match s { Circle(r) => r * r Rect(w, h) => w * h }"), 0); maca_say(stdout, maca_cat("bind C:    ", emit_fn(ar.snode)), "\n", 1); maca_say(stdout, maca_cat("bind Rust: ", remit_fn(ar.snode)), "\n", 1); PStmt cc = parse_fn(lex("wrap(s: str) -> str => \"[\" ++ s ++ \"]\""), 0); maca_say(stdout, maca_cat("concat C:    ", emit_fn(cc.snode)), "\n", 1); maca_say(stdout, maca_cat("concat Rust: ", remit_fn(cc.snode)), "\n", 1); PStmt st = parse_fn(lex("numbered(n: int) -> str => \"#\" ++ str(n)"), 0); maca_say(stdout, maca_cat("str C:    ", emit_fn(st.snode)), "\n", 1); maca_say(stdout, maca_cat("str Rust: ", remit_fn(st.snode)), "\n", 1); PStmt lm = parse_fn(lex("slen(s: str) -> int => s.length()"), 0); maca_say(stdout, maca_cat("method C:    ", emit_fn(lm.snode)), "\n", 1); maca_say(stdout, maca_cat("method Rust: ", remit_fn(lm.snode)), "\n", 1); PStmt at = parse_fn(lex("first(s: str) -> int => s.at(0)"), 0); maca_say(stdout, maca_cat("at C:    ", emit_fn(at.snode)), "\n", 1); maca_say(stdout, maca_cat("at Rust: ", remit_fn(at.snode)), "\n", 1); PStmt arr = parse_fn(lex("total(xs: int[]) -> int => xs.get(0) + xs.get(1) + xs.count()"), 0); maca_say(stdout, maca_cat("array C:    ", emit_fn(arr.snode)), "\n", 1); maca_say(stdout, maca_cat("array Rust: ", remit_fn(arr.snode)), "\n", 1); const char* real = "import std/str import maca/token Kind = TInt | TStr classify(k: Kind) -> int => match k { TInt => 1 TStr => 2 }"; Module rm = parse_module(lex(real), 0, maca_listv(0)); maca_say(stdout, maca_cat_own(maca_cat_own("real file: ", maca_int_to_str((rm.items.len)), 2), " items after skipping imports", 1), "\n", 1); const char* program = "Point = { x: int, y: int } Color = Red | Green | Blue fld(p: Point) -> int => p.x code(c: Color) -> int => match c { Red => 1 Green => 2 Blue => 3 } Shape = Circle(int) | Rect(int, int) area(sh: Shape) -> int => match sh { Circle(r) => r * r Rect(w, h) => w * h } hi(a: str, b: str) -> int => (a ++ b) == \"hello\" ? 1 : 0 chk(n: int) -> int => (\"v\" ++ str(n)) == \"v42\" ? 1 : 0 slen(s: str) -> int => s.length() head(xs: int[]) -> int => xs.get(0) el(s: str) -> int => div(class = \"w\", s).length() == 22 ? 1 : 0 main() -> int { q = Point { x = 40, y = 9 } ys = [1, 99] info(\"self-hosted!\") (fld(q) + code(Green)) * hi(\"hel\", \"lo\") * chk(42) * slen(\"z\") * head(ys) * area(Rect(1, 1)) * el(\"x\") }"; Module prog_mod = annotated(parse_module(lex(program), 0, maca_listv(0))); maca_say(stdout, "=== emitted program ===", "\n", 0); maca_say(stdout, emit_module(prog_mod), "\n", 0); maca_say(stdout, "=== end program ===", "\n", 0); maca_say(stdout, "=== emitted rust ===", "\n", 0); maca_say(stdout, remit_module(prog_mod), "\n", 0); maca_say(stdout, "=== end rust ===", "\n", 0); return 0;  }
