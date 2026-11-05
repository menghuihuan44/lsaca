
/* ====================================================================== */
/* lsaca runtime I/O test suite                                           */
/*                                                                        */
/* Standalone in-process tests for the operating system I/O layer added   */
/* in runtime/lsaca_rt_io.c: the handle table, file and directory APIs,   */
/* TCP sockets, time and environment access.                              */
/*                                                                        */
/* Usage: io_tests.exe <scratch-directory>                                */
/*                                                                        */
/* The scratch directory is created (all parents) and removed by the      */
/* test; when no argument is given the current directory is used. The     */
/* program prints "ALL CHECKS PASSED (<n> checks)" and exits 0, or prints */
/* the failing checks and exits 1.                                        */
/* ====================================================================== */

#include "lsaca_rt.h"

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int lsaca_checks = 0;
static int lsaca_failures = 0;

static void report_check(bool condition, int line, const char* text) {
    lsaca_checks += 1;
    if (!condition) {
        lsaca_failures += 1;
        (void)fprintf(stdout, "FAIL line %d: %s\n", line, text);
    }
}

static void report_i64(int64_t actual, int64_t expected, int line,
                       const char* text) {
    lsaca_checks += 1;
    if (actual != expected) {
        lsaca_failures += 1;
        (void)fprintf(stdout, "FAIL line %d: %s (expected %lld, got %lld)\n",
                      line, text, (long long)expected, (long long)actual);
    }
}

#define CHECK(expression) report_check((expression), __LINE__, #expression)
#define CHECK_I64(actual, expected) \
    report_i64((int64_t)(actual), (int64_t)(expected), __LINE__, #actual)

/* Every public function resets the per-thread error, so a failed call can be
 * checked with its documented error kind. The message must be non-empty. */
static void check_error_message(int line) {
    LsString* message = ls_io_last_error_message();

    report_check(message != NULL, line, "error message is not null");
    if (message != NULL) {
        report_check(ls_str_len(message) > 0, line, "error message is not empty");
        ls_release(message);
    }
}

#define CHECK_ERROR(kind)                            \
    do {                                             \
        CHECK_I64(ls_io_last_error_kind(), (kind));  \
        check_error_message(__LINE__);               \
    } while (0)

/* --- Small path/handle helpers ------------------------------------------ */

static int64_t io_create(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t handle = ls_io_create(text);

    ls_release(text);
    return handle;
}

static int64_t io_open(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t handle = ls_io_open(text);

    ls_release(text);
    return handle;
}

static int64_t io_metadata(const char* path, int64_t* size, int64_t* mtime_ms,
                           int64_t* kind) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_metadata(text, size, mtime_ms, kind);

    ls_release(text);
    return result;
}

static int64_t io_remove_file(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_remove_file(text);

    ls_release(text);
    return result;
}

static int64_t io_remove_dir(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_remove_dir(text);

    ls_release(text);
    return result;
}

static int64_t io_remove_dir_all(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_remove_dir_all(text);

    ls_release(text);
    return result;
}

static int64_t io_create_dir(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_create_dir(text);

    ls_release(text);
    return result;
}

static int64_t io_create_dir_all(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_create_dir_all(text);

    ls_release(text);
    return result;
}

static int64_t io_rename(const char* from, const char* to) {
    LsString* from_text = ls_str_cstr(from);
    LsString* to_text = ls_str_cstr(to);
    int64_t result = ls_io_rename(from_text, to_text);

    ls_release(from_text);
    ls_release(to_text);
    return result;
}

static LsString* io_canonicalize(const char* path) {
    LsString* text = ls_str_cstr(path);
    LsString* result = ls_io_canonicalize(text);

    ls_release(text);
    return result;
}

static LsVec_String* io_read_dir(const char* path) {
    LsString* text = ls_str_cstr(path);
    LsVec_String* result = ls_io_read_dir(text);

    ls_release(text);
    return result;
}

static int64_t io_set_current_dir(const char* path) {
    LsString* text = ls_str_cstr(path);
    int64_t result = ls_io_set_current_dir(text);

    ls_release(text);
    return result;
}

