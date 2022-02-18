#include <h2o.h>
#include <h2o/serverutil.h>
#include <cgreen/cgreen.h>
#include "datatypes.h"
#include "routes.h"

#define RESTAPI_ENDPOINT_HANDLER(func_name, http_method, hdlr_var, req_var) \
    static __attribute__((optimize("O0"))) int func_name(RESTAPI_HANDLER_ARGS(hdlr_var, req_var))


RESTAPI_ENDPOINT_HANDLER(_test_service_xyz_action_add, POST, self, req)
{   // dummy for test
    return 0;
}

RESTAPI_ENDPOINT_HANDLER(_test_service_xyz_action_discard, DELETE, self, req)
{
    return 0;
}

RESTAPI_ENDPOINT_HANDLER(_test_service_antutu_action_edit, PATCH, self, req)
{
    return 0;
}


RESTAPI_ENDPOINT_HANDLER(_test_service_n22_melon_action_add, POST, self, req)
{
    return 0;
}


Ensure(setup_route_test) {
    h2o_globalconf_t glbl_cfg;
    h2o_iovec_t host = {.base="localhost", .len=9};
    uint16_t port = 8010;
    const char *exe_path = "./media/build/unit_test.out";
    int result = 0;
    h2o_config_init(&glbl_cfg);
    h2o_hostconf_t *hostcfg = h2o_config_register_host(&glbl_cfg, host, port);
    json_t *urls_cfg = json_array();
    // sub case #1, wrong handler function name
    json_array_append_new(urls_cfg, json_object());
    json_object_set_new(json_array_get(urls_cfg, 0), "path", json_string("/v3/service_XYZ/func123"));
    json_object_set_new(json_array_get(urls_cfg, 0), "entry_fn", json_string("service_xyz_action_one"));
    result = setup_routes(hostcfg, urls_cfg, exe_path);
    assert_that(result, is_equal_to(0));
    assert_that(hostcfg->paths.size, is_equal_to(0));
    // sub case #2, multiple handler functions pointing to one API endpoint (different HTTP methods)
    json_object_del(json_array_get(urls_cfg, 0), "entry_fn");
    json_object_set_new(json_array_get(urls_cfg, 0), "entry_fn", json_string("_test_service_xyz_action_add"));
    json_array_append_new(urls_cfg, json_object());
    json_object_set_new(json_array_get(urls_cfg, 1), "path", json_string("/v3/service_XYZ/func123"));
    json_object_set_new(json_array_get(urls_cfg, 1), "entry_fn", json_string("_test_service_xyz_action_discard"));
    result = setup_routes(hostcfg, urls_cfg, exe_path);
    assert_that(hostcfg->paths.size, is_equal_to(1));
    assert_that(hostcfg->paths.entries[0]->path.base, is_equal_to_string("/v3/service_XYZ/func123"));
    assert_that(hostcfg->paths.entries[0]->handlers.size, is_equal_to(3)); // plus default handler to return status 405
    assert_that(hostcfg->paths.entries[0]->handlers.entries[0]->on_req, is_equal_to(_test_service_xyz_action_add));
    assert_that(hostcfg->paths.entries[0]->handlers.entries[1]->on_req, is_equal_to(_test_service_xyz_action_discard));
    // sub case #3, add another path & handler function
    json_array_clear(urls_cfg);
    json_array_append_new(urls_cfg, json_object());
    json_object_set_new(json_array_get(urls_cfg, 0), "path", json_string("/v3/service_An22/bch"));
    json_object_set_new(json_array_get(urls_cfg, 0), "entry_fn", json_string("_test_service_antutu_action_edit"));
    result = setup_routes(hostcfg, urls_cfg, exe_path);
    assert_that(hostcfg->paths.size, is_equal_to(2));
    assert_that(hostcfg->paths.entries[1]->path.base, is_equal_to_string("/v3/service_An22/bch"));
    assert_that(hostcfg->paths.entries[0]->path.base, is_equal_to_string("/v3/service_XYZ/func123"));
    assert_that(hostcfg->paths.entries[1]->handlers.size, is_equal_to(2)); // plus default handler to return status 405
    assert_that(hostcfg->paths.entries[1]->handlers.entries[0]->on_req, is_equal_to(_test_service_antutu_action_edit));
    // sub case #4, add another longer path & handler function, check sorted paths
    json_array_clear(urls_cfg);
    json_array_append_new(urls_cfg, json_object());
    json_object_set_new(json_array_get(urls_cfg, 0), "path", json_string("/v3/service_An22/bch/melon"));
    json_object_set_new(json_array_get(urls_cfg, 0), "entry_fn", json_string("_test_service_n22_melon_action_add"));
    result = setup_routes(hostcfg, urls_cfg, exe_path);
    assert_that(hostcfg->paths.size, is_equal_to(3));
    assert_that(hostcfg->paths.entries[0]->path.base, is_equal_to_string("/v3/service_An22/bch/melon"));
    assert_that(hostcfg->paths.entries[1]->path.base, is_equal_to_string("/v3/service_XYZ/func123"));
    assert_that(hostcfg->paths.entries[2]->path.base, is_equal_to_string("/v3/service_An22/bch"));
    assert_that(hostcfg->paths.entries[0]->handlers.size, is_equal_to(2)); // plus default handler to return status 405
    assert_that(hostcfg->paths.entries[1]->handlers.size, is_equal_to(3)); // plus default handler to return status 405
    assert_that(hostcfg->paths.entries[0]->handlers.entries[0]->on_req, is_equal_to(_test_service_n22_melon_action_add));
    h2o_config_dispose(&glbl_cfg);
    json_decref(urls_cfg);
} // end of setup_route_test


TestSuite *app_cfg_route_tests(void) {
    TestSuite *suite = create_test_suite();
    add_test(suite, setup_route_test);
    return suite;
}

