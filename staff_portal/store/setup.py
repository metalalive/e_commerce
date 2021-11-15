
if __name__ == '__main__':
    import sys
    from migrations.alembic.config import init_migration, deinit_migration
    from .settings import migration as settings
    kwargs = {'app_settings':settings,}
    if sys.argv[-1] == 'reverse':
        deinit_migration(**kwargs)
    else:
        kwargs['auth_init_rev_id'] = '000002'
        init_migration(**kwargs)

