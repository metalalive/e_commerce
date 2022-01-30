#include <assert.h>
#include "app.h"

int main(int argc, char *argv[]) {
    assert(argc > 1);
    const char *cfg_file_path = argv[argc - 1];
    const char *exe_path = argv[0]; // program path and name
    return  start_application(cfg_file_path, exe_path);
}

