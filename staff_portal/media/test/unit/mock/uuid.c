#include <cgreen/mocks.h>
#include <uuid/uuid.h>

void uuid_generate_random(uuid_t out)
{ mock(out); }

void uuid_unparse(const uuid_t uu, char *out)
{ mock(uu, out); }