static LsString* io_env_var(const char* name) {
    LsString* text = ls_str_cstr(name);
    LsString* result = ls_env_var(text);

    ls_release(text);
    return result;
}

static int64_t io_net_listen(const char* address) {
    LsString* text = ls_str_cstr(address);
    int64_t result = ls_net_listen(text);

    ls_release(text);
    return result;
}

static int64_t io_net_connect(const char* address) {
    LsString* text = ls_str_cstr(address);
    int64_t result = ls_net_connect(text);

    ls_release(text);
    return result;
}

static bool io_write_text(int64_t handle, const char* text, int64_t length) {
    return ls_io_write(handle, text, length) == length;
}

/* Case-insensitive prefix search: Windows environment variable names keep
 * their original casing (typically "Path=" rather than "PATH="). */
static bool vector_has_prefix(const LsVec_String* vector, const char* prefix) {
    size_t prefix_length = strlen(prefix);
    size_t index;

    for (index = 0; index < vector->len; index += 1) {
        const LsString* item = vector->data[index];
        size_t offset;
        bool match = ls_str_len(item) >= prefix_length;

        for (offset = 0; match && offset < prefix_length; offset += 1) {
            char actual = ls_str_data(item)[offset];
            char expected = prefix[offset];

            if (actual >= 'A' && actual <= 'Z') {
                actual = (char)(actual - 'A' + 'a');
            }
            if (expected >= 'A' && expected <= 'Z') {
                expected = (char)(expected - 'A' + 'a');
            }
            if (actual != expected) {
                match = false;
            }
        }
        if (match) {
            return true;
        }
    }
    return false;
}

static bool vector_name_at(const LsVec_String* vector, size_t index,
                           const char* expected) {
    LsString* text = ls_str_cstr(expected);
    bool equal = false;

    if (index < vector->len) {
        equal = ls_str_eq(vector->data[index], text);
    }
    ls_release(text);
    return equal;
}

static char scratch[1024];
static char file_a[1200];
static char file_b[1200];
static char dir_a[1200];
static char dir_b[1200];

static void build_path(char* out, size_t capacity, const char* base,
                       const char* suffix) {
    size_t base_length = strlen(base);
    size_t suffix_length = strlen(suffix);
    size_t total = base_length + 1 + suffix_length + 1;

    /* Copy explicitly instead of using snprintf: GCC's -Wformat-truncation
     * flags the old call when the compiler cannot prove that base fits, and
     * the suite is built with -Werror in CI. */
    if (total > capacity) {
        (void)fprintf(stderr,
                      "io_tests: path '%s/%s' does not fit in %zu bytes\n",
                      base, suffix, capacity);
        exit(2);
    }
    memcpy(out, base, base_length);
    out[base_length] = '/';
    memcpy(out + base_length + 1, suffix, suffix_length + 1);
}

