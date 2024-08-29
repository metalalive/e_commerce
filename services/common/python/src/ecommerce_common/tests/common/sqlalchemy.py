async def init_test_database(
    dbs_engine, db_engine, metadata_objs, dropdb_sql, createdb_sql, keepdb
):
    async with dbs_engine.connect() as conn:
        async with conn.begin():  # explicit commit on exit, sqlalchemy 2.0 no longer supports auto-commit
            if not keepdb:  # caller might skip to CREATE existing tables
                await conn.exec_driver_sql(dropdb_sql)
            await conn.exec_driver_sql(createdb_sql)
        # the given raw SQL is usually database-dependent, switch to
        # driver-level execution function
    async with db_engine.connect() as conn:
        for metadata in metadata_objs:
            await conn.run_sync(metadata.create_all)


async def deinit_test_database(dbs_engine, db_engine, dropdb_sql, keepdb):
    if keepdb:
        return
    async with dbs_engine.connect() as conn:
        async with conn.begin():
            await conn.exec_driver_sql(dropdb_sql)


async def clean_test_data(conn, metadatas):
    for metadata in metadatas:
        for table in metadata.tables.values():
            async with conn.begin():
                stmt = table.delete()
                result = await conn.execute(stmt)  # will commit automatically
            # if result.rowcount > 0:
            #    pass
