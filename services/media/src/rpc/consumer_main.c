#include <assert.h>
#include "rpc/consumer.h"

int main(int argc, char *argv[]) {
    assert(argc > 2);
    const char *cfg_file_path = argv[argc - 1];
    // ensure relative path to executable program name ,
    // Note `argv[0]` is also the path to prgoram, however it might be full path
    // or relative path depending on system environment, to reduce such uncertainty
    // executable path is always retrieved from user-defined argument.
    const char *exe_path = argv[argc - 2];
    return start_application(cfg_file_path, exe_path);
}
