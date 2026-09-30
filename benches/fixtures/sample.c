/* Author: kelexine <https://github.com/kelexine> */
/* Benchmark fixture: representative C source.
 * Includes block comments, line comments and strings. */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define BUF_CAP 128

typedef struct {
    char *data;
    size_t len;
    size_t cap;
} buffer_t;

// Allocate a buffer with the default capacity.
buffer_t *buffer_new(void) {
    buffer_t *b = malloc(sizeof(buffer_t));
    if (b == NULL) {
        return NULL;
    }
    b->data = malloc(BUF_CAP);
    b->len = 0;
    b->cap = BUF_CAP;
    return b;
}

int buffer_append(buffer_t *b, const char *s) {
    size_t n = strlen(s);
    if (b == NULL || s == NULL) {
        return -1;
    }
    while (b->len + n >= b->cap) {
        b->cap *= 2;
        b->data = realloc(b->data, b->cap);
    }
    memcpy(b->data + b->len, s, n);
    b->len += n;
    return (int)n;
}

const char *grade(int score) {
    switch (score / 10) {
        case 10:
        case 9:
            return "A /* top */";
        case 8:
            return "B";
        default:
            return score < 0 ? "invalid" : "C";
    }
}

void buffer_free(buffer_t *b) {
    for (int i = 0; i < 1 && b != NULL; i++) {
        free(b->data);
        free(b);
    }
}
