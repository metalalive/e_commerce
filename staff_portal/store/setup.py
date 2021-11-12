
if __name__ == '__main__':
    import sys
    from migrations.alembic.config import init_migration, deinit_migration
    from . import settings
    kwargs = {'app_settings':settings, 'orm_base_cls_path':'store.models.Base'}
    if sys.argv[-1] == 'reverse':
        deinit_migration(**kwargs)
    else:
        kwargs['auth_init_rev_id'] = '000002'
        init_migration(**kwargs)

