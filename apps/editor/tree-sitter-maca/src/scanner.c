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
#ifdef _WIN32
#include <direct.h>
#include <process.h>
#include <stdlib.h>
#define maca_mkdir(p) _mkdir(p)
#define maca_gmtime(t, g) gmtime_s(g, t)
static char* maca_full_path(const char* p, char* r) { return _fullpath(r, p, 4096); }
#else
#define maca_mkdir(p) mkdir(p, 0777)
#define maca_gmtime(t, g) gmtime_r(t, g)
static char* maca_full_path(const char* p, char* r) { return realpath(p, r); }
#endif
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
static char* maca_int_to_str(long n) { char* r = malloc(24); snprintf(r, 24, "%ld", n); return r; }
static char* maca_float_to_str(double x) { char* r = malloc(32); if (x == (double)(long long)x && x < 1e15 && x > -1e15) snprintf(r, 32, "%.1f", x); else snprintf(r, 32, "%g", x); return r; }
static char* maca_fixed(double x, int n) { if (n < 0) n = 0; if (n > 17) n = 17; int need = snprintf(NULL, 0, "%.*f", n, x); char* r = malloc((size_t)need + 1); snprintf(r, (size_t)need + 1, "%.*f", n, x); return r; }
static const char* maca_bool_to_str(int b) { return b ? "true" : "false"; }
static char* maca_upper(const char* s) { size_t n = strlen(s); char* r = malloc(n + 1); for (size_t i = 0; i < n; i++) r[i] = toupper((unsigned char)s[i]); r[n] = 0; return r; }
typedef struct { long* data; int len; } MacaList;
typedef struct { long index_mc; long value; } MacaEntry;
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
static MacaList maca_listv(int n, ...) { MacaList l; l.data = n > 0 ? maca_cells(n * sizeof(long)) : 0; l.len = n; va_list ap; va_start(ap, n); for (int i = 0; i < n; i++) l.data[i] = va_arg(ap, long); va_end(ap); return l; }
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
static int maca_make_dir(const char* p) { char* d = maca_cat(p, ""); for (char* q = d + 1; *q; q++) if (*q == '/') { *q = 0; maca_mkdir(d); *q = '/'; } maca_mkdir(d); return maca_is_dir(d); }
static int maca_remove_file(const char* p) { return unlink(p) == 0; }
static int maca_remove_dir(const char* p) { DIR* d = opendir(p); if (!d) return 0; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, ".") == 0 || strcmp(it->d_name, "..") == 0) continue; char* c = maca_cat(p, maca_cat("/", it->d_name)); if (maca_is_dir(c)) maca_remove_dir(c); else maca_remove_file(c); } closedir(d); return rmdir(p) == 0; }
static int maca_copy_bytes(const char* src, const char* dst) { FILE* a = fopen(src, "rb"); if (!a) return 0; FILE* b = fopen(dst, "wb"); if (!b) { fclose(a); return 0; } char buf[8192]; size_t n; while ((n = fread(buf, 1, sizeof buf, a)) > 0) fwrite(buf, 1, n, b); fclose(a); fclose(b); return 1; }
static MacaList maca_list_dir(const char* p) { MacaList l; l.len = 0; int cap = 16; l.data = maca_cells((size_t)cap * sizeof(long)); DIR* d = opendir(p); if (!d) return l; struct dirent* it; while ((it = readdir(d))) { if (strcmp(it->d_name, ".") == 0 || strcmp(it->d_name, "..") == 0) continue; if (l.len == cap) { cap *= 2; long* g = maca_cells((size_t)cap * sizeof(long)); memcpy(g, l.data, (size_t)l.len * sizeof(long)); l.data = g; } l.data[l.len++] = (long)maca_cat(it->d_name, ""); } closedir(d); if (l.len > 1) qsort(l.data, (size_t)l.len, sizeof(long), maca_cmp_cell_str); return l; }
static char* maca_real_path(const char* p) { char* r = malloc(4096); if (!maca_full_path(p, r)) return maca_cat(p, ""); return r; }
static char* maca_path_join(const char* a, const char* b) { if (!*a) return maca_cat(b, ""); if (!*b) return maca_cat(a, ""); return a[strlen(a) - 1] == '/' ? maca_cat(a, b) : maca_cat(a, maca_cat("/", b)); }
static long maca_now_ms(void) { struct timespec ts; clock_gettime(CLOCK_REALTIME, &ts); return (long)(ts.tv_sec * 1000 + ts.tv_nsec / 1000000); }
static char* maca_now_iso(void) { time_t t = time(NULL); struct tm g; maca_gmtime(&t, &g); char* r = malloc(32); strftime(r, 32, "%Y-%m-%dT%H:%M:%SZ", &g); return r; }
static char* maca_format_time(long ms, const char* fmt) { time_t t = (time_t)(ms / 1000); struct tm g; maca_gmtime(&t, &g); char* r = malloc(128); if (!strftime(r, 128, fmt, &g)) r[0] = 0; return r; }
static void maca_sleep_ms(int ms) { if (ms > 0) usleep((unsigned)ms * 1000); }
static char* maca_input(const char* prompt) { if (prompt && *prompt) { printf("%s", prompt); fflush(stdout); } size_t cap = 128, n = 0; char* b = malloc(cap); int c; while ((c = fgetc(stdin)) != EOF && c != '\n') { if (n + 1 >= cap) { cap *= 2; char* g = malloc(cap); memcpy(g, b, n); b = g; } b[n++] = (char)c; } b[n] = 0; return b; }
static int maca_at_eof(void) { int c = fgetc(stdin); if (c == EOF) return 1; ungetc(c, stdin); return 0; }
static char* maca_attr(const char* name, const char* value) { if (!name || !*name) return maca_cat("", ""); size_t n = strlen(name), v = strlen(value); char* r = malloc(n + v * 6 + 5); char* w = r; *w++ = ' '; memcpy(w, name, n); w += n; *w++ = '='; *w++ = '"'; for (size_t i = 0; i < v; i++) { char c = value[i]; if (c == '&') { memcpy(w, "&amp;", 5); w += 5; } else if (c == '<') { memcpy(w, "&lt;", 4); w += 4; } else if (c == '>') { memcpy(w, "&gt;", 4); w += 4; } else if (c == '"') { memcpy(w, "&quot;", 6); w += 6; } else { *w++ = c; } } *w++ = '"'; *w = 0; return r; }
static char* maca_flag(const char* name, int on) { if (!on || !name || !*name) return maca_cat("", ""); return maca_cat(" ", name); }
static int maca_void_tag(const char* t) { const char* v[] = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr", 0}; for (int i = 0; v[i]; i++) if (strcmp(t, v[i]) == 0) return 1; return 0; }
static char* maca_element(const char* tag, const char* attrs, const char* kids) { size_t t = strlen(tag), a = strlen(attrs), k = strlen(kids); char* r = malloc(t * 2 + a + k + 6); char* w = r; *w++ = '<'; memcpy(w, tag, t); w += t; memcpy(w, attrs, a); w += a; *w++ = '>'; if (!maca_void_tag(tag)) { memcpy(w, kids, k); w += k; *w++ = '<'; *w++ = '/'; memcpy(w, tag, t); w += t; *w++ = '>'; } *w = 0; return r; }
static int maca_fail(const char* s) { fprintf(stderr, "error: %s\n", s ? s : ""); exit(1); return 0; }
static const long Space = 32;
static const long Tab = 9;
static const long Return = 13;
static const long Feed = 10;
static const long Newline = 0;
long ts_look();
long ts_take();
long ts_ok(long sym);
long ts_emit(long sym);
long blank(long c);
long continues_line(long c);
long scan_newline();
long blank(long c) { return ((((c == Space) || (c == Tab)) || (c == Return)) || (c == Feed));  }
long continues_line(long c) { return ((((((((((((((((c == 63) || (c == 58)) || (c == 43)) || (c == 45)) || (c == 42)) || (c == 37)) || (c == 60)) || (c == 62)) || (c == 61)) || (c == 38)) || (c == 124)) || (c == 46)) || (c == 41)) || (c == 93)) || (c == 125)) || (c == 44));  }
long scan_newline() { long crossed = 0; while (blank(ts_look())) { if ((ts_look() == Feed)) { crossed = 1; crossed; } ts_take(); } if (((!crossed) || (!ts_ok(Newline)))) { return 0; } if (continues_line(ts_look())) { return 0; } ts_emit(Newline); return 1;  }
#include "tree_sitter/parser.h"

static TSLexer *maca_ts_lexer;
static const bool *maca_ts_valid;

long ts_look(void) { return (long)maca_ts_lexer->lookahead; }
long ts_take(void) { maca_ts_lexer->advance(maca_ts_lexer, true); return 0; }
long ts_ok(long sym) { return maca_ts_valid[sym] ? 1 : 0; }
long ts_emit(long sym) { maca_ts_lexer->result_symbol = (TSSymbol)sym; return 0; }

void *tree_sitter_maca_external_scanner_create(void) { return NULL; }
void tree_sitter_maca_external_scanner_destroy(void *p) { (void)p; }
unsigned tree_sitter_maca_external_scanner_serialize(void *p, char *b) { (void)p; (void)b; return 0; }
void tree_sitter_maca_external_scanner_deserialize(void *p, const char *b, unsigned n) { (void)p; (void)b; (void)n; }

bool tree_sitter_maca_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  (void)payload;
  maca_ts_lexer = lexer;
  maca_ts_valid = valid_symbols;
  return scan_newline() != 0;
}
