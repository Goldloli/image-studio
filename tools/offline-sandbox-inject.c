#include <sandbox.h>
#include <stdio.h>
#include <stdlib.h>

extern const char sandbox_profile[];

__attribute__((constructor))
static void apply_offline_policy(void) {
    char *sandbox_error = NULL;
    if (sandbox_init(sandbox_profile, 0, &sandbox_error) != 0) {
        fprintf(
            stderr,
            "offline sandbox: cannot apply policy: %s\n",
            sandbox_error == NULL ? "unknown error" : sandbox_error
        );
        sandbox_free_error(sandbox_error);
        _Exit(127);
    }
}
