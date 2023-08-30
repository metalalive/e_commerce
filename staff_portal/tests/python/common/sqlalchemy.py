
def init_test_database(dbs_engine, db_engine, metadata_objs, dropdb_sql, createdb_sql, keepdb):
    with dbs_engine.connect() as conn:
        with conn.begin(): # explicit commit on exit, sqlalchemy 2.0 no longer supports auto-commit
            if not keepdb: # caller might skip to CREATE existing tables
                conn.exec_driver_sql(dropdb_sql)
            conn.exec_driver_sql(createdb_sql)
        # the given raw SQL is usually database-dependent, switch to
        # driver-level execution function
    tuple(map(lambda metadata: metadata.create_all(db_engine), metadata_objs))


def deinit_test_database(dbs_engine, db_engine, dropdb_sql, keepdb):
    if keepdb:
        return
    with dbs_engine.connect() as conn:
        with conn.begin():
            conn.exec_driver_sql(dropdb_sql)

def clean_test_data(conn, metadatas):
    for metadata in metadatas:
        for table in metadata.tables.values():
            stmt = table.delete()
            result = conn.execute(stmt) # will commit automatically
            #if result.rowcount > 0:
            #    pass

