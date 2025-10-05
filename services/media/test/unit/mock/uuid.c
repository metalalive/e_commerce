#include <cgreen/mocks.h>
#include <uuid/uuid.h>

void uuid_generate_random(uuid_t uuo) { mock(uuo); }

void uuid_unparse(const uuid_t uuo, char *out) { mock(uuo, out); }