int main(int argc, char** argv) {
    char base_buffer[1024];
    const char* base = ".";
    const char* payload = "hello, lsaca I/O!\n";
    const int64_t payload_length = (int64_t)strlen(payload);
    char buffer[256];
    char address[64];
    char nested[1300];
    char canonical_path[1300];
    int64_t handle;
    int64_t size = 0;
    int64_t mtime = 0;
    int64_t kind = -1;
    int64_t listener = -1;
    int64_t client = -1;
    int64_t server_side = -1;
    int64_t port = -1;
    LsString* text;
    LsVec_String* vector;
    int64_t now_ms;
    int64_t first_monotonic;
    int64_t second_monotonic;
    int32_t year = 0;
    int32_t month = 0;
    int32_t day = 0;
    int32_t hour = 0;
    int32_t minute = 0;
    int32_t second = 0;
    int32_t weekday = -1;
    int32_t yearday = -1;

    (void)ls_runtime_init(argc, argv);
    /* Resolve the scratch directory through the runtime argument vector rather
     * than raw argv[]: on Windows the runtime rebuilds that vector from the
     * wide command line, so non-ASCII paths survive. The suite then creates,
     * lists and removes the directory, which fails if the conversion is
     * wrong. */
    if (argc > 1) {
        LsVec_String arguments = ls_args();

        if (arguments.len > 0) {
            size_t length = ls_str_len(arguments.data[0]);

            CHECK(length > 0);
            CHECK(length < sizeof base_buffer);
            if (length > 0 && length < sizeof base_buffer) {
                memcpy(base_buffer, ls_str_data(arguments.data[0]), length);
                base_buffer[length] = '\0';
                base = base_buffer;
            }
        } else {
            CHECK(false);
        }
        ls_vec_string_free(&arguments);
    }
    build_path(scratch, sizeof scratch, base, "lsaca_io_tests");
    build_path(file_a, sizeof file_a, scratch, "file_a.bin");
    build_path(file_b, sizeof file_b, scratch, "file_b.bin");
    build_path(dir_a, sizeof dir_a, scratch, "dir_a");
    build_path(dir_b, sizeof dir_b, scratch, "dir_b");

    /* --- standard handles and invalid handles --------------------------- */
    CHECK_I64(ls_io_stdin(), 0);
    CHECK_I64(ls_io_stdout(), 1);
    CHECK_I64(ls_io_stderr(), 2);
    CHECK_I64(ls_io_close(987654), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_close(-1), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_read(987654, buffer, 8), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_write(987654, "x", 1), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_flush(987654), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_size(987654), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_seek(987654, 0, 0), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_net_local_port(987654), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_close(ls_io_stdout()), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);

    /* --- scratch directory ---------------------------------------------- */
    (void)io_remove_dir_all(scratch);
    CHECK_I64(io_create_dir_all(scratch), 0);
    CHECK_I64(io_create_dir_all(scratch), 0);
    CHECK_I64(io_metadata(scratch, &size, &mtime, &kind), 0);
    CHECK_I64(kind, 1);
    CHECK(mtime > 0);

    /* --- file round trip ------------------------------------------------ */
    handle = io_create(file_a);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_write(handle, payload, payload_length), payload_length);
    CHECK_I64(ls_io_flush(handle), 0);
    CHECK_I64(ls_io_size(handle), payload_length);
    CHECK_I64(ls_io_close(handle), 0);
    CHECK_I64(ls_io_close(handle), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_read(handle, buffer, 8), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);

    handle = io_open(file_a);
    CHECK(handle >= 0);
    memset(buffer, 0, sizeof buffer);
    CHECK_I64(ls_io_read(handle, buffer, (int64_t)sizeof buffer),
              payload_length);
    CHECK(memcmp(buffer, payload, (size_t)payload_length) == 0);
    CHECK_I64(ls_io_read(handle, buffer, (int64_t)sizeof buffer), 0);
    CHECK_I64(ls_io_seek(handle, 0, 2), payload_length);
    CHECK_I64(ls_io_seek(handle, -3, 2), payload_length - 3);
    memset(buffer, 0, sizeof buffer);
    CHECK_I64(ls_io_read(handle, buffer, 3), 3);
    CHECK(memcmp(buffer, "O!\n", 3) == 0);
    CHECK_I64(ls_io_seek(handle, 0, 1), payload_length);
    CHECK_I64(ls_io_seek(handle, 0, 0), 0);
    CHECK_I64(ls_io_read(handle, buffer, 5), 5);
    CHECK(memcmp(buffer, "hello", 5) == 0);
    CHECK_I64(ls_io_seek(handle, 0, 7), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_io_close(handle), 0);

    /* --- metadata ------------------------------------------------------- */
    CHECK_I64(io_metadata(file_a, &size, &mtime, &kind), 0);
    CHECK_I64(size, payload_length);
    CHECK_I64(kind, 0);
    CHECK(mtime > 0);
    {
        LsString* only = ls_str_cstr(file_a);

        CHECK_I64(ls_io_metadata(only, NULL, NULL, NULL), 0);
        ls_release(only);
    }
    {
        LsString* missing = ls_str_cstr("this/path/does/not/exist.txt");

        CHECK_I64(ls_io_metadata(missing, &size, NULL, &kind), -1);
        ls_release(missing);
        CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    }
    CHECK_I64(io_open("this/path/does/not/exist.txt"), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_create("this/path/does/not/exist.txt"), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    /* --- directory listing ---------------------------------------------- */
    CHECK_I64(io_create_dir(dir_a), 0);
    build_path(nested, sizeof nested, dir_a, "c.txt");
    handle = io_create(nested);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_close(handle), 0);
    build_path(nested, sizeof nested, dir_a, "a.txt");
    handle = io_create(nested);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_close(handle), 0);
    build_path(nested, sizeof nested, dir_a, "b.txt");
    handle = io_create(nested);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_close(handle), 0);
    vector = io_read_dir(dir_a);
    CHECK(vector != NULL);
    if (vector != NULL) {
        CHECK_I64(vector->len, 3);
        CHECK(vector_name_at(vector, 0, "a.txt"));
        CHECK(vector_name_at(vector, 1, "b.txt"));
        CHECK(vector_name_at(vector, 2, "c.txt"));
        ls_release(vector);
    }
    CHECK_I64(ls_io_last_error_kind(), LS_IO_ERROR_NONE);
    vector = io_read_dir(file_a);
    CHECK(vector == NULL);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    vector = io_read_dir("this/path/does/not/exist");
    CHECK(vector == NULL);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    /* --- directories ---------------------------------------------------- */
    CHECK_I64(io_create_dir(dir_b), 0);
    CHECK_I64(io_create_dir(dir_b), -1);
    CHECK_ERROR(LS_IO_ERROR_ALREADY_EXISTS);
    build_path(nested, sizeof nested, dir_b, "x/y/z");
    CHECK_I64(io_create_dir_all(nested), 0);
    CHECK_I64(io_create_dir_all(nested), 0);
    CHECK_I64(io_metadata(nested, &size, &mtime, &kind), 0);
    CHECK_I64(kind, 1);
    CHECK_I64(io_remove_dir(dir_b), -1);
    CHECK_ERROR(LS_IO_ERROR_OTHER);
    build_path(nested, sizeof nested, dir_b, "x/y/z");
    CHECK_I64(io_remove_dir(nested), 0);
    build_path(nested, sizeof nested, dir_b, "x/y");
    CHECK_I64(io_remove_dir(nested), 0);
    build_path(nested, sizeof nested, dir_b, "x");
    CHECK_I64(io_remove_dir(nested), 0);
    CHECK_I64(io_remove_dir(dir_b), 0);
    CHECK_I64(io_remove_dir(dir_b), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_remove_dir_all(file_a), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(io_remove_dir_all("this/path/does/not/exist"), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    /* --- recursive removal ---------------------------------------------- */
    CHECK_I64(io_create_dir_all(dir_b), 0);
    build_path(nested, sizeof nested, dir_b, "one/two/three");
    CHECK_I64(io_create_dir_all(nested), 0);
    build_path(nested, sizeof nested, dir_b, "one/two/three/deep.txt");
    handle = io_create(nested);
    CHECK(handle >= 0);
    CHECK(io_write_text(handle, "deep", 4));
    CHECK_I64(ls_io_close(handle), 0);
    build_path(nested, sizeof nested, dir_b, "top.txt");
    handle = io_create(nested);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_close(handle), 0);
    CHECK_I64(io_remove_dir_all(dir_b), 0);
    CHECK_I64(io_metadata(dir_b, &size, &mtime, &kind), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    /* --- rename --------------------------------------------------------- */
    handle = io_create(file_a);
    CHECK(handle >= 0);
    CHECK_I64(ls_io_close(handle), 0);
    CHECK_I64(io_rename(file_a, file_b), 0);
    CHECK_I64(io_metadata(file_a, &size, &mtime, &kind), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_metadata(file_b, &size, &mtime, &kind), 0);
    CHECK_I64(kind, 0);
    CHECK_I64(io_rename(file_a, file_b), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_remove_file(file_b), 0);
    CHECK_I64(io_remove_file(file_b), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    /* --- canonicalize and working directory ----------------------------- */
    text = io_canonicalize(scratch);
    CHECK(text != NULL);
    if (text != NULL) {
        CHECK(ls_str_len(text) > 0);
        CHECK(ls_str_data(text)[0] != '\0');
        ls_release(text);
    }
    text = io_canonicalize("this/path/does/not/exist");
    CHECK(text == NULL);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_create_dir_all(dir_a), 0);
    build_path(nested, sizeof nested, dir_a, "inner");
    CHECK_I64(io_create_dir_all(nested), 0);
    build_path(canonical_path, sizeof canonical_path, scratch, "dir_a/..");
    if (strlen(canonical_path) + 3 < sizeof canonical_path) {
        strcat(canonical_path, "/.");
    }
    {
        LsString* first = io_canonicalize(scratch);
        LsString* second = io_canonicalize(canonical_path);

        CHECK(first != NULL);
        CHECK(second != NULL);
        if (first != NULL && second != NULL) {
            CHECK(ls_str_eq(first, second));
        }
        if (first != NULL) ls_release(first);
        if (second != NULL) ls_release(second);
    }
    text = ls_io_current_dir();
    CHECK(text != NULL);
    CHECK(text != NULL && ls_str_len(text) > 0);
    if (text != NULL) {
        LsString* saved = ls_str_cstr(ls_str_data(text));

        ls_release(text);
        CHECK_I64(io_set_current_dir(dir_a), 0);
        text = ls_io_current_dir();
        CHECK(text != NULL);
        if (text != NULL) {
            CHECK(!ls_str_eq(text, saved));
            ls_release(text);
        }
        CHECK_I64(io_set_current_dir(ls_str_data(saved)), 0);
        text = ls_io_current_dir();
        CHECK(text != NULL);
        if (text != NULL) {
            CHECK(ls_str_eq(text, saved));
            ls_release(text);
        }
        ls_release(saved);
    }
    CHECK_I64(io_set_current_dir("this/path/does/not/exist"), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);
    text = ls_io_temp_dir();
    CHECK(text != NULL);
    if (text != NULL) {
        CHECK(ls_str_len(text) > 0);
        ls_release(text);
    }

    /* --- environment ---------------------------------------------------- */
    text = io_env_var("PATH");
    CHECK(text != NULL);
    if (text != NULL) {
        CHECK(ls_str_len(text) > 0);
        ls_release(text);
    }
    text = io_env_var("LSACA_IO_TEST_DEFINITELY_UNSET_VAR_42");
    CHECK(text == NULL);
    CHECK_I64(ls_io_last_error_kind(), LS_IO_ERROR_NONE);
    {
        LsString* empty = ls_str_cstr("");

        CHECK(ls_env_var(empty) == NULL);
        ls_release(empty);
    }
    vector = ls_env_vars();
    CHECK(vector != NULL);
    if (vector != NULL) {
        CHECK(vector->len >= 5);
        CHECK(vector_has_prefix(vector, "PATH="));
        ls_release(vector);
    }
    vector = ls_proc_args();
    CHECK(vector != NULL);
    if (vector != NULL) {
        CHECK(vector->len >= 1);
        if (vector->len >= 1) {
            CHECK(ls_str_len(vector->data[0]) > 0);
        }
        ls_release(vector);
    }

    /* --- time ----------------------------------------------------------- */
    now_ms = ls_time_now_ms();
    CHECK(now_ms > 1600000000000LL);
    first_monotonic = ls_time_monotonic_ns();
    ls_time_sleep_ms(20);
    second_monotonic = ls_time_monotonic_ns();
    CHECK(second_monotonic > first_monotonic);
    CHECK(second_monotonic - first_monotonic >= 15000000LL);
    CHECK(ls_time_now_ms() - now_ms >= 10);
    ls_time_sleep_ms(0);
    CHECK(true);
    CHECK_I64(ls_time_utc_parts(now_ms, &year, &month, &day, &hour, &minute,
                                &second, &weekday, &yearday),
              0);
    CHECK(year >= 2020);
    CHECK(month >= 1 && month <= 12);
    CHECK(day >= 1 && day <= 31);
    CHECK(hour >= 0 && hour <= 23);
    CHECK(minute >= 0 && minute <= 59);
    CHECK(second >= 0 && second <= 59);
    CHECK(weekday >= 0 && weekday <= 6);
    CHECK(yearday >= 0 && yearday <= 365);
    CHECK_I64(ls_time_local_parts(now_ms, &year, &month, &day, &hour, &minute,
                                  &second, &weekday, &yearday),
              0);
    CHECK(year >= 2020);
    CHECK(month >= 1 && month <= 12);
    CHECK(day >= 1 && day <= 31);
    CHECK(hour >= 0 && hour <= 23);
    CHECK(minute >= 0 && minute <= 59);
    CHECK(second >= 0 && second <= 59);
    CHECK(weekday >= 0 && weekday <= 6);
    CHECK(yearday >= 0 && yearday <= 365);
    CHECK_I64(ls_time_utc_parts(1600000000000LL, &year, NULL, NULL, NULL, NULL,
                                NULL, NULL, NULL),
              0);
    CHECK(year >= 2020);

    /* --- TCP loopback --------------------------------------------------- */
    listener = io_net_listen("127.0.0.1:0");
    CHECK(listener >= 0);
    port = ls_net_local_port(listener);
    CHECK(port > 0);
    CHECK(port <= 65535);
    (void)snprintf(address, sizeof address, "127.0.0.1:%d", (int)port);
    client = io_net_connect(address);
    CHECK(client >= 0);
    server_side = ls_net_accept(listener);
    CHECK(server_side >= 0);
    if (client >= 0 && server_side >= 0) {
        CHECK_I64(ls_net_set_timeout(server_side, 5000), 0);
        CHECK_I64(ls_net_set_timeout(client, 5000), 0);
        CHECK_I64(ls_io_write(client, "ping", 4), 4);
        memset(buffer, 0, sizeof buffer);
        CHECK_I64(ls_io_read(server_side, buffer, (int64_t)sizeof buffer), 4);
        CHECK(memcmp(buffer, "ping", 4) == 0);
        CHECK_I64(ls_io_write(server_side, "pong", 4), 4);
        memset(buffer, 0, sizeof buffer);
        CHECK_I64(ls_io_read(client, buffer, (int64_t)sizeof buffer), 4);
        CHECK(memcmp(buffer, "pong", 4) == 0);
        CHECK_I64(ls_net_shutdown(client, 2), 0);
        CHECK_I64(ls_net_shutdown(server_side, 0), 0);
        CHECK_I64(ls_io_close(server_side), 0);
        CHECK_I64(ls_io_close(client), 0);
        CHECK_I64(ls_io_close(client), -1);
        CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    }
    CHECK_I64(ls_io_close(listener), 0);
    CHECK_I64(ls_io_close(listener), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(io_net_listen("no port here"), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(io_net_listen("127.0.0.1:70000"), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(io_net_connect("127.0.0.1:1"), -1);
    CHECK_ERROR(LS_IO_ERROR_CONNECTION_REFUSED);
    CHECK_I64(ls_net_set_timeout(987654, 1000), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    CHECK_I64(ls_net_shutdown(987654, 0), -1);
    CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
    {
        int64_t spare = io_net_listen("127.0.0.1:0");

        CHECK(spare >= 0);
        if (spare >= 0) {
            CHECK_I64(ls_net_set_timeout(spare, -5), -1);
            CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
            CHECK_I64(ls_net_shutdown(spare, 7), -1);
            CHECK_ERROR(LS_IO_ERROR_INVALID_INPUT);
            CHECK_I64(ls_io_close(spare), 0);
        }
    }

    /* --- cleanup -------------------------------------------------------- */
    CHECK_I64(io_remove_file(file_a), -1);
    CHECK_I64(ls_io_last_error_kind(), LS_IO_ERROR_NOT_FOUND);
    CHECK_I64(io_remove_dir_all(scratch), 0);
    CHECK_I64(io_metadata(scratch, &size, &mtime, &kind), -1);
    CHECK_ERROR(LS_IO_ERROR_NOT_FOUND);

    ls_runtime_shutdown();
    if (lsaca_failures == 0) {
        (void)fprintf(stdout, "ALL CHECKS PASSED (%d checks)\n", lsaca_checks);
        return 0;
    }
    (void)fprintf(stdout, "%d of %d CHECKS FAILED\n", lsaca_failures,
                  lsaca_checks);
    return 1;
}