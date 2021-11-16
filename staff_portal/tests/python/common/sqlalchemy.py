
def init_test_database(dbs_engine, db_engine, metadata_objs, dropdb_sql, createdb_sql, keepdb):
    with dbs_engine.connect() as conn:
        if not keepdb:
            conn.execute(dropdb_sql)
        conn.execute(createdb_sql)
    # by default it skips to CREATE existing tables
    tuple(map(lambda metadata: metadata.create_all(db_engine), metadata_objs))


def deinit_test_database(dbs_engine, db_engine, dropdb_sql, keepdb):
    if keepdb:
        return
    with dbs_engine.connect() as conn:
        conn.execute(dropdb_sql)

def clean_test_data(conn, metadatas):
    for metadata in metadatas:
        for table in metadata.tables.values():
            stmt = table.delete()
            result = conn.execute(stmt) # will commit automatically
            #if result.rowcount > 0:
            #    pass

