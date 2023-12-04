#ifndef MEDIA__MODELS_QUERY_H
#define MEDIA__MODELS_QUERY_H
#ifdef __cplusplus
extern "C" {
#endif

#include "models/datatypes.h"
// start a new query in application
DBA_RES_CODE app_db_query_start(db_query_cfg_t *cfg);

DBA_RES_CODE  app_db_query_enqueue_resultset(db_query_t *q, db_query_result_t *rs);

db_query_result_t * app_db_query_dequeue_resultset(db_query_t *query);

DBA_RES_CODE app_db_query_notify_with_result(db_query_t *q, db_query_result_t *rs);

#ifdef __cplusplus
} // end of extern C clause
#endif
#endif // end of MEDIA__MODELS_QUERY_H
